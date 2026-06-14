// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Scanner, parser, and extractor. Language-specific logic sits in
//! `parser`/`extract`; framework dictionaries live in `squick-dict`.

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
    Manifest, ManifestKind, Project, Reference, SemanticTag, StrapiAttribute, StrapiSchema, Symbol,
    SymbolKind, TagSource,
};
