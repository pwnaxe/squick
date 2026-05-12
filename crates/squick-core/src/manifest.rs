// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Discovery and parsing of project manifests and data schemas.
//!
//! Two artifacts are extracted alongside source code:
//!
//! * **Manifests** (currently `package.json`) — declare project identity,
//!   dependencies, and framework affinity. The Node.js layer of a project
//!   may be configured purely through these files even when no JavaScript
//!   import discloses the framework.
//!
//! * **Strapi content-type schemas** (`schema.json`) — declare the data
//!   surface of the CMS. They are extracted into a dedicated artifact
//!   because LLM agents rarely need them, but when they do, the value
//!   is high.

use crate::types::{
    Confidence, Manifest, ManifestKind, Project, SemanticTag, StrapiAttribute, StrapiSchema,
    TagSource,
};
use ignore::WalkBuilder;
use serde_json::Value;
use std::path::Path;

const PACKAGE_JSON: &str = "package.json";
const PYPROJECT_TOML: &str = "pyproject.toml";
const SCHEMA_JSON: &str = "schema.json";

pub fn scan(project: &mut Project, respect_ignore: bool) {
    let walker = WalkBuilder::new(&project.root)
        .standard_filters(respect_ignore)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        match name {
            PACKAGE_JSON => {
                if let Some(manifest) = parse_package_json(path) {
                    project.manifests.push(manifest);
                }
            }
            PYPROJECT_TOML => {
                if let Some(manifest) = parse_pyproject_toml(path) {
                    project.manifests.push(manifest);
                }
            }
            SCHEMA_JSON => {
                if let Some(schema) = parse_strapi_schema(path) {
                    project.strapi_schemas.push(schema);
                }
            }
            _ => {}
        }
    }
}

fn parse_package_json(path: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;

    let name = value.get("name").and_then(|v| v.as_str()).map(String::from);
    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);
    let dependencies = collect_dependencies(&value);
    let scripts = value
        .get("scripts")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();
    let framework_tags = derive_framework_tags(&dependencies);

    Some(Manifest {
        kind: ManifestKind::NodePackage,
        path: path.to_path_buf(),
        name,
        version,
        dependencies,
        scripts,
        framework_tags,
    })
}

fn collect_dependencies(value: &Value) -> Vec<String> {
    const FIELDS: &[&str] = &["dependencies", "devDependencies", "peerDependencies"];
    let mut deps = Vec::new();
    for field in FIELDS {
        if let Some(obj) = value.get(*field).and_then(|v| v.as_object()) {
            for key in obj.keys() {
                if !deps.contains(key) {
                    deps.push(key.clone());
                }
            }
        }
    }
    deps
}

fn parse_pyproject_toml(path: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;

    let project_table = value.get("project").and_then(|v| v.as_table());
    let poetry_table = value
        .get("tool")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("poetry"))
        .and_then(|v| v.as_table());

    let name = project_table
        .and_then(|t| t.get("name"))
        .or_else(|| poetry_table.and_then(|t| t.get("name")))
        .and_then(|v| v.as_str())
        .map(String::from);

    let version = project_table
        .and_then(|t| t.get("version"))
        .or_else(|| poetry_table.and_then(|t| t.get("version")))
        .and_then(|v| v.as_str())
        .map(String::from);

    let dependencies = collect_python_dependencies(project_table, poetry_table);
    let scripts = collect_python_scripts(project_table, poetry_table);
    let framework_tags = derive_python_framework_tags(&dependencies);

    Some(Manifest {
        kind: ManifestKind::PythonProject,
        path: path.to_path_buf(),
        name,
        version,
        dependencies,
        scripts,
        framework_tags,
    })
}

fn collect_python_dependencies(
    project_table: Option<&toml::map::Map<String, toml::Value>>,
    poetry_table: Option<&toml::map::Map<String, toml::Value>>,
) -> Vec<String> {
    let mut deps = Vec::new();

    if let Some(list) = project_table
        .and_then(|t| t.get("dependencies"))
        .and_then(|v| v.as_array())
    {
        for item in list {
            if let Some(spec) = item.as_str() {
                let name = python_requirement_name(spec);
                if !name.is_empty() && !deps.contains(&name) {
                    deps.push(name);
                }
            }
        }
    }

    if let Some(table) = project_table
        .and_then(|t| t.get("optional-dependencies"))
        .and_then(|v| v.as_table())
    {
        for group in table.values() {
            if let Some(list) = group.as_array() {
                for item in list {
                    if let Some(spec) = item.as_str() {
                        let name = python_requirement_name(spec);
                        if !name.is_empty() && !deps.contains(&name) {
                            deps.push(name);
                        }
                    }
                }
            }
        }
    }

    if let Some(table) = poetry_table
        .and_then(|t| t.get("dependencies"))
        .and_then(|v| v.as_table())
    {
        for key in table.keys() {
            if key != "python" && !deps.contains(key) {
                deps.push(key.clone());
            }
        }
    }

    deps
}

fn collect_python_scripts(
    project_table: Option<&toml::map::Map<String, toml::Value>>,
    poetry_table: Option<&toml::map::Map<String, toml::Value>>,
) -> Vec<String> {
    let mut scripts = Vec::new();
    if let Some(table) = project_table
        .and_then(|t| t.get("scripts"))
        .and_then(|v| v.as_table())
    {
        scripts.extend(table.keys().cloned());
    }
    if let Some(table) = poetry_table
        .and_then(|t| t.get("scripts"))
        .and_then(|v| v.as_table())
    {
        for key in table.keys() {
            if !scripts.contains(key) {
                scripts.push(key.clone());
            }
        }
    }
    scripts
}

/// Strips PEP 508 requirement specifiers (extras, version, markers) and
/// returns the canonical distribution name.
fn python_requirement_name(spec: &str) -> String {
    let stripped: String = spec
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .collect();
    stripped.trim().to_string()
}

fn derive_python_framework_tags(dependencies: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();
    let mut emit = |label: &str| {
        if !tags.iter().any(|t: &SemanticTag| t.label == label) {
            tags.push(SemanticTag {
                label: label.to_string(),
                source: TagSource::Heuristic {
                    rule: "pyproject-dependency".to_string(),
                },
                confidence: Confidence::High,
            });
        }
    };
    for dep in dependencies {
        let lower = dep.to_ascii_lowercase();
        match lower.as_str() {
            "django" => emit("framework-django"),
            "djangorestframework" => emit("framework-drf"),
            "fastapi" => emit("framework-fastapi"),
            "flask" => emit("framework-flask"),
            "starlette" => emit("framework-starlette"),
            "litestar" => emit("framework-litestar"),
            "tornado" => emit("framework-tornado"),
            "pyramid" => emit("framework-pyramid"),
            "sanic" => emit("framework-sanic"),
            "sqlalchemy" => emit("orm-sqlalchemy"),
            "tortoise-orm" => emit("orm-tortoise"),
            "peewee" => emit("orm-peewee"),
            "alembic" => emit("orm-alembic-migrations"),
            "pydantic" => emit("validation-pydantic"),
            "celery" => emit("task-queue-celery"),
            "rq" => emit("task-queue-rq"),
            "pytest" => emit("test-pytest"),
            _ => {}
        }
    }
    tags
}

fn derive_framework_tags(dependencies: &[String]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();
    let mut emit = |label: &str| {
        if !tags.iter().any(|t: &SemanticTag| t.label == label) {
            tags.push(SemanticTag {
                label: label.to_string(),
                source: TagSource::Heuristic {
                    rule: "package-json-dependency".to_string(),
                },
                confidence: Confidence::High,
            });
        }
    };
    for dep in dependencies {
        if dep == "@strapi/strapi" || dep.starts_with("@strapi/") || dep == "strapi" {
            emit("framework-strapi");
        }
        if dep == "next" {
            emit("framework-nextjs");
        }
        if dep == "react" || dep == "react-dom" {
            emit("framework-react");
        }
        if dep == "vue" || dep.starts_with("@vue/") {
            emit("framework-vue");
        }
        if dep.starts_with("@angular/") {
            emit("framework-angular");
        }
        if dep == "express" {
            emit("framework-express");
        }
        if dep == "fastify" || dep.starts_with("@fastify/") {
            emit("framework-fastify");
        }
        if dep == "@nestjs/core" || dep.starts_with("@nestjs/") {
            emit("framework-nestjs");
        }
        if dep == "svelte" || dep.starts_with("@sveltejs/") {
            emit("framework-svelte");
        }
        if dep == "remix" || dep.starts_with("@remix-run/") {
            emit("framework-remix");
        }
        if dep == "astro" {
            emit("framework-astro");
        }
        if dep == "tailwindcss" {
            emit("styling-tailwind");
        }
        if dep == "next-intl" || dep == "react-intl" || dep == "i18next" {
            emit("i18n");
        }
        if dep == "prisma" || dep == "@prisma/client" {
            emit("orm-prisma");
        }
        if dep == "typeorm" {
            emit("orm-typeorm");
        }
        if dep == "drizzle-orm" {
            emit("orm-drizzle");
        }
        if dep == "mongoose" {
            emit("orm-mongoose");
        }
    }
    tags
}

fn parse_strapi_schema(path: &Path) -> Option<StrapiSchema> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;

    let kind = value.get("kind").and_then(|v| v.as_str())?;
    if kind != "collectionType" && kind != "singleType" {
        return None;
    }

    let info = value.get("info").and_then(|v| v.as_object());
    let singular_name = info
        .and_then(|i| i.get("singularName"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let plural_name = info
        .and_then(|i| i.get("pluralName"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let display_name = info
        .and_then(|i| i.get("displayName"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let description = info
        .and_then(|i| i.get("description"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let attributes = value
        .get("attributes")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(name, attr)| extract_attribute(name, attr))
                .collect()
        })
        .unwrap_or_default();

    Some(StrapiSchema {
        path: path.to_path_buf(),
        kind: kind.to_string(),
        singular_name,
        plural_name,
        display_name,
        description,
        attributes,
    })
}

fn extract_attribute(name: &str, attr: &Value) -> StrapiAttribute {
    let data_type = attr
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let required = attr
        .get("required")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let relation_target = if data_type == "relation" {
        attr.get("target")
            .and_then(|v| v.as_str())
            .map(String::from)
    } else {
        None
    };
    StrapiAttribute {
        name: name.to_string(),
        data_type,
        required,
        relation_target,
    }
}
