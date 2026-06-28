// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Splits a monorepo's output by detected sub-project ("area"). Each manifest
//! roots an area; every source file is assigned to the deepest manifest
//! directory that contains it. Emits one focused doc per area plus a
//! navigation index and a cross-cutting infrastructure doc, so an agent (or a
//! human) attaches only the file relevant to its question. The global
//! `conventions.md` and the full graph stay whole, so cross-area references
//! are never severed.

use crate::conventions::{
    containerization_summary, endpoint_source_label, framework_stack, library_choices,
};
use squick_core::Project;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

const MAX_ENDPOINTS: usize = 60;
const MAX_FILES: usize = 40;

/// A detected sub-project: the subtree rooted at one manifest-bearing
/// directory. A directory holding several manifests (a polyglot package) is a
/// single area aggregating all of them.
pub struct Area {
    /// Filename-safe, unique identifier used for `area-<slug>.md`.
    pub slug: String,
    /// Human title (the relative directory, or `root`).
    pub title: String,
    /// Directory relative to the repo root (`(root)` for the root).
    pub rel_dir: String,
    /// Indices into `project.manifests` rooted in this directory.
    pub manifest_indices: Vec<usize>,
    /// Indices into `project.files` assigned to this area.
    pub file_indices: Vec<usize>,
}

/// Detects areas for a monorepo. An area is a distinct directory that holds
/// at least one manifest. Returns an empty vector unless there are at least
/// two such directories, which keeps the single-file output for both small
/// repos and polyglot single-root projects (several manifests in one dir).
pub fn detect_areas(project: &Project) -> Vec<Area> {
    if project.manifests.len() < 2 {
        return Vec::new();
    }

    let root = &project.root;
    // Group manifests by their directory, preserving a stable order.
    let mut by_dir: BTreeMap<Vec<String>, Vec<usize>> = BTreeMap::new();
    for (i, manifest) in project.manifests.iter().enumerate() {
        let dir = manifest.path.parent().unwrap_or_else(|| Path::new(""));
        let comps = components(dir.strip_prefix(root).unwrap_or(dir));
        by_dir.entry(comps).or_default().push(i);
    }
    if by_dir.len() < 2 {
        return Vec::new();
    }

    let dirs: Vec<Vec<String>> = by_dir.keys().cloned().collect();
    let mut areas: Vec<Area> = Vec::with_capacity(dirs.len());
    let mut used: BTreeSet<String> = BTreeSet::new();
    for (rel, manifest_indices) in &by_dir {
        let rel_dir = if rel.is_empty() {
            "(root)".to_string()
        } else {
            rel.join("/")
        };
        let first_name = project.manifests[manifest_indices[0]].name.as_deref();
        let slug = unique_slug(base_slug(&rel_dir, first_name), &mut used);
        let title = if rel.is_empty() {
            "root".to_string()
        } else {
            rel_dir.clone()
        };
        areas.push(Area {
            slug,
            title,
            rel_dir,
            manifest_indices: manifest_indices.clone(),
            file_indices: Vec::new(),
        });
    }

    for (fi, file) in project.files.iter().enumerate() {
        let rel = components(file.path.strip_prefix(root).unwrap_or(&file.path));
        let mut best: Option<(usize, usize)> = None;
        for (ai, dir) in dirs.iter().enumerate() {
            if is_prefix(dir, &rel) {
                let depth = dir.len();
                if best.is_none_or(|(_, b)| depth > b) {
                    best = Some((ai, depth));
                }
            }
        }
        if let Some((ai, _)) = best {
            areas[ai].file_indices.push(fi);
        }
    }

    areas
}

/// Union of framework-tag labels across all manifests in an area.
fn area_framework_tags(project: &Project, area: &Area) -> BTreeSet<String> {
    area.manifest_indices
        .iter()
        .flat_map(|&mi| project.manifests[mi].framework_tags.iter())
        .map(|t| t.label.clone())
        .collect()
}

/// Union of dependencies across all manifests in an area.
fn area_deps(project: &Project, area: &Area) -> BTreeSet<String> {
    area.manifest_indices
        .iter()
        .flat_map(|&mi| project.manifests[mi].dependencies.iter().cloned())
        .collect()
}

/// Renders the navigation index: which file answers which kind of question.
pub fn format_navigation(
    project: &Project,
    areas: &[Area],
    has_infra: bool,
    has_schemas: bool,
) -> String {
    let mut out = String::with_capacity(2048);
    let _ = writeln!(out, "# Squick navigation");
    let _ = writeln!(
        out,
        "\nThis repository is split into {} areas. Attach the area file that \
         matches your question; attach `conventions.md` for the global stack \
         and layout. The full graph in `--full` artifacts spans all areas.",
        areas.len()
    );

    let _ = writeln!(out, "\n## Areas");
    for area in areas {
        let tags = area_framework_tags(project, area);
        let frameworks = framework_stack(&tags)
            .into_values()
            .collect::<Vec<_>>()
            .join(", ");
        let endpoints: usize = area
            .file_indices
            .iter()
            .map(|&fi| project.files[fi].endpoints.len())
            .sum();
        let _ = writeln!(
            out,
            "- **{}** (`{}`) -> `area-{}.md` - {}{} file(s), {} endpoint(s)",
            area.title,
            area.rel_dir,
            area.slug,
            if frameworks.is_empty() {
                String::new()
            } else {
                format!("{frameworks}; ")
            },
            area.file_indices.len(),
            endpoints,
        );
    }

    let _ = writeln!(out, "\n## Cross-cutting");
    if has_infra {
        let _ = writeln!(
            out,
            "- **Containers / infrastructure** -> `infra.md` - Dockerfiles and \
             Compose stack (spans areas)"
        );
    }
    let _ = writeln!(
        out,
        "- **Global stack, layout, libraries** -> `conventions.md`"
    );
    if has_schemas {
        let _ = writeln!(out, "- **All endpoints and data schemas** -> `schemas.md`");
    }

    let _ = writeln!(out, "\n## Full structured graph (`--full`)");
    let _ = writeln!(
        out,
        "- `context.txt`, `context.ndjson`, `graph.txt` - one graph over the \
         whole repo; cross-area references are kept intact here."
    );

    out
}

/// Renders one area's focused view: its stack, libraries, API surface, and
/// notable files, scoped to that sub-project.
pub fn format_area(project: &Project, area: &Area) -> String {
    let mut out = String::with_capacity(1024);
    let _ = writeln!(out, "# Squick area: {}", area.title);
    let identity = area
        .manifest_indices
        .iter()
        .map(|&mi| {
            let m = &project.manifests[mi];
            match (&m.name, &m.version) {
                (Some(n), Some(v)) => format!("{n}@{v}"),
                (Some(n), None) => n.clone(),
                _ => "unnamed".to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        out,
        "\nPath: `{}` | {identity} | {} file(s)",
        area.rel_dir,
        area.file_indices.len()
    );

    // Stack: frameworks from this area's manifests, languages from its files.
    let stack = framework_stack(&area_framework_tags(project, area));
    let mut languages: BTreeSet<&str> = area
        .file_indices
        .iter()
        .map(|&fi| project.files[fi].language.as_str())
        .collect();
    languages.remove("");
    if !stack.is_empty() || !languages.is_empty() {
        let _ = writeln!(out, "\n## Stack");
        for (slot, label) in &stack {
            let _ = writeln!(out, "- **{slot}**: {label}");
        }
        if !languages.is_empty() {
            let _ = writeln!(
                out,
                "- **Languages**: {}",
                languages.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
    }

    // Library choices from this area's dependencies.
    let libraries = library_choices(&area_deps(project, area));
    if !libraries.is_empty() {
        let _ = writeln!(out, "\n## Library choices");
        for (category, items) in &libraries {
            let _ = writeln!(
                out,
                "- **{category}**: {}",
                items.iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
    }

    // API surface scoped to this area.
    let mut endpoints: Vec<(&str, String, usize)> = Vec::new();
    let mut sources: BTreeSet<&'static str> = BTreeSet::new();
    for &fi in &area.file_indices {
        let file = &project.files[fi];
        for ep in &file.endpoints {
            endpoints.push((ep.method.as_str(), ep.path.clone(), fi));
            sources.insert(endpoint_source_label(&ep.source));
        }
    }
    if !endpoints.is_empty() {
        let _ = writeln!(out, "\n## API surface");
        let _ = writeln!(
            out,
            "{} endpoint(s) ({})",
            endpoints.len(),
            sources.into_iter().collect::<Vec<_>>().join(", ")
        );
        for (method, path, _) in endpoints.iter().take(MAX_ENDPOINTS) {
            let _ = writeln!(out, "- `{method} {path}`");
        }
        if endpoints.len() > MAX_ENDPOINTS {
            let _ = writeln!(
                out,
                "- ... {} more (see `schemas.md` / `context.txt`)",
                endpoints.len() - MAX_ENDPOINTS
            );
        }
    }

    // Notable files: those carrying detected roles/tags.
    let mut notable: Vec<(String, String)> = Vec::new();
    for &fi in &area.file_indices {
        let file = &project.files[fi];
        if file.semantic_tags.is_empty() {
            continue;
        }
        let rel = rel_path(file.path.strip_prefix(&project.root).unwrap_or(&file.path));
        let mut labels: Vec<&str> = file
            .semantic_tags
            .iter()
            .map(|t| t.label.as_str())
            .collect();
        labels.sort();
        labels.dedup();
        notable.push((rel, labels.join(", ")));
    }
    if !notable.is_empty() {
        let _ = writeln!(out, "\n## Notable files");
        for (rel, tags) in notable.iter().take(MAX_FILES) {
            let _ = writeln!(out, "- `{rel}` - {tags}");
        }
        if notable.len() > MAX_FILES {
            let _ = writeln!(out, "- ... {} more", notable.len() - MAX_FILES);
        }
    }

    out
}

/// Renders the cross-cutting infrastructure doc, or `None` when the repo has
/// no container files.
pub fn format_infra(project: &Project) -> Option<String> {
    let (stack_value, lines) = containerization_summary(project)?;
    let mut out = String::with_capacity(1024);
    let _ = writeln!(out, "# Squick infrastructure");
    let _ = writeln!(
        out,
        "\nCross-cutting container and orchestration configuration for the \
         whole repository ({stack_value})."
    );
    let _ = writeln!(out, "\n## Containerization");
    for line in &lines {
        if line.starts_with(' ') {
            let _ = writeln!(out, "{line}");
        } else {
            let _ = writeln!(out, "- {line}");
        }
    }
    Some(out)
}

fn components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
        .collect()
}

fn is_prefix(dir: &[String], file: &[String]) -> bool {
    dir.len() <= file.len() && dir.iter().zip(file).all(|(a, b)| a == b)
}

fn rel_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn base_slug(rel_dir: &str, manifest_name: Option<&str>) -> String {
    let source = if rel_dir == "(root)" {
        manifest_name.unwrap_or("root")
    } else {
        rel_dir
    };
    let slug: String = source
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "root".to_string()
    } else {
        trimmed
    }
}

fn unique_slug(base: String, used: &mut BTreeSet<String>) -> String {
    if used.insert(base.clone()) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}
