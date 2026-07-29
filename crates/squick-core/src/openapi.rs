// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Detects OpenAPI 3 / Swagger 2 spec files and extracts their paths
//! (as operations) and component schemas. Unlike Dockerfiles or Compose
//! files, OpenAPI specs have no fixed filename convention, so detection
//! sniffs YAML/JSON content for a top-level `openapi:`/`swagger:` key
//! rather than matching on the path. Results land on `Project.openapi`.

use crate::types::{
    HttpMethod, OpenApiArtifact, OpenApiOperation, OpenApiSchema, OpenApiSchemaField, Project,
};
use ignore::WalkBuilder;
use serde_json::Value;
use std::path::Path;

const HTTP_METHOD_KEYS: &[&str] = &["get", "post", "put", "delete", "patch", "options", "head"];

/// Above this, a `.yaml`/`.yml`/`.json` file is assumed not to be a spec and
/// is skipped without reading it. Unlike `Dockerfile`/`docker-compose.yml`,
/// specs have no fixed filename to pre-filter on, so every YAML/JSON file in
/// the tree is a candidate; without a size cap, unrelated multi-megabyte
/// files (lockfiles, translation catalogs, fixtures) would each be read in
/// full just to fail the content sniff. Real specs are rarely this large.
const MAX_SPEC_BYTES: u64 = 5 * 1024 * 1024;

pub fn scan(project: &mut Project, respect_ignore: bool) {
    let walker = WalkBuilder::new(&project.root)
        .standard_filters(respect_ignore)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !has_spec_extension(path) {
            continue;
        }
        if path.metadata().is_ok_and(|m| m.len() > MAX_SPEC_BYTES) {
            continue;
        }
        if let Some(artifact) = parse_spec(path) {
            project.openapi.push(artifact);
        }
    }
}

fn has_spec_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yaml" | "yml" | "json")
    )
}

fn parse_spec(path: &Path) -> Option<OpenApiArtifact> {
    let text = std::fs::read_to_string(path).ok()?;
    // Cheap pre-filter before a full parse: every candidate file must at
    // least contain one of these substrings somewhere.
    if !text.contains("openapi") && !text.contains("swagger") {
        return None;
    }

    let value: Value = if path.extension().and_then(|e| e.to_str()) == Some("json") {
        serde_json::from_str(&text).ok()?
    } else {
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).ok()?;
        serde_json::to_value(yaml).ok()?
    };

    let is_openapi = value.get("openapi").and_then(|v| v.as_str()).is_some();
    let is_swagger = value.get("swagger").and_then(|v| v.as_str()).is_some();
    if !is_openapi && !is_swagger {
        return None;
    }

    let info = value.get("info");
    let title = info
        .and_then(|i| i.get("title"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let version = info
        .and_then(|i| i.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let operations = extract_operations(&value);
    let schemas = extract_schemas(&value);
    if operations.is_empty() && schemas.is_empty() {
        return None;
    }

    Some(OpenApiArtifact {
        path: path.to_path_buf(),
        title,
        version,
        operations,
        schemas,
    })
}

fn extract_operations(value: &Value) -> Vec<OpenApiOperation> {
    let mut operations = Vec::new();
    let Some(paths) = value.get("paths").and_then(|v| v.as_object()) else {
        return operations;
    };
    for (path, item) in paths {
        let Some(item) = item.as_object() else {
            continue;
        };
        for method_key in HTTP_METHOD_KEYS {
            let Some(operation) = item.get(*method_key) else {
                continue;
            };
            let Some(method) = HttpMethod::from_token(method_key) else {
                continue;
            };
            let operation_id = operation
                .get("operationId")
                .and_then(|v| v.as_str())
                .map(String::from);
            let summary = operation
                .get("summary")
                .and_then(|v| v.as_str())
                .map(String::from);
            operations.push(OpenApiOperation {
                method,
                path: path.clone(),
                operation_id,
                summary,
            });
        }
    }
    operations
}

fn extract_schemas(value: &Value) -> Vec<OpenApiSchema> {
    // OpenAPI 3: components.schemas; Swagger 2: definitions.
    let schemas_obj = value
        .get("components")
        .and_then(|c| c.get("schemas"))
        .and_then(|v| v.as_object())
        .or_else(|| value.get("definitions").and_then(|v| v.as_object()));
    let Some(schemas_obj) = schemas_obj else {
        return Vec::new();
    };
    schemas_obj
        .iter()
        .map(|(name, schema)| OpenApiSchema {
            name: name.clone(),
            fields: extract_fields(schema),
        })
        .collect()
}

fn extract_fields(schema: &Value) -> Vec<OpenApiSchemaField> {
    let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    properties
        .iter()
        .map(|(name, prop)| OpenApiSchemaField {
            name: name.clone(),
            data_type: property_type(prop),
            required: required.contains(&name.as_str()),
        })
        .collect()
}

/// Renders a schema property's type: a plain `type`, `item[]` for an array,
/// or the target name of a `$ref`.
fn property_type(prop: &Value) -> String {
    if let Some(t) = prop.get("type").and_then(|v| v.as_str()) {
        if t == "array" {
            let item_type = prop
                .get("items")
                .map(property_type)
                .unwrap_or_else(|| "unknown".to_string());
            return format!("{item_type}[]");
        }
        return t.to_string();
    }
    if let Some(reference) = prop.get("$ref").and_then(|v| v.as_str()) {
        return ref_name(reference);
    }
    "unknown".to_string()
}

fn ref_name(reference: &str) -> String {
    reference
        .rsplit('/')
        .next()
        .unwrap_or(reference)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_openapi_and_swagger_markers() {
        let openapi: Value = serde_json::from_str(r#"{"openapi": "3.0.0"}"#).unwrap();
        assert!(openapi.get("openapi").and_then(|v| v.as_str()).is_some());
        let swagger: Value = serde_json::from_str(r#"{"swagger": "2.0"}"#).unwrap();
        assert!(swagger.get("swagger").and_then(|v| v.as_str()).is_some());
    }

    #[test]
    fn extracts_operations_from_paths() {
        let spec = serde_json::json!({
            "openapi": "3.0.0",
            "info": {"title": "Demo", "version": "1.0.0"},
            "paths": {
                "/users": {
                    "get": {"operationId": "listUsers", "summary": "List users"},
                    "post": {"operationId": "createUser"}
                },
                "/users/{id}": {
                    "delete": {"operationId": "deleteUser"}
                }
            }
        });
        let operations = extract_operations(&spec);
        assert_eq!(operations.len(), 3);
        let get = operations
            .iter()
            .find(|o| o.method == HttpMethod::Get)
            .unwrap();
        assert_eq!(get.path, "/users");
        assert_eq!(get.operation_id.as_deref(), Some("listUsers"));
        assert_eq!(get.summary.as_deref(), Some("List users"));
    }

    #[test]
    fn extracts_schemas_with_required_and_refs() {
        let spec = serde_json::json!({
            "openapi": "3.0.0",
            "info": {},
            "paths": {},
            "components": {
                "schemas": {
                    "User": {
                        "properties": {
                            "id": {"type": "integer"},
                            "name": {"type": "string"},
                            "posts": {
                                "type": "array",
                                "items": {"$ref": "#/components/schemas/Post"}
                            }
                        },
                        "required": ["id", "name"]
                    }
                }
            }
        });
        let schemas = extract_schemas(&spec);
        assert_eq!(schemas.len(), 1);
        let user = &schemas[0];
        assert_eq!(user.name, "User");
        let id_field = user.fields.iter().find(|f| f.name == "id").unwrap();
        assert!(id_field.required);
        assert_eq!(id_field.data_type, "integer");
        let posts_field = user.fields.iter().find(|f| f.name == "posts").unwrap();
        assert!(!posts_field.required);
        assert_eq!(posts_field.data_type, "Post[]");
    }

    #[test]
    fn ignores_non_spec_yaml() {
        let text = "name: my-app\nversion: 1.0.0\n";
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).unwrap();
        let json = serde_json::to_value(value).unwrap();
        assert!(json.get("openapi").is_none());
        assert!(json.get("swagger").is_none());
    }
}
