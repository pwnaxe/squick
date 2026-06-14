// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Newline-delimited JSON emitter. One fact per line, short keys,
//! stable IDs, no formatting overhead. Designed as the primary input
//! for LLM-driven consumers.

use serde::Serialize;
use serde_json::json;
use squick_core::{Endpoint, FileSummary, Manifest, Project, SemanticTag, StrapiSchema, Symbol};
use std::fmt::Write;

pub fn format_ndjson(project: &Project) -> String {
    let mut out = String::with_capacity(8 * 1024);

    push_project(&mut out, project);
    for (i, manifest) in project.manifests.iter().enumerate() {
        push_manifest(&mut out, i, manifest, &project.root);
    }
    let file_ids = enumerate_file_ids(project);
    for (idx, file) in project.files.iter().enumerate() {
        push_file(&mut out, &file_ids[idx], file, &project.root);
    }
    let mut symbol_id = 0usize;
    let mut symbol_index = Vec::with_capacity(project.files.len());
    for (file_idx, file) in project.files.iter().enumerate() {
        let mut ids_for_file = Vec::with_capacity(file.symbols.len());
        for symbol in &file.symbols {
            let sid = format!("s{symbol_id}");
            symbol_id += 1;
            push_symbol(&mut out, &sid, &file_ids[file_idx], symbol);
            ids_for_file.push(sid);
        }
        symbol_index.push(ids_for_file);
    }
    for (file_idx, file) in project.files.iter().enumerate() {
        for (sym_idx, symbol) in file.symbols.iter().enumerate() {
            let sid = &symbol_index[file_idx][sym_idx];
            for reference in &symbol.references {
                push_ref(&mut out, reference, sid, &project.root);
            }
        }
    }
    for (file_idx, file) in project.files.iter().enumerate() {
        for endpoint in &file.endpoints {
            push_endpoint(&mut out, endpoint, &file_ids[file_idx]);
        }
    }
    for schema in &project.strapi_schemas {
        push_schema(&mut out, schema);
    }

    out
}

#[derive(Serialize)]
struct ProjectFact<'a> {
    k: &'a str,
    root: String,
    files: usize,
    symbols: usize,
    references: usize,
    endpoints: usize,
    schemas: usize,
    frameworks: Vec<&'a str>,
}

fn push_project(out: &mut String, project: &Project) {
    let symbols = project.files.iter().map(|f| f.symbols.len()).sum();
    let references = project
        .files
        .iter()
        .flat_map(|f| f.symbols.iter())
        .map(|s| s.references.len())
        .sum();
    let endpoints = project.files.iter().map(|f| f.endpoints.len()).sum();

    let mut frameworks: Vec<&str> = project
        .manifests
        .iter()
        .flat_map(|m| m.framework_tags.iter().map(|t| t.label.as_str()))
        .collect();
    frameworks.sort();
    frameworks.dedup();

    let fact = ProjectFact {
        k: "proj",
        root: project.root.display().to_string(),
        files: project.files.len(),
        symbols,
        references,
        endpoints,
        schemas: project.strapi_schemas.len(),
        frameworks,
    };
    let _ = writeln!(out, "{}", serde_json::to_string(&fact).unwrap());
}

fn push_manifest(out: &mut String, idx: usize, manifest: &Manifest, root: &std::path::Path) {
    let path = manifest
        .path
        .strip_prefix(root)
        .unwrap_or(&manifest.path)
        .display()
        .to_string();
    let fact = json!({
        "k": "man",
        "id": format!("m{idx}"),
        "p": path,
        "kind": format!("{:?}", manifest.kind).to_lowercase(),
        "name": manifest.name,
        "v": manifest.version,
        "deps": manifest.dependencies,
        "scripts": manifest.scripts,
        "fw": label_list(&manifest.framework_tags),
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&fact).unwrap());
}

fn enumerate_file_ids(project: &Project) -> Vec<String> {
    (0..project.files.len()).map(|i| format!("f{i}")).collect()
}

fn push_file(out: &mut String, id: &str, file: &FileSummary, root: &std::path::Path) {
    let path = file
        .path
        .strip_prefix(root)
        .unwrap_or(&file.path)
        .display()
        .to_string();
    let fact = json!({
        "k": "file",
        "id": id,
        "p": path,
        "lang": file.language.as_str(),
        "loc": file.line_count,
        "imp": file.imports,
        "tags": label_list(&file.semantic_tags),
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&fact).unwrap());
}

fn push_symbol(out: &mut String, sid: &str, fid: &str, symbol: &Symbol) {
    let fact = json!({
        "k": "sym",
        "id": sid,
        "fid": fid,
        "n": symbol.name,
        "t": symbol_kind_str(&format!("{:?}", symbol.kind)),
        "l": symbol.line,
        "col": symbol.column,
        "doc": symbol.doc_comment,
        "tags": label_list(&symbol.semantic_tags),
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&fact).unwrap());
}

fn push_ref(
    out: &mut String,
    reference: &squick_core::Reference,
    to_sid: &str,
    root: &std::path::Path,
) {
    let from = reference
        .from_file
        .strip_prefix(root)
        .unwrap_or(&reference.from_file)
        .display()
        .to_string();
    let fact = json!({
        "k": "ref",
        "from": from,
        "to": to_sid,
        "l": reference.line,
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&fact).unwrap());
}

fn push_endpoint(out: &mut String, endpoint: &Endpoint, fid: &str) {
    let fact = json!({
        "k": "ep",
        "fid": fid,
        "method": endpoint.method.as_str(),
        "path": endpoint.path,
        "h": endpoint.handler,
        "l": endpoint.line,
        "src": format!("{:?}", endpoint.source).to_lowercase(),
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&fact).unwrap());
}

fn push_schema(out: &mut String, schema: &StrapiSchema) {
    let mut attrs = serde_json::Map::new();
    for attr in &schema.attributes {
        let required_marker = if attr.required { "!" } else { "" };
        let value = match &attr.relation_target {
            Some(target) => format!("rel:{target}{required_marker}"),
            None => format!("{}{required_marker}", attr.data_type),
        };
        attrs.insert(attr.name.clone(), serde_json::Value::String(value));
    }
    let fact = json!({
        "k": "sch",
        "name": schema.singular_name.as_deref().unwrap_or("unnamed"),
        "display": schema.display_name,
        "kind": schema.kind,
        "attrs": attrs,
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&fact).unwrap());
}

fn label_list(tags: &[SemanticTag]) -> Vec<&str> {
    let mut out: Vec<&str> = tags.iter().map(|t| t.label.as_str()).collect();
    out.sort();
    out.dedup();
    out
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
