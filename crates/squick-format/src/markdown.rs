// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use squick_core::{Confidence, Endpoint, Project, Reference, SemanticTag};
use std::fmt::Write;
use std::path::Path;

const MAX_REFERENCES_RENDERED: usize = 5;
const MAX_DEPENDENCIES_RENDERED: usize = 12;
const MAX_ENDPOINTS_INLINE: usize = 6;

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

    if !project.manifests.is_empty() {
        render_project_overview(&mut out, project);
    }

    let endpoint_count: usize = project.files.iter().map(|f| f.endpoints.len()).sum();
    let has_aux = !project.strapi_schemas.is_empty() || endpoint_count > 0;
    if has_aux {
        let _ = writeln!(
            out,
            "\n_{} data schema(s), {} endpoint(s) extracted to `.squick/schemas.md`._",
            project.strapi_schemas.len(),
            endpoint_count,
        );
    }

    if !project.project_tags.is_empty() {
        let _ = writeln!(out, "\n## Project tags");
        for t in &project.project_tags {
            let _ = writeln!(out, "- {}", render_tag(t));
        }
    }

    let mut rendered_any_file = false;
    let _ = writeln!(out, "\n## Files");
    for file in &project.files {
        if is_file_empty(file) {
            continue;
        }
        rendered_any_file = true;
        let rel = file
            .path
            .strip_prefix(&project.root)
            .unwrap_or(&file.path)
            .display();
        let _ = writeln!(
            out,
            "\n### {} _(lang: {}, lines: {})_",
            rel,
            file.language.as_str(),
            file.line_count
        );
        if !file.semantic_tags.is_empty() {
            let tags = render_tags(&file.semantic_tags);
            let _ = writeln!(out, "- tags: {tags}");
        }
        if !file.imports.is_empty() {
            let _ = writeln!(out, "- imports: {}", file.imports.join(", "));
        }
        if !file.endpoints.is_empty() {
            let _ = writeln!(out, "- endpoints: {}", render_endpoints_inline(&file.endpoints));
        }
        if !file.symbols.is_empty() {
            let _ = writeln!(out, "- symbols:");
            for s in &file.symbols {
                let _ = writeln!(
                    out,
                    "  - `{}` ({:?}) @ L{}",
                    s.name, s.kind, s.line
                );
                if !s.semantic_tags.is_empty() {
                    let _ = writeln!(out, "    - tags: {}", render_tags(&s.semantic_tags));
                }
                if !s.references.is_empty() {
                    let _ = writeln!(
                        out,
                        "    - referenced by: {}",
                        render_references(&s.references, &project.root)
                    );
                }
                if let Some(doc) = &s.doc_comment {
                    let one_line = doc.replace('\n', " ");
                    let trimmed = one_line.trim();
                    if !trimmed.is_empty() {
                        let snippet = if trimmed.len() > 160 {
                            format!("{}...", &trimmed[..160])
                        } else {
                            trimmed.to_string()
                        };
                        let _ = writeln!(out, "    - doc: {snippet}");
                    }
                }
            }
        }
    }
    let skipped = project.files.iter().filter(|f| is_file_empty(f)).count();
    if !rendered_any_file && skipped > 0 {
        let _ = writeln!(out, "\n_All {skipped} scanned file(s) had no extractable signal._");
    } else if skipped > 0 {
        let _ = writeln!(
            out,
            "\n_{skipped} additional file(s) had no extractable signal and were omitted._"
        );
    }
    out
}

fn is_file_empty(file: &squick_core::FileSummary) -> bool {
    file.symbols.is_empty()
        && file.imports.is_empty()
        && file.semantic_tags.is_empty()
        && file.endpoints.is_empty()
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

fn render_endpoints_inline(endpoints: &[Endpoint]) -> String {
    let total = endpoints.len();
    let shown: Vec<String> = endpoints
        .iter()
        .take(MAX_ENDPOINTS_INLINE)
        .map(|e| format!("{} {}", e.method.as_str(), e.path))
        .collect();
    if total > MAX_ENDPOINTS_INLINE {
        format!(
            "{} (and {} more)",
            shown.join(", "),
            total - MAX_ENDPOINTS_INLINE
        )
    } else {
        shown.join(", ")
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

fn render_references(refs: &[Reference], root: &Path) -> String {
    let total = refs.len();
    let shown: Vec<String> = refs
        .iter()
        .take(MAX_REFERENCES_RENDERED)
        .map(|r| {
            let rel = r.from_file.strip_prefix(root).unwrap_or(&r.from_file);
            format!("{}:{}", rel.display(), r.line)
        })
        .collect();
    if total > MAX_REFERENCES_RENDERED {
        format!(
            "{} (and {} more)",
            shown.join(", "),
            total - MAX_REFERENCES_RENDERED
        )
    } else {
        shown.join(", ")
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
