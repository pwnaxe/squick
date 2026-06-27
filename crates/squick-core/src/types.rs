// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use crate::language::Language;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Deterministic structural signal.
    High,
    /// Strong heuristic match.
    Medium,
    /// Informed guess; consumers should treat the tag as a hint.
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    TypeAlias,
    Variable,
    Constant,
    Module,
    Component,
    Route,
    Handler,
    Model,
    Service,
    Hook,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TagSource {
    Dictionary { dict: String, entry: String },
    Heuristic { rule: String },
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticTag {
    pub label: String,
    pub source: TagSource,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Comment {
    pub line: usize,
    pub text: String,
    pub is_doc: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reference {
    pub from_file: PathBuf,
    pub from_symbol: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub doc_comment: Option<String>,
    pub inline_comments: Vec<Comment>,
    pub references: Vec<Reference>,
    pub semantic_tags: Vec<SemanticTag>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallKind {
    Call,
    JsxComponent,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Options,
    Head,
    Any,
}

impl HttpMethod {
    pub fn from_token(token: &str) -> Option<Self> {
        Some(match token.to_ascii_lowercase().as_str() {
            "get" => HttpMethod::Get,
            "post" => HttpMethod::Post,
            "put" => HttpMethod::Put,
            "delete" | "del" => HttpMethod::Delete,
            "patch" => HttpMethod::Patch,
            "options" => HttpMethod::Options,
            "head" => HttpMethod::Head,
            "all" | "any" => HttpMethod::Any,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Options => "OPTIONS",
            HttpMethod::Head => "HEAD",
            HttpMethod::Any => "ANY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointSource {
    PythonDecorator,
    PythonUrlpatterns,
    JsMethodCall,
    NextjsRouteHandler,
    PhpRoute,
    PhpAttributeRoute,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub method: HttpMethod,
    pub path: String,
    pub handler: Option<String>,
    pub line: usize,
    pub source: EndpointSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallSite {
    pub name: String,
    pub line: usize,
    pub kind: CallKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: PathBuf,
    pub language: Language,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<String>,
    pub semantic_tags: Vec<SemanticTag>,
    pub endpoints: Vec<Endpoint>,
    pub line_count: usize,
    /// Raw call sites collected during extraction. Drained by the
    /// resolver and not serialized.
    #[serde(skip)]
    pub call_sites: Vec<CallSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestKind {
    NodePackage,
    PythonProject,
    PhpComposer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub kind: ManifestKind,
    pub path: PathBuf,
    pub name: Option<String>,
    pub version: Option<String>,
    pub dependencies: Vec<String>,
    pub scripts: Vec<String>,
    pub framework_tags: Vec<SemanticTag>,
}

/// Which container artifact a `DockerArtifact` was parsed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerKind {
    /// A `Dockerfile`, `Dockerfile.*`, `*.dockerfile`, or `Containerfile`.
    Dockerfile,
    /// A `docker-compose.yml` / `compose.yaml` orchestration file.
    Compose,
}

/// One `FROM` line in a Dockerfile. Multi-stage builds yield several.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerStage {
    pub base_image: String,
    /// The `AS <name>` alias, when the stage is named.
    pub name: Option<String>,
}

/// A service declared in a Compose file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerService {
    pub name: String,
    /// Pre-built image reference (`image:` key).
    pub image: Option<String>,
    /// Build context (`build:` key), when the service builds locally.
    pub build: Option<String>,
    pub ports: Vec<String>,
    pub depends_on: Vec<String>,
    /// `command:` override, rendered as a single string.
    pub command: Option<String>,
    /// Keys of `environment:` (values dropped: noise and possible secrets).
    pub environment: Vec<String>,
    /// `env_file:` paths.
    pub env_file: Vec<String>,
    /// `volumes:` entries (short or long form, rendered as strings).
    pub volumes: Vec<String>,
    /// `networks:` the service is attached to.
    pub networks: Vec<String>,
}

/// Container tooling detected in the repository. A Dockerfile populates
/// `stages`/`exposed_ports` plus the runtime/config fields; a Compose file
/// populates `services`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerArtifact {
    pub kind: DockerKind,
    pub path: PathBuf,
    pub stages: Vec<DockerStage>,
    pub exposed_ports: Vec<String>,
    pub services: Vec<DockerService>,
    pub tags: Vec<SemanticTag>,
    /// Effective `ENTRYPOINT` (last one wins), rendered as a string.
    pub entrypoint: Option<String>,
    /// Effective `CMD` (last one wins), rendered as a string.
    pub cmd: Option<String>,
    /// Effective `WORKDIR`.
    pub workdir: Option<String>,
    /// Effective `USER` the container runs as.
    pub user: Option<String>,
    /// Keys declared via `ENV` (values dropped).
    pub env_keys: Vec<String>,
    /// Names declared via `ARG` (build-time configuration).
    pub build_args: Vec<String>,
    /// Paths declared via `VOLUME`.
    pub volumes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrapiAttribute {
    pub name: String,
    pub data_type: String,
    pub required: bool,
    pub relation_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrapiSchema {
    pub path: PathBuf,
    pub kind: String,
    pub singular_name: Option<String>,
    pub plural_name: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub attributes: Vec<StrapiAttribute>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    pub root: PathBuf,
    pub files: Vec<FileSummary>,
    pub project_tags: Vec<SemanticTag>,
    pub manifests: Vec<Manifest>,
    pub strapi_schemas: Vec<StrapiSchema>,
    pub docker: Vec<DockerArtifact>,
}

#[cfg(test)]
mod tests {
    use super::HttpMethod;

    #[test]
    fn parses_standard_http_methods() {
        assert_eq!(HttpMethod::from_token("get"), Some(HttpMethod::Get));
        assert_eq!(HttpMethod::from_token("GET"), Some(HttpMethod::Get));
        assert_eq!(HttpMethod::from_token("Post"), Some(HttpMethod::Post));
        assert_eq!(HttpMethod::from_token("DELETE"), Some(HttpMethod::Delete));
        assert_eq!(HttpMethod::from_token("patch"), Some(HttpMethod::Patch));
    }

    #[test]
    fn delete_aliases() {
        assert_eq!(HttpMethod::from_token("del"), Some(HttpMethod::Delete));
    }

    #[test]
    fn rejects_middleware_and_garbage() {
        assert_eq!(HttpMethod::from_token("use"), None);
        assert_eq!(HttpMethod::from_token("listen"), None);
        assert_eq!(HttpMethod::from_token(""), None);
        assert_eq!(HttpMethod::from_token("getUserById"), None);
    }
}
