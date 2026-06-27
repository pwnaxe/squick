// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Compact columnar emitter for AI consumers. Same facts as the NDJSON
//! view, but column names are declared once per record type and rows are
//! bare TAB-delimited values. This drops the per-line key/quote/brace
//! overhead that dominates JSON token cost. Plain text the model reads
//! natively; not a binary format.
//!
//! Conventions: a section starts with `@<type> col1 col2 ...`; following
//! lines are its rows. `-` marks an empty cell; `,` separates list values;
//! tabs/newlines inside free text are flattened to spaces.

use squick_core::{DockerArtifact, DockerKind, Project, SemanticTag, StrapiSchema, Symbol};
use std::fmt::Write;

const LEGEND: &str = "# squick compact v1 | '@type col...' header then TAB rows | '-'=empty | ','=list | tabs/newlines in text -> space";

pub fn format_compact(project: &Project) -> String {
    let mut out = String::with_capacity(8 * 1024);
    let _ = writeln!(out, "{LEGEND}");

    push_project(&mut out, project);
    push_manifests(&mut out, project);
    let file_ids = enumerate_file_ids(project);
    push_files(&mut out, project, &file_ids);
    let symbol_ids = push_symbols(&mut out, project, &file_ids);
    push_refs(&mut out, project, &symbol_ids);
    push_endpoints(&mut out, project, &file_ids);
    push_schemas(&mut out, project);
    push_docker(&mut out, project);

    out
}

fn push_project(out: &mut String, project: &Project) {
    let symbols: usize = project.files.iter().map(|f| f.symbols.len()).sum();
    let references: usize = project
        .files
        .iter()
        .flat_map(|f| f.symbols.iter())
        .map(|s| s.references.len())
        .sum();
    let endpoints: usize = project.files.iter().map(|f| f.endpoints.len()).sum();

    let mut frameworks: Vec<&str> = project
        .manifests
        .iter()
        .flat_map(|m| m.framework_tags.iter().map(|t| t.label.as_str()))
        .collect();
    frameworks.sort();
    frameworks.dedup();

    let _ = writeln!(
        out,
        "@proj\troot\tfiles\tsyms\trefs\teps\tschemas\tcontainers\tframeworks"
    );
    let _ = writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        cell(&project.root.display().to_string()),
        project.files.len(),
        symbols,
        references,
        endpoints,
        project.strapi_schemas.len(),
        project.docker.len(),
        list(&frameworks)
    );
}

fn push_manifests(out: &mut String, project: &Project) {
    if project.manifests.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "@man\tid\tpath\tkind\tname\tversion\tdeps\tscripts\tframeworks"
    );
    for (i, man) in project.manifests.iter().enumerate() {
        let fw: Vec<&str> = man
            .framework_tags
            .iter()
            .map(|t| t.label.as_str())
            .collect();
        let _ = writeln!(
            out,
            "m{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            cell(&rel(&man.path, project)),
            cell(&format!("{:?}", man.kind).to_lowercase()),
            opt(man.name.as_deref()),
            opt(man.version.as_deref()),
            list_strings(&man.dependencies),
            list_strings(&man.scripts),
            list(&fw)
        );
    }
}

fn push_files(out: &mut String, project: &Project, file_ids: &[String]) {
    if project.files.is_empty() {
        return;
    }
    let _ = writeln!(out, "@file\tid\tpath\tlang\tloc\ttags\timports");
    for (idx, file) in project.files.iter().enumerate() {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}",
            file_ids[idx],
            cell(&rel(&file.path, project)),
            file.language.as_str(),
            file.line_count,
            tag_list(&file.semantic_tags),
            list_strings(&file.imports)
        );
    }
}

fn push_symbols(out: &mut String, project: &Project, file_ids: &[String]) -> Vec<Vec<String>> {
    let total: usize = project.files.iter().map(|f| f.symbols.len()).sum();
    let mut symbol_index = Vec::with_capacity(project.files.len());
    if total == 0 {
        for _ in &project.files {
            symbol_index.push(Vec::new());
        }
        return symbol_index;
    }

    let _ = writeln!(out, "@sym\tid\tfid\tname\tkind\tline\tcol\tdoc\ttags");
    let mut symbol_id = 0usize;
    for (file_idx, file) in project.files.iter().enumerate() {
        let mut ids_for_file = Vec::with_capacity(file.symbols.len());
        for symbol in &file.symbols {
            let sid = format!("s{symbol_id}");
            symbol_id += 1;
            write_symbol(out, &sid, &file_ids[file_idx], symbol);
            ids_for_file.push(sid);
        }
        symbol_index.push(ids_for_file);
    }
    symbol_index
}

fn write_symbol(out: &mut String, sid: &str, fid: &str, symbol: &Symbol) {
    let _ = writeln!(
        out,
        "{sid}\t{fid}\t{}\t{}\t{}\t{}\t{}\t{}",
        cell(&symbol.name),
        symbol_kind_str(&format!("{:?}", symbol.kind)),
        symbol.line,
        symbol.column,
        opt(symbol.doc_comment.as_deref()),
        tag_list(&symbol.semantic_tags)
    );
}

fn push_refs(out: &mut String, project: &Project, symbol_ids: &[Vec<String>]) {
    let has_refs = project
        .files
        .iter()
        .flat_map(|f| f.symbols.iter())
        .any(|s| !s.references.is_empty());
    if !has_refs {
        return;
    }
    let _ = writeln!(out, "@ref\tfrom\tto\tline");
    for (file_idx, file) in project.files.iter().enumerate() {
        for (sym_idx, symbol) in file.symbols.iter().enumerate() {
            let sid = &symbol_ids[file_idx][sym_idx];
            for reference in &symbol.references {
                let from = reference
                    .from_file
                    .strip_prefix(&project.root)
                    .unwrap_or(&reference.from_file)
                    .display()
                    .to_string();
                let _ = writeln!(out, "{}\t{sid}\t{}", cell(&from), reference.line);
            }
        }
    }
}

fn push_endpoints(out: &mut String, project: &Project, file_ids: &[String]) {
    let has_eps = project.files.iter().any(|f| !f.endpoints.is_empty());
    if !has_eps {
        return;
    }
    let _ = writeln!(out, "@ep\tfid\tmethod\tpath\thandler\tline\tsrc");
    for (idx, file) in project.files.iter().enumerate() {
        for ep in &file.endpoints {
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}",
                file_ids[idx],
                ep.method.as_str(),
                cell(&ep.path),
                opt(ep.handler.as_deref()),
                ep.line,
                format!("{:?}", ep.source).to_lowercase()
            );
        }
    }
}

fn push_schemas(out: &mut String, project: &Project) {
    if project.strapi_schemas.is_empty() {
        return;
    }
    let _ = writeln!(out, "@sch\tname\tdisplay\tkind\tattrs");
    for schema in &project.strapi_schemas {
        push_schema(out, schema);
    }
}

fn push_schema(out: &mut String, schema: &StrapiSchema) {
    let attrs: Vec<String> = schema
        .attributes
        .iter()
        .map(|attr| {
            let required = if attr.required { "!" } else { "" };
            let value = match &attr.relation_target {
                Some(target) => format!("rel:{target}{required}"),
                None => format!("{}{required}", attr.data_type),
            };
            format!("{}={}", attr.name, value)
        })
        .collect();
    let attrs_cell = if attrs.is_empty() {
        "-".to_string()
    } else {
        attrs.join(",")
    };
    let _ = writeln!(
        out,
        "{}\t{}\t{}\t{}",
        cell(schema.singular_name.as_deref().unwrap_or("unnamed")),
        opt(schema.display_name.as_deref()),
        cell(&schema.kind),
        attrs_cell
    );
}

fn push_docker(out: &mut String, project: &Project) {
    if project.docker.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "@dock\tid\tpath\tkind\tfrom\tports\tentrypoint\tcmd\tworkdir\tuser\tenv\targs\tvolumes\ttags"
    );
    for (i, art) in project.docker.iter().enumerate() {
        write_docker(out, i, art, project);
    }
    // Compose services live in their own section, keyed by the dock id.
    let has_services = project.docker.iter().any(|a| !a.services.is_empty());
    if has_services {
        let _ = writeln!(
            out,
            "@docksvc\tdock\tname\timage\tbuild\tports\tcmd\tenv\tenv_file\tvolumes\tnetworks\tdepends_on"
        );
        for (i, art) in project.docker.iter().enumerate() {
            for svc in &art.services {
                let _ = writeln!(
                    out,
                    "d{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    cell(&svc.name),
                    opt(svc.image.as_deref()),
                    opt(svc.build.as_deref()),
                    list_strings(&svc.ports),
                    opt(svc.command.as_deref()),
                    list_strings(&svc.environment),
                    list_strings(&svc.env_file),
                    list_strings(&svc.volumes),
                    list_strings(&svc.networks),
                    list_strings(&svc.depends_on)
                );
            }
        }
    }
}

fn write_docker(out: &mut String, idx: usize, art: &DockerArtifact, _project: &Project) {
    let kind = match art.kind {
        DockerKind::Dockerfile => "dockerfile",
        DockerKind::Compose => "compose",
    };
    let from: Vec<String> = art
        .stages
        .iter()
        .map(|s| match &s.name {
            Some(name) => format!("{} AS {name}", s.base_image),
            None => s.base_image.clone(),
        })
        .collect();
    let _ = writeln!(
        out,
        "d{idx}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        cell(&art.path.display().to_string()),
        kind,
        list_strings(&from),
        list_strings(&art.exposed_ports),
        opt(art.entrypoint.as_deref()),
        opt(art.cmd.as_deref()),
        opt(art.workdir.as_deref()),
        opt(art.user.as_deref()),
        list_strings(&art.env_keys),
        list_strings(&art.build_args),
        list_strings(&art.volumes),
        tag_list(&art.tags)
    );
}

fn enumerate_file_ids(project: &Project) -> Vec<String> {
    (0..project.files.len()).map(|i| format!("f{i}")).collect()
}

fn rel(path: &std::path::Path, project: &Project) -> String {
    path.strip_prefix(&project.root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Sanitizes one cell: flattens embedded tabs/newlines and maps empty to `-`.
fn cell(value: &str) -> String {
    let flattened: String = value
        .chars()
        .map(|c| {
            if c == '\t' || c == '\n' || c == '\r' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = flattened.trim();
    if trimmed.is_empty() {
        "-".to_string()
    } else {
        trimmed.to_string()
    }
}

fn opt(value: Option<&str>) -> String {
    match value {
        Some(v) => cell(v),
        None => "-".to_string(),
    }
}

fn list(values: &[&str]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.iter().map(|v| cell(v)).collect::<Vec<_>>().join(",")
    }
}

fn list_strings(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.iter().map(|v| cell(v)).collect::<Vec<_>>().join(",")
    }
}

fn tag_list(tags: &[SemanticTag]) -> String {
    let mut labels: Vec<&str> = tags.iter().map(|t| t.label.as_str()).collect();
    labels.sort();
    labels.dedup();
    list(&labels)
}

fn symbol_kind_str(debug_form: &str) -> &str {
    match debug_form {
        "Function" => "fn",
        "Method" => "method",
        "Class" => "class",
        "Struct" => "struct",
        "Interface" => "iface",
        "TypeAlias" => "type",
        "Variable" => "var",
        "Constant" => "const",
        "Module" => "mod",
        "Component" => "component",
        "Route" => "route",
        "Handler" => "handler",
        "Model" => "model",
        "Service" => "service",
        "Hook" => "hook",
        "Test" => "test",
        _ => "other",
    }
}
