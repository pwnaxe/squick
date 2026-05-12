// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unsupported language for path {0:?}")]
    UnsupportedLanguage(PathBuf),

    #[error("tree-sitter language load failed: {0}")]
    LanguageLoad(String),

    #[error("parse failed for {path:?}")]
    Parse { path: PathBuf },

    #[error("walk error: {0}")]
    Walk(#[from] ignore::Error),

    #[error("{0}")]
    Other(String),
}
