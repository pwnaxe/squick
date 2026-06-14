// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    TypeScript,
    Tsx,
    JavaScript,
    Jsx,
    Python,
    Php,
}

impl Language {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "ts" => Language::TypeScript,
            "tsx" => Language::Tsx,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "jsx" => Language::Jsx,
            "py" | "pyi" => Language::Python,
            "php" | "php3" | "php4" | "php5" | "phtml" => Language::Php,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
            Language::Tsx => "tsx",
            Language::JavaScript => "javascript",
            Language::Jsx => "jsx",
            Language::Python => "python",
            Language::Php => "php",
        }
    }

    pub fn ts_language(self) -> tree_sitter::Language {
        match self {
            Language::TypeScript => tree_sitter_typescript::language_typescript(),
            Language::Tsx => tree_sitter_typescript::language_tsx(),
            Language::JavaScript | Language::Jsx => tree_sitter_javascript::language(),
            Language::Python => tree_sitter_python::language(),
            Language::Php => tree_sitter_php::language_php(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn routes_known_extensions() {
        assert_eq!(
            Language::from_path(&PathBuf::from("foo.ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("foo.tsx")),
            Some(Language::Tsx)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("foo.JS")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("foo.mjs")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("foo.py")),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("foo.php")),
            Some(Language::Php)
        );
        assert_eq!(
            Language::from_path(&PathBuf::from("index.PHTML")),
            Some(Language::Php)
        );
    }

    #[test]
    fn ignores_unknown_extensions() {
        assert_eq!(Language::from_path(&PathBuf::from("foo.rs")), None);
        assert_eq!(Language::from_path(&PathBuf::from("README")), None);
        assert_eq!(Language::from_path(&PathBuf::from("foo")), None);
    }
}
