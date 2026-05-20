// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use squick_core::{Confidence, Project, SemanticTag};
use std::fmt::Write;

const MAX_DEPENDENCIES_RENDERED: usize = 12;

pub fn format_markdown(project: &Project) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Squick context");
    let _ = writeln!(
        out,
        "\nRoot: `{}` · Files: {}",
        project.root.display(),
        project.files.len()
    );

    render_overview(&mut out, project);
    render_artifacts_index(&mut out, project);

    if !project.manifests.is_empty() {
        render_project_overview(&mut out, project);
    }

    out
}

fn render_overview(out: &mut String, project: &Project) {
    let mut by_dir: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for file in &project.files {
        let rel = file.path.strip_prefix(&project.root).unwrap_or(&file.path);
        let mut comps = rel.components();
        let first = comps.next();
        let has_more = comps.next().is_some();
        let key = if has_more {
            first
                .map(|c| format!("{}/", c.as_os_str().to_string_lossy()))
                .unwrap_or_else(|| "(root)".to_string())
        } else {
            "(root)".to_string()
        };
        *by_dir.entry(key).or_default() += 1;
    }

    let endpoint_count: usize = project.files.iter().map(|f| f.endpoints.len()).sum();
    let symbol_count: usize = project.files.iter().map(|f| f.symbols.len()).sum();
    let reference_count: usize = project
        .files
        .iter()
        .flat_map(|f| f.symbols.iter())
        .map(|s| s.references.len())
        .sum();

    let mut framework_labels: Vec<String> = project
        .manifests
        .iter()
        .flat_map(|m| m.framework_tags.iter().map(|t| t.label.clone()))
        .collect();
    framework_labels.sort();
    framework_labels.dedup();

    let _ = writeln!(out, "\n## Overview");
    if !by_dir.is_empty() {
        let _ = writeln!(out, "- top-level layout:");
        for (key, count) in &by_dir {
            let noun = if *count == 1 { "file" } else { "files" };
            let _ = writeln!(out, "  - `{key}` ({count} {noun})");
        }
    }
    let _ = writeln!(
        out,
        "- symbols: {symbol_count} · references: {reference_count} · endpoints: {endpoint_count}"
    );
    if !framework_labels.is_empty() {
        let _ = writeln!(out, "- frameworks: {}", framework_labels.join(", "));
    }
    if !project.strapi_schemas.is_empty() {
        let _ = writeln!(
            out,
            "- strapi content types: {}",
            project.strapi_schemas.len()
        );
    }
}

fn render_artifacts_index(out: &mut String, project: &Project) {
    let endpoint_count: usize = project.files.iter().map(|f| f.endpoints.len()).sum();
    let _ = writeln!(out, "\n## Artifacts");
    let _ = writeln!(
        out,
        "- `.squick/context.ndjson` - full structured data (LLM-primary, ~5x smaller than this file)"
    );
    let _ = writeln!(
        out,
        "- `.squick/graph.txt` - subject-predicate-object triples (graph form, ultra-compact)"
    );
    let _ = writeln!(out, "- `.squick/conventions.md` - detected stack and library choices");
    if !project.strapi_schemas.is_empty() || endpoint_count > 0 {
        let _ = writeln!(
            out,
            "- `.squick/schemas.md` - {} content schemas, {} endpoints",
            project.strapi_schemas.len(),
            endpoint_count
        );
    }
}

fn render_project_overview(out: &mut String, project: &Project) {
    let _ = writeln!(out, "\n## Project");
    for manifest in &project.manifests {
        let rel = manifest
            .path
            .strip_prefix(&project.root)
            .unwrap_or(&manifest.path);
        let identity = match (&manifest.name, &manifest.version) {
            (Some(name), Some(version)) => format!("{name}@{version}"),
            (Some(name), None) => name.clone(),
            _ => "<unnamed>".to_string(),
        };
        let _ = writeln!(out, "\n### {identity} _({})_", rel.display());
        if !manifest.framework_tags.is_empty() {
            let _ = writeln!(out, "- frameworks: {}", render_tags(&manifest.framework_tags));
        }
        render_manifest_list(out, "dependencies", &manifest.dependencies);
        render_manifest_list(out, "scripts", &manifest.scripts);
    }
}

fn render_manifest_list(out: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let total = items.len();
    let shown: Vec<&str> = items
        .iter()
        .take(MAX_DEPENDENCIES_RENDERED)
        .map(String::as_str)
        .collect();
    if total > MAX_DEPENDENCIES_RENDERED {
        let _ = writeln!(
            out,
            "- {label} ({total}): {} (and {} more)",
            shown.join(", "),
            total - MAX_DEPENDENCIES_RENDERED
        );
    } else {
        let _ = writeln!(out, "- {label} ({total}): {}", shown.join(", "));
    }
}

fn render_tags(tags: &[SemanticTag]) -> String {
    tags.iter()
        .map(render_tag)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_tag(tag: &SemanticTag) -> String {
    let conf = match tag.confidence {
        Confidence::High => "H",
        Confidence::Medium => "M",
        Confidence::Low => "L",
    };
    format!("`{}`[{}]", tag.label, conf)
}
