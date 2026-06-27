// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! RDF-style triple emitter (subject predicate object, one per line).
//! Compact graph representation. No quoting, no nesting; identifiers
//! use short URI-like prefixes (`file:`, `sym:`, `schema:`, `ep:`).

use squick_core::{
    DockerArtifact, DockerKind, Endpoint, FileSummary, Manifest, Project, StrapiSchema, Symbol,
};
use std::fmt::Write;

pub fn format_triples(project: &Project) -> String {
    let mut out = String::with_capacity(8 * 1024);
    let root_id = "proj";

    project_triples(&mut out, root_id, project);
    for manifest in &project.manifests {
        manifest_triples(&mut out, root_id, manifest, &project.root);
    }
    for file in &project.files {
        file_triples(&mut out, root_id, file, &project.root);
    }
    for file in &project.files {
        for symbol in &file.symbols {
            symbol_triples(&mut out, file, symbol, &project.root);
        }
    }
    for file in &project.files {
        for endpoint in &file.endpoints {
            endpoint_triples(&mut out, file, endpoint, &project.root);
        }
    }
    for schema in &project.strapi_schemas {
        schema_triples(&mut out, schema);
    }
    for artifact in &project.docker {
        docker_triples(&mut out, root_id, artifact, &project.root);
    }

    out
}

fn docker_triples(out: &mut String, proj: &str, artifact: &DockerArtifact, root: &std::path::Path) {
    let path = relative_path(&artifact.path, root);
    let id = format!("docker:{path}");
    let kind = match artifact.kind {
        DockerKind::Dockerfile => "dockerfile",
        DockerKind::Compose => "compose",
    };
    let _ = writeln!(out, "{id} type {kind}");
    let _ = writeln!(out, "{proj} declares {id}");
    for stage in &artifact.stages {
        let _ = writeln!(out, "{id} from image:{}", stage.base_image);
        if let Some(name) = &stage.name {
            let _ = writeln!(out, "{id} stage {name}");
        }
    }
    for port in &artifact.exposed_ports {
        let _ = writeln!(out, "{id} exposes port:{port}");
    }
    if let Some(entrypoint) = &artifact.entrypoint {
        let _ = writeln!(out, "{id} entrypoint {entrypoint}");
    }
    if let Some(cmd) = &artifact.cmd {
        let _ = writeln!(out, "{id} cmd {cmd}");
    }
    if let Some(workdir) = &artifact.workdir {
        let _ = writeln!(out, "{id} workdir {workdir}");
    }
    if let Some(user) = &artifact.user {
        let _ = writeln!(out, "{id} user {user}");
    }
    for key in &artifact.env_keys {
        let _ = writeln!(out, "{id} env {key}");
    }
    for arg in &artifact.build_args {
        let _ = writeln!(out, "{id} arg {arg}");
    }
    for volume in &artifact.volumes {
        let _ = writeln!(out, "{id} volume {volume}");
    }
    for service in &artifact.services {
        let _ = writeln!(out, "{id} service svc:{}", service.name);
        if let Some(image) = &service.image {
            let _ = writeln!(out, "svc:{} image {image}", service.name);
        }
        if let Some(command) = &service.command {
            let _ = writeln!(out, "svc:{} cmd {command}", service.name);
        }
        for dep in &service.depends_on {
            let _ = writeln!(out, "svc:{} depends svc:{dep}", service.name);
        }
        for key in &service.environment {
            let _ = writeln!(out, "svc:{} env {key}", service.name);
        }
        for volume in &service.volumes {
            let _ = writeln!(out, "svc:{} volume {volume}", service.name);
        }
        for network in &service.networks {
            let _ = writeln!(out, "svc:{} network {network}", service.name);
        }
    }
    for tag in &artifact.tags {
        let _ = writeln!(out, "{id} tag {}", tag.label);
    }
}

fn project_triples(out: &mut String, id: &str, project: &Project) {
    let _ = writeln!(out, "{id} type project");
    let _ = writeln!(out, "{id} root {}", project.root.display());

    let mut frameworks: Vec<&str> = project
        .manifests
        .iter()
        .flat_map(|m| m.framework_tags.iter().map(|t| t.label.as_str()))
        .collect();
    frameworks.sort();
    frameworks.dedup();
    for fw in frameworks {
        let stripped = fw.strip_prefix("framework-").unwrap_or(fw);
        let _ = writeln!(out, "{id} uses framework:{stripped}");
    }
}

fn manifest_triples(out: &mut String, proj: &str, manifest: &Manifest, root: &std::path::Path) {
    let path = relative_path(&manifest.path, root);
    let id = format!("man:{path}");
    let _ = writeln!(out, "{id} type manifest");
    let _ = writeln!(out, "{proj} declares {id}");
    if let Some(name) = &manifest.name {
        let _ = writeln!(out, "{id} name {name}");
    }
    if let Some(version) = &manifest.version {
        let _ = writeln!(out, "{id} version {version}");
    }
    for dep in &manifest.dependencies {
        let _ = writeln!(out, "{id} depends dep:{dep}");
    }
    for script in &manifest.scripts {
        let _ = writeln!(out, "{id} script {script}");
    }
    for tag in &manifest.framework_tags {
        let stripped = tag.label.strip_prefix("framework-").unwrap_or(&tag.label);
        let _ = writeln!(out, "{id} uses framework:{stripped}");
    }
}

fn file_triples(out: &mut String, proj: &str, file: &FileSummary, root: &std::path::Path) {
    let path = relative_path(&file.path, root);
    let id = format!("file:{path}");
    let _ = writeln!(out, "{id} type file");
    let _ = writeln!(out, "{proj} contains {id}");
    let _ = writeln!(out, "{id} lang {}", file.language.as_str());
    let _ = writeln!(out, "{id} loc {}", file.line_count);
    for import in &file.imports {
        let _ = writeln!(out, "{id} imports dep:{import}");
    }
    for tag in &file.semantic_tags {
        let _ = writeln!(out, "{id} tag {}", tag.label);
    }
    for endpoint in &file.endpoints {
        let _ = writeln!(
            out,
            "{id} exposes ep:{}:{}",
            endpoint.method.as_str(),
            endpoint.path
        );
    }
}

fn symbol_triples(out: &mut String, file: &FileSummary, symbol: &Symbol, root: &std::path::Path) {
    let path = relative_path(&file.path, root);
    let file_id = format!("file:{path}");
    let sym_id = format!("sym:{path}#{}", symbol.name);
    let _ = writeln!(out, "{sym_id} type symbol");
    let _ = writeln!(out, "{file_id} defines {sym_id}");
    let _ = writeln!(
        out,
        "{sym_id} kind {}",
        format_kind(&format!("{:?}", symbol.kind))
    );
    let _ = writeln!(out, "{sym_id} line {}", symbol.line);
    if let Some(doc) = &symbol.doc_comment {
        let one_line = doc.replace('\n', " ");
        let trimmed = one_line.trim();
        if !trimmed.is_empty() {
            let snippet = if trimmed.len() > 120 {
                format!("{}...", &trimmed[..120])
            } else {
                trimmed.to_string()
            };
            let _ = writeln!(out, "{sym_id} doc {snippet}");
        }
    }
    for tag in &symbol.semantic_tags {
        let _ = writeln!(out, "{sym_id} tag {}", tag.label);
    }
    for reference in &symbol.references {
        let from = relative_path(&reference.from_file, root);
        let _ = writeln!(out, "file:{from} refs {sym_id}");
    }
}

fn endpoint_triples(
    out: &mut String,
    file: &FileSummary,
    endpoint: &Endpoint,
    root: &std::path::Path,
) {
    let path = relative_path(&file.path, root);
    let ep_id = format!("ep:{}:{}", endpoint.method.as_str(), endpoint.path);
    let _ = writeln!(out, "{ep_id} type endpoint");
    let _ = writeln!(out, "{ep_id} method {}", endpoint.method.as_str());
    let _ = writeln!(out, "{ep_id} path {}", endpoint.path);
    let _ = writeln!(out, "{ep_id} defined-in file:{path}");
    let _ = writeln!(out, "{ep_id} line {}", endpoint.line);
    if let Some(handler) = &endpoint.handler {
        let _ = writeln!(out, "{ep_id} handler {handler}");
    }
}

fn schema_triples(out: &mut String, schema: &StrapiSchema) {
    let name = schema.singular_name.as_deref().unwrap_or("unnamed");
    let id = format!("schema:{name}");
    let _ = writeln!(out, "{id} type content-schema");
    let _ = writeln!(out, "{id} kind {}", schema.kind);
    if let Some(display) = &schema.display_name {
        let one_line = display.replace('\n', " ");
        let _ = writeln!(out, "{id} display {}", one_line.trim());
    }
    for attr in &schema.attributes {
        let required_marker = if attr.required { "!" } else { "" };
        let value = match &attr.relation_target {
            Some(target) => format!("rel:{target}{required_marker}"),
            None => format!("{}{required_marker}", attr.data_type),
        };
        let _ = writeln!(out, "{id} field {} {value}", attr.name);
    }
}

fn relative_path(path: &std::path::Path, root: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn format_kind(debug_form: &str) -> &str {
    match debug_form {
        "Function" => "fn",
        "Method" => "method",
        "Class" => "class",
        "Struct" => "struct",
        "Interface" => "interface",
        "TypeAlias" => "type",
        "Variable" => "var",
        "Constant" => "const",
        "Module" => "module",
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
