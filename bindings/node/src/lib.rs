// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

#![deny(clippy::all)]

use napi_derive::napi;
use squick_core::{ScanOptions, Scanner};
use std::path::PathBuf;

#[napi(object)]
pub struct ScanResult {
    pub root: String,
    pub file_count: u32,
    pub markdown: String,
}

#[napi]
pub fn scan(root: String) -> napi::Result<ScanResult> {
    let mut scanner = Scanner::new(ScanOptions::default());
    let project = scanner
        .scan_project(&PathBuf::from(&root))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    let markdown = squick_format::format_markdown(&project);
    Ok(ScanResult {
        root,
        file_count: project.files.len() as u32,
        markdown,
    })
}
