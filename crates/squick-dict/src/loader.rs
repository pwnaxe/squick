// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use crate::types::Dictionary;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn load_file(path: &Path) -> Result<Dictionary> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading dictionary {}", path.display()))?;
    let mut dict: Dictionary = serde_yaml_ng::from_str(&text)
        .with_context(|| format!("parsing dictionary {}", path.display()))?;
    if dict.name.is_empty() {
        dict.name = derive_name(path);
    }
    Ok(dict)
}

pub fn load_directory(dir: &Path) -> Result<Vec<Dictionary>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yaml" && ext != "yml" {
            continue;
        }
        out.push(load_file(p)?);
    }
    Ok(out)
}

fn derive_name(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut p = PathBuf::from(path);
    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
        parts.push(stem.to_string());
    }
    p.pop();
    if let Some(parent) = p.file_name().and_then(|s| s.to_str()) {
        parts.push(parent.to_string());
    }
    parts.reverse();
    parts.join("/")
}
