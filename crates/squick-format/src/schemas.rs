// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! `schemas.md` renderer: manifests, Strapi content types, and endpoints.

use squick_core::{Endpoint, Project, StrapiSchema};
use std::fmt::Write;
use std::path::Path;

pub fn format_schemas(project: &Project) -> Option<String> {
    let endpoint_count: usize = project.files.iter().map(|f| f.endpoints.len()).sum();
    if project.manifests.is_empty() && project.strapi_schemas.is_empty() && endpoint_count == 0 {
        return None;
    }

    let mut out = String::new();
    let _ = writeln!(out, "# Squick schemas");
    let _ = writeln!(
        out,
        "\nRoot: `{}` · Manifests: {} · Strapi schemas: {} · Endpoints: {}",
        project.root.display(),
        project.manifests.len(),
        project.strapi_schemas.len(),
        endpoint_count,
    );

    if !project.manifests.is_empty() {
        render_manifests(&mut out, project);
    }

    if endpoint_count > 0 {
        render_endpoints(&mut out, project);
    }

    if !project.strapi_schemas.is_empty() {
        render_strapi_schemas(&mut out, project);
    }

    Some(out)
}

fn render_endpoints(out: &mut String, project: &Project) {
    let _ = writeln!(out, "\n## Endpoints");
    let mut had_section = false;
    for file in &project.files {
        if file.endpoints.is_empty() {
            continue;
        }
        let rel = file.path.strip_prefix(&project.root).unwrap_or(&file.path);
        let _ = writeln!(out, "\n### {}", rel.display());
        for endpoint in &file.endpoints {
            render_endpoint_line(out, endpoint, rel);
        }
        had_section = true;
    }
    if !had_section {
        let _ = writeln!(out, "\n_No endpoints detected._");
    }
}

fn render_endpoint_line(out: &mut String, endpoint: &Endpoint, _file: &Path) {
    let handler = endpoint
        .handler
        .as_deref()
        .map(|h| format!(" -> `{h}`"))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "- `{} {}`{} (L{})",
        endpoint.method.as_str(),
        endpoint.path,
        handler,
        endpoint.line
    );
}

fn render_manifests(out: &mut String, project: &Project) {
    let _ = writeln!(out, "\n## Manifests");
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
        let _ = writeln!(out, "\n### {identity}");
        let _ = writeln!(out, "- path: `{}`", rel.display());
        if !manifest.framework_tags.is_empty() {
            let labels: Vec<String> = manifest
                .framework_tags
                .iter()
                .map(|t| t.label.clone())
                .collect();
            let _ = writeln!(out, "- frameworks: {}", labels.join(", "));
        }
        if !manifest.dependencies.is_empty() {
            let _ = writeln!(
                out,
                "- dependencies ({}): {}",
                manifest.dependencies.len(),
                manifest.dependencies.join(", ")
            );
        }
        if !manifest.scripts.is_empty() {
            let _ = writeln!(
                out,
                "- scripts ({}): {}",
                manifest.scripts.len(),
                manifest.scripts.join(", ")
            );
        }
    }
}

fn render_strapi_schemas(out: &mut String, project: &Project) {
    let _ = writeln!(out, "\n## Strapi content types");
    for schema in &project.strapi_schemas {
        render_strapi_schema(out, project, schema);
    }
}

fn render_strapi_schema(out: &mut String, project: &Project, schema: &StrapiSchema) {
    let rel = schema
        .path
        .strip_prefix(&project.root)
        .unwrap_or(&schema.path);
    let title = schema
        .display_name
        .as_deref()
        .or(schema.singular_name.as_deref())
        .unwrap_or("<unnamed>");
    let _ = writeln!(out, "\n### {title} _({})_", schema.kind);
    let _ = writeln!(out, "- path: `{}`", rel.display());
    if let Some(singular) = &schema.singular_name {
        let _ = writeln!(out, "- singularName: `{singular}`");
    }
    if let Some(plural) = &schema.plural_name {
        let _ = writeln!(out, "- pluralName: `{plural}`");
    }
    if let Some(desc) = &schema.description {
        let _ = writeln!(out, "- description: {desc}");
    }
    if !schema.attributes.is_empty() {
        let _ = writeln!(out, "- attributes:");
        for attr in &schema.attributes {
            let required = if attr.required { " (required)" } else { "" };
            let extra = attr
                .relation_target
                .as_ref()
                .map(|t| format!(" -> {t}"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "  - `{}`: {}{}{}",
                attr.name, attr.data_type, extra, required
            );
        }
    }
}
