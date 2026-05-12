// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use crate::error::{Error, Result};
use crate::language::Language;
use crate::manifest;
use crate::parser::FileParser;
use crate::resolve::resolve_references;
use crate::types::{FileSummary, Project};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Respect `.gitignore`, `.ignore`, and global ignore files.
    pub respect_ignore: bool,
    /// Follow symlinks during traversal.
    pub follow_symlinks: bool,
    /// Maximum file size in bytes; larger files are skipped.
    pub max_file_bytes: u64,
    /// Glob patterns of paths to include. When non-empty, only matching
    /// paths are scanned.
    pub includes: Vec<String>,
    /// Glob patterns of paths to exclude. Always applied.
    pub excludes: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            respect_ignore: true,
            follow_symlinks: false,
            max_file_bytes: 2 * 1024 * 1024,
            includes: Vec::new(),
            excludes: Vec::new(),
        }
    }
}

pub struct Scanner {
    options: ScanOptions,
    parsers: HashMap<Language, FileParser>,
}

impl Scanner {
    pub fn new(options: ScanOptions) -> Self {
        Self {
            options,
            parsers: HashMap::new(),
        }
    }

    pub fn scan_project(&mut self, root: &Path) -> Result<Project> {
        let mut project = Project {
            root: root.to_path_buf(),
            ..Default::default()
        };

        let mut builder = WalkBuilder::new(root);
        builder
            .standard_filters(self.options.respect_ignore)
            .follow_links(self.options.follow_symlinks);
        if let Some(overrides) = self.build_overrides(root)? {
            builder.overrides(overrides);
        }
        let walker = builder.build();

        for entry in walker {
            let entry = entry.map_err(Error::Walk)?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let Some(language) = Language::from_path(path) else {
                continue;
            };
            if let Ok(meta) = path.metadata() {
                if meta.len() > self.options.max_file_bytes {
                    continue;
                }
            }
            match self.scan_file(path, language) {
                Ok(summary) => project.files.push(summary),
                Err(e) => {
                    eprintln!("squick: skip {}: {e}", path.display());
                }
            }
        }

        project.files.sort_by(|a, b| a.path.cmp(&b.path));
        manifest::scan(&mut project, self.options.respect_ignore);
        project.manifests.sort_by(|a, b| a.path.cmp(&b.path));
        project.strapi_schemas.sort_by(|a, b| a.path.cmp(&b.path));
        resolve_references(&mut project);

        Ok(project)
    }

    fn build_overrides(&self, root: &Path) -> Result<Option<ignore::overrides::Override>> {
        if self.options.includes.is_empty() && self.options.excludes.is_empty() {
            return Ok(None);
        }
        let mut builder = OverrideBuilder::new(root);
        for pattern in &self.options.includes {
            builder
                .add(pattern)
                .map_err(|e| Error::Other(format!("invalid include pattern {pattern}: {e}")))?;
        }
        for pattern in &self.options.excludes {
            let negated = format!("!{pattern}");
            builder
                .add(&negated)
                .map_err(|e| Error::Other(format!("invalid exclude pattern {pattern}: {e}")))?;
        }
        let overrides = builder
            .build()
            .map_err(|e| Error::Other(format!("override build failed: {e}")))?;
        Ok(Some(overrides))
    }

    pub fn scan_file(&mut self, path: &Path, language: Language) -> Result<FileSummary> {
        let source = std::fs::read_to_string(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parser = match self.parsers.get_mut(&language) {
            Some(p) => p,
            None => {
                let p = FileParser::for_language(language)?;
                self.parsers.entry(language).or_insert(p)
            }
        };
        parser.parse_source(path, &source)
    }
}
