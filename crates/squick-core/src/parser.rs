// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use crate::error::{Error, Result};
use crate::extract;
use crate::language::Language;
use crate::types::FileSummary;
use std::path::Path;
use tree_sitter::Parser;

pub struct FileParser {
    parser: Parser,
    language: Language,
}

impl FileParser {
    pub fn for_language(language: Language) -> Result<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&language.ts_language())
            .map_err(|e| Error::LanguageLoad(e.to_string()))?;
        Ok(Self { parser, language })
    }

    pub fn parse_source(&mut self, path: &Path, source: &str) -> Result<FileSummary> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| Error::Parse {
                path: path.to_path_buf(),
            })?;

        let mut summary = FileSummary {
            path: path.to_path_buf(),
            language: self.language,
            symbols: Vec::new(),
            imports: Vec::new(),
            semantic_tags: Vec::new(),
            endpoints: Vec::new(),
            line_count: source.lines().count(),
            call_sites: Vec::new(),
        };

        extract::extract(
            self.language,
            tree.root_node(),
            source.as_bytes(),
            &mut summary,
        );

        for sym in summary.symbols.iter_mut() {
            sym.file = path.to_path_buf();
        }

        Ok(summary)
    }
}
