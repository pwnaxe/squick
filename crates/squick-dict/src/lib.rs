// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! YAML pattern dictionaries. Each entry maps a pattern (literal / glob /
//! regex) on a given surface (filename, symbol, import, ...) to a tag with
//! a confidence level.

pub mod loader;
pub mod matcher;
pub mod types;

pub use loader::{load_directory, load_file};
pub use matcher::Matcher;
pub use types::{Dictionary, Entry, MatchKind, PatternKind};
