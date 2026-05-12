// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use crate::types::{Dictionary, Entry, MatchKind, PatternKind};
use regex::Regex;
use squick_core::{Project, SemanticTag, TagSource};

pub struct Matcher {
    compiled: Vec<CompiledEntry>,
}

struct CompiledEntry {
    dict_name: String,
    entry: Entry,
    regex: Option<Regex>,
}

impl Matcher {
    pub fn from_dictionaries(dicts: impl IntoIterator<Item = Dictionary>) -> Self {
        let mut compiled = Vec::new();
        for dict in dicts {
            for entry in dict.entries.iter() {
                let regex = match entry.kind {
                    PatternKind::Regex => Regex::new(&entry.pattern).ok(),
                    PatternKind::Glob => Regex::new(&glob_to_regex(&entry.pattern)).ok(),
                    PatternKind::Literal => None,
                };
                compiled.push(CompiledEntry {
                    dict_name: dict.name.clone(),
                    entry: entry.clone(),
                    regex,
                });
            }
        }
        Self { compiled }
    }

    pub fn match_value(&self, surface: MatchKind, value: &str) -> Vec<SemanticTag> {
        let mut out = Vec::new();
        for c in &self.compiled {
            if c.entry.r#match != surface {
                continue;
            }
            let hit = match c.entry.kind {
                PatternKind::Literal => value.eq_ignore_ascii_case(&c.entry.pattern),
                PatternKind::Glob | PatternKind::Regex => {
                    c.regex.as_ref().is_some_and(|r| r.is_match(value))
                }
            };
            if !hit {
                continue;
            }
            out.push(SemanticTag {
                label: c.entry.tag.clone(),
                source: TagSource::Dictionary {
                    dict: c.dict_name.clone(),
                    entry: c.entry.pattern.clone(),
                },
                confidence: c.entry.confidence,
            });
        }
        out
    }

    /// Applies every dictionary entry to every file, path segment, import,
    /// and symbol name in the project, attaching matched tags in place.
    /// Duplicate tags (same label) are collapsed.
    pub fn apply(&self, project: &mut Project) {
        for file in project.files.iter_mut() {
            if let Some(filename) = file.path.file_name().and_then(|f| f.to_str()) {
                file.semantic_tags
                    .extend(self.match_value(MatchKind::Filename, filename));
            }
            for segment in file.path.iter().filter_map(|s| s.to_str()) {
                file.semantic_tags
                    .extend(self.match_value(MatchKind::PathSegment, segment));
            }
            for import in &file.imports {
                file.semantic_tags
                    .extend(self.match_value(MatchKind::Import, import));
            }
            dedup_tags(&mut file.semantic_tags);

            for symbol in file.symbols.iter_mut() {
                symbol
                    .semantic_tags
                    .extend(self.match_value(MatchKind::SymbolName, &symbol.name));
                if is_pascal_case(&symbol.name) {
                    symbol
                        .semantic_tags
                        .extend(self.match_value(MatchKind::Component, &symbol.name));
                }
                dedup_tags(&mut symbol.semantic_tags);
            }
        }
    }
}

fn dedup_tags(tags: &mut Vec<SemanticTag>) {
    let mut seen = std::collections::HashSet::new();
    tags.retain(|t| seen.insert(t.label.clone()));
}

fn is_pascal_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => chars.all(|c| c.is_ascii_alphanumeric() || c == '_'),
        _ => false,
    }
}

fn glob_to_regex(glob: &str) -> String {
    let mut re = String::from("^");
    for ch in glob.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\' => {
                re.push('\\');
                re.push(ch);
            }
            _ => re.push(ch),
        }
    }
    re.push('$');
    re
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Entry;
    use squick_core::Confidence;

    #[test]
    fn glob_translates_wildcards_and_escapes() {
        assert_eq!(glob_to_regex("*.test.ts"), r"^.*\.test\.ts$");
        assert_eq!(glob_to_regex("models.py"), "^models\\.py$");
        assert_eq!(glob_to_regex("a?c"), "^a.c$");
    }

    fn make_dict(pattern: &str, kind: PatternKind, surface: MatchKind, tag: &str) -> Dictionary {
        Dictionary {
            name: "test".to_string(),
            description: None,
            entries: vec![Entry {
                pattern: pattern.to_string(),
                kind,
                r#match: surface,
                tag: tag.to_string(),
                confidence: Confidence::High,
                note: None,
            }],
        }
    }

    #[test]
    fn literal_match_is_case_insensitive() {
        let dict = make_dict("Models.py", PatternKind::Literal, MatchKind::Filename, "models");
        let matcher = Matcher::from_dictionaries([dict]);
        let tags = matcher.match_value(MatchKind::Filename, "models.py");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].label, "models");
    }

    #[test]
    fn glob_match_anchored() {
        let dict = make_dict("*.test.ts", PatternKind::Glob, MatchKind::Filename, "unit-test");
        let matcher = Matcher::from_dictionaries([dict]);
        assert_eq!(
            matcher.match_value(MatchKind::Filename, "foo.test.ts").len(),
            1
        );
        assert_eq!(
            matcher.match_value(MatchKind::Filename, "foo.test.tsx").len(),
            0
        );
    }

    #[test]
    fn regex_match_with_capture_pattern() {
        let dict = make_dict(
            "^use[A-Z].*$",
            PatternKind::Regex,
            MatchKind::SymbolName,
            "react-hook",
        );
        let matcher = Matcher::from_dictionaries([dict]);
        assert_eq!(matcher.match_value(MatchKind::SymbolName, "useState").len(), 1);
        assert_eq!(matcher.match_value(MatchKind::SymbolName, "user").len(), 0);
    }

    #[test]
    fn surface_isolation() {
        let dict = make_dict("react", PatternKind::Literal, MatchKind::Import, "framework-react");
        let matcher = Matcher::from_dictionaries([dict]);
        assert_eq!(matcher.match_value(MatchKind::Import, "react").len(), 1);
        assert_eq!(matcher.match_value(MatchKind::Filename, "react").len(), 0);
    }

    #[test]
    fn pascal_case_predicate() {
        assert!(is_pascal_case("Navbar"));
        assert!(is_pascal_case("UserAccount"));
        assert!(!is_pascal_case("navbar"));
        assert!(!is_pascal_case("user_account"));
        assert!(!is_pascal_case(""));
    }
}
