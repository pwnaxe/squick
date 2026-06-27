// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! MCP server (stdio transport) backed by `rmcp`. Exposes four tools:
//! `squick_scan`, `squick_get_endpoints`, `squick_get_schemas`,
//! `squick_get_file_context`.

use anyhow::Result;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, ErrorData, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    schemars,
    service::ServiceExt,
    tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler,
};
use serde::{Deserialize, Serialize};
use squick_core::{Project, ScanOptions, Scanner};
use squick_dict::Matcher;
use std::path::PathBuf;
use std::sync::Arc;

pub fn run(dict_dir: Option<PathBuf>) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let server = SquickServer::new(dict_dir);
        let service = server.serve(stdio()).await?;
        service.waiting().await?;
        Ok::<_, anyhow::Error>(())
    })
}

#[derive(Clone)]
pub struct SquickServer {
    dict_dir: Arc<Option<PathBuf>>,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ScanArgs {
    /// Absolute or relative path to the project root.
    pub root: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileArgs {
    /// Absolute or relative path to the project root.
    pub root: String,
    /// File path inside the project, relative to root or absolute.
    pub file: String,
}

#[tool_router]
impl SquickServer {
    pub fn new(dict_dir: Option<PathBuf>) -> Self {
        Self {
            dict_dir: Arc::new(dict_dir),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Scan a project and return the most useful summary: detected stack, library choices, repository layout, and API surface. Same content as `.squick/conventions.md`. Use squick_get_ndjson or squick_get_graph when you need the full structured graph."
    )]
    async fn squick_scan(
        &self,
        Parameters(args): Parameters<ScanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.scan_project(&args.root)?;
        Ok(CallToolResult::success(vec![Content::text(
            squick_format::format_conventions(&project),
        )]))
    }

    #[tool(
        description = "Return the full project context as newline-delimited JSON. Each line is a fact (project, file, symbol, reference, endpoint, schema, manifest). Most compact format for LLM consumption."
    )]
    async fn squick_get_ndjson(
        &self,
        Parameters(args): Parameters<ScanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.scan_project(&args.root)?;
        Ok(CallToolResult::success(vec![Content::text(
            squick_format::format_ndjson(&project),
        )]))
    }

    #[tool(
        description = "Return the project context as RDF-style triples (subject predicate object, one per line). Graph form for traversal queries."
    )]
    async fn squick_get_graph(
        &self,
        Parameters(args): Parameters<ScanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.scan_project(&args.root)?;
        Ok(CallToolResult::success(vec![Content::text(
            squick_format::format_triples(&project),
        )]))
    }

    #[tool(
        description = "Return detected architectural conventions: stack, library choices, repository layout, API surface. Use this to answer 'which library does this project use for X' without scanning the codebase."
    )]
    async fn squick_get_conventions(
        &self,
        Parameters(args): Parameters<ScanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.scan_project(&args.root)?;
        Ok(CallToolResult::success(vec![Content::text(
            squick_format::format_conventions(&project),
        )]))
    }

    #[tool(
        description = "Return the list of HTTP endpoints detected in a project as JSON. Covers FastAPI/Flask decorators, Django urlpatterns, Express member-calls, and Next.js App Router."
    )]
    async fn squick_get_endpoints(
        &self,
        Parameters(args): Parameters<ScanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.scan_project(&args.root)?;
        let endpoints: Vec<_> = project
            .files
            .iter()
            .flat_map(|f| {
                let file = f.path.to_string_lossy().into_owned();
                f.endpoints.iter().map(move |e| EndpointDescriptor {
                    method: e.method.as_str().to_string(),
                    path: e.path.clone(),
                    handler: e.handler.clone(),
                    file: file.clone(),
                    line: e.line,
                })
            })
            .collect();
        let payload = serde_json::to_string_pretty(&endpoints)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(
        description = "Return the list of data schemas detected in a project as JSON. Currently covers Strapi content types (kind, names, attributes, relations)."
    )]
    async fn squick_get_schemas(
        &self,
        Parameters(args): Parameters<ScanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.scan_project(&args.root)?;
        let payload = serde_json::to_string_pretty(&project.strapi_schemas)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    #[tool(
        description = "Return structural context for a single file as markdown. Cheaper than a full project scan when the agent already knows which file it cares about."
    )]
    async fn squick_get_file_context(
        &self,
        Parameters(args): Parameters<FileArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.scan_project(&args.root)?;
        let target = resolve_target_path(&project.root, &args.file);
        let summary = project
            .files
            .iter()
            .find(|f| paths_match(&f.path, &target))
            .ok_or_else(|| {
                ErrorData::invalid_params(format!("file `{}` not found in scan", args.file), None)
            })?;
        let single = Project {
            root: project.root.clone(),
            files: vec![summary.clone()],
            project_tags: Vec::new(),
            manifests: Vec::new(),
            strapi_schemas: Vec::new(),
            docker: Vec::new(),
        };
        let markdown = squick_format::format_markdown(&single);
        Ok(CallToolResult::success(vec![Content::text(markdown)]))
    }

    fn scan_project(&self, root: &str) -> Result<Project, ErrorData> {
        let mut scanner = Scanner::new(ScanOptions::default());
        let mut project = scanner
            .scan_project(&PathBuf::from(root))
            .map_err(|e| ErrorData::internal_error(format!("scan failed: {e}"), None))?;
        self.apply_dictionaries(&mut project)?;
        Ok(project)
    }

    fn apply_dictionaries(&self, project: &mut Project) -> Result<(), ErrorData> {
        let dicts = crate::resolve_dictionaries(self.dict_dir.as_ref().as_deref())
            .map_err(|e| ErrorData::internal_error(format!("loading dictionaries: {e}"), None))?;
        if !dicts.is_empty() {
            Matcher::from_dictionaries(dicts).apply(project);
        }
        Ok(())
    }
}

#[tool_handler]
impl ServerHandler for SquickServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "squick".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            instructions: Some(
                "Squick MCP server. Use squick_scan for a full project map, \
                 squick_get_endpoints / squick_get_schemas for targeted data, \
                 and squick_get_file_context when you already know the file."
                    .to_string(),
            ),
        }
    }
}

#[derive(Debug, Serialize)]
struct EndpointDescriptor {
    method: String,
    path: String,
    handler: Option<String>,
    file: String,
    line: usize,
}

fn resolve_target_path(root: &std::path::Path, file: &str) -> PathBuf {
    let candidate = PathBuf::from(file);
    if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    }
}

fn paths_match(a: &std::path::Path, b: &std::path::Path) -> bool {
    let canon = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    canon(a) == canon(b)
}
