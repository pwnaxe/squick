// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Pattern dictionary engine.
//!
//! Dictionaries are YAML files grouped by category (routes, components,
//! files, frameworks, naming). Each entry declares a pattern (literal,
//! glob, or regex), a target tag, and a confidence level. The matcher
//! applies them against `Symbol` and `FileSummary` values produced by
//! `squick-core`.

pub mod loader;
pub mod matcher;
pub mod types;

pub use loader::{load_directory, load_file};
pub use matcher::Matcher;
pub use types::{Dictionary, Entry, MatchKind, PatternKind};
