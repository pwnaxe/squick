// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use squick_core::Confidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatchKind {
    Route,
    Component,
    Filename,
    PathSegment,
    SymbolName,
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternKind {
    /// Case-insensitive literal match.
    Literal,
    /// Glob with `*` and `?` wildcards.
    Glob,
    /// Rust-syntax regular expression.
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub pattern: String,
    #[serde(default = "default_pattern_kind")]
    pub kind: PatternKind,
    pub r#match: MatchKind,
    pub tag: String,
    #[serde(default = "default_confidence")]
    pub confidence: Confidence,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_pattern_kind() -> PatternKind {
    PatternKind::Literal
}

fn default_confidence() -> Confidence {
    Confidence::Medium
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dictionary {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub entries: Vec<Entry>,
}
