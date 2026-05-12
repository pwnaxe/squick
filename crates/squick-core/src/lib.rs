// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Structural and semantic analysis of source code.
//!
//! This crate is language- and framework-agnostic at the orchestration
//! layer. Per-language behavior is contained in `parser` and `extract`;
//! per-domain knowledge (such as framework conventions or naming patterns)
//! lives in the `squick-dict` crate.

pub mod error;
pub mod extract;
pub mod graph;
pub mod language;
pub mod manifest;
pub mod parser;
pub mod resolve;
pub mod scanner;
pub mod types;

pub use error::{Error, Result};
pub use graph::{CallGraph, EdgeKind};
pub use language::Language;
pub use scanner::{ScanOptions, Scanner};
pub use types::{
    CallKind, CallSite, Comment, Confidence, Endpoint, EndpointSource, FileSummary, HttpMethod,
    Manifest, ManifestKind, Project, Reference, SemanticTag, StrapiAttribute, StrapiSchema,
    Symbol, SymbolKind, TagSource,
};
