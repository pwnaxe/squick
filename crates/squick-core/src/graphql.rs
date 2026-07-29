// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Detects `.graphql`/`.gql` SDL schema files and extracts their type
//! declarations (`type`/`input`/`interface`/`union`/`enum`/`scalar`) and
//! the root `Query`/`Mutation`/`Subscription` field names as operations.
//! This is a schema extractor, not a client query-document parser: `.graphql`
//! files holding operations/fragments rather than a schema simply yield no
//! declarations. Hand-rolled rather than pulled in as a dependency, matching
//! how `docker.rs` hand-rolls Dockerfile/Compose parsing rather than using a
//! parser crate. Results land on `Project.graphql`.

use crate::types::{GraphqlArtifact, GraphqlField, GraphqlType, GraphqlTypeKind, Project};
use ignore::WalkBuilder;
use regex::Regex;
use std::path::Path;

pub fn scan(project: &mut Project, respect_ignore: bool) {
    let walker = WalkBuilder::new(&project.root)
        .standard_filters(respect_ignore)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !is_graphql_schema(path) {
            continue;
        }
        if let Some(artifact) = parse_schema(path) {
            project.graphql.push(artifact);
        }
    }
}

fn is_graphql_schema(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("graphql" | "gql")
    )
}

fn parse_schema(path: &Path) -> Option<GraphqlArtifact> {
    let text = std::fs::read_to_string(path).ok()?;
    let clean = strip_comments_and_descriptions(&text);
    let types = parse_declarations(&clean);
    if types.is_empty() {
        return None;
    }

    let queries = root_field_names(&types, "Query");
    let mutations = root_field_names(&types, "Mutation");
    let subscriptions = root_field_names(&types, "Subscription");

    Some(GraphqlArtifact {
        path: path.to_path_buf(),
        types,
        queries,
        mutations,
        subscriptions,
    })
}

fn root_field_names(types: &[GraphqlType], root_name: &str) -> Vec<String> {
    types
        .iter()
        .find(|t| t.name == root_name)
        .map(|t| t.fields.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default()
}

/// Removes `#`-line-comments and `"..."`/`"""..."""` descriptions so they
/// can't be mistaken for schema syntax. Iterates over `char`s rather than
/// bytes: a byte-indexed version that assumes non-ASCII text can only ever
/// appear inside a string or comment does not hold for malformed/unusual
/// input (e.g. a `//`-style comment, or a stray non-ASCII byte outside any
/// string), and slicing `text[i..]` at a byte offset that lands mid-codepoint
/// panics — this must not crash the whole scan over one odd file.
fn strip_comments_and_descriptions(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' => {
                while let Some(&next) = chars.peek() {
                    if next == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '"' => {
                if chars.peek() == Some(&'"') {
                    chars.next(); // second quote
                    if chars.peek() == Some(&'"') {
                        chars.next(); // third quote: a genuine `"""` block
                        let mut closing_run = 0;
                        loop {
                            match chars.next() {
                                None => break,
                                Some('"') => {
                                    closing_run += 1;
                                    if closing_run == 3 {
                                        break;
                                    }
                                }
                                Some(_) => closing_run = 0,
                            }
                        }
                    }
                    // Otherwise this was `""`: an empty single-line description.
                } else {
                    loop {
                        match chars.next() {
                            None | Some('"') => break,
                            Some('\\') => {
                                chars.next();
                            }
                            Some(_) => {}
                        }
                    }
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

fn parse_declarations(clean: &str) -> Vec<GraphqlType> {
    let decl_re =
        Regex::new(r"(?m)^\s*(?:extend\s+)?(type|input|interface|enum|union|scalar)\s+(\w+)")
            .expect("valid regex");
    let mut types = Vec::new();
    let mut search_from = 0;

    while let Some(m) = decl_re.captures(&clean[search_from..]) {
        let whole = m.get(0).unwrap();
        let kind_str = &m[1];
        let name = m[2].to_string();
        let after_name = search_from + whole.end();
        let kind = match kind_str {
            "type" => GraphqlTypeKind::Object,
            "input" => GraphqlTypeKind::Input,
            "interface" => GraphqlTypeKind::Interface,
            "union" => GraphqlTypeKind::Union,
            "enum" => GraphqlTypeKind::Enum,
            "scalar" => GraphqlTypeKind::Scalar,
            _ => unreachable!("regex only matches the six keywords above"),
        };

        let (fields, next_pos) = match kind {
            GraphqlTypeKind::Scalar => (Vec::new(), after_name),
            GraphqlTypeKind::Union => parse_union(clean, after_name),
            GraphqlTypeKind::Enum => parse_braced(clean, after_name, parse_enum_values),
            _ => parse_braced(clean, after_name, parse_fields),
        };

        types.push(GraphqlType { name, kind, fields });
        search_from = next_pos.max(after_name + 1).min(clean.len());
    }

    types
}

/// `union Name = A | B | C`: the member list runs to the next newline.
fn parse_union(clean: &str, from: usize) -> (Vec<GraphqlField>, usize) {
    let rest = &clean[from..];
    let Some(eq) = rest.find('=') else {
        return (Vec::new(), from);
    };
    let after_eq = &rest[eq + 1..];
    let line_end = after_eq.find('\n').unwrap_or(after_eq.len());
    let members = &after_eq[..line_end];
    let fields = members
        .split('|')
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(|m| GraphqlField {
            name: m.to_string(),
            field_type: "union-member".to_string(),
            required: false,
        })
        .collect();
    (fields, from + eq + 1 + line_end)
}

/// Finds the `{ ... }` block starting at or after `from`, tracking brace
/// depth so nested braces (e.g. in a default-value object literal) don't
/// end the block early, then hands the body to `parse_body`.
fn parse_braced(
    clean: &str,
    from: usize,
    parse_body: fn(&str) -> Vec<GraphqlField>,
) -> (Vec<GraphqlField>, usize) {
    let rest = &clean[from..];
    let Some(open_rel) = rest.find('{') else {
        return (Vec::new(), from);
    };
    let open = from + open_rel;
    let mut depth = 0usize;
    let mut close = None;
    for (offset, ch) in clean[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return (Vec::new(), clean.len());
    };
    let body = &clean[open + 1..close];
    (parse_body(body), close + 1)
}

fn parse_fields(body: &str) -> Vec<GraphqlField> {
    let field_re =
        Regex::new(r"^\s*(\w+)\s*(?:\([^)]*\))?\s*:\s*([\[\]!\w]+)").expect("valid regex");
    body.lines()
        .filter_map(|line| {
            let caps = field_re.captures(line)?;
            let name = caps[1].to_string();
            let raw_type = &caps[2];
            let required = raw_type.ends_with('!');
            let field_type = raw_type
                .chars()
                .filter(|c| !matches!(c, '[' | ']' | '!'))
                .collect();
            Some(GraphqlField {
                name,
                field_type,
                required,
            })
        })
        .collect()
}

fn parse_enum_values(body: &str) -> Vec<GraphqlField> {
    let value_re = Regex::new(r"^\s*(\w+)\s*$").expect("valid regex");
    body.lines()
        .filter_map(|line| {
            let caps = value_re.captures(line)?;
            Some(GraphqlField {
                name: caps[1].to_string(),
                field_type: "enum-value".to_string(),
                required: false,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn strips_comments_and_descriptions() {
        let src = "# a comment\n\"\"\"\nA description\nspanning lines\n\"\"\"\ntype User {\n  id: ID! # inline\n}\n";
        let clean = strip_comments_and_descriptions(src);
        assert!(!clean.contains('#'));
        assert!(!clean.contains("description"));
        assert!(clean.contains("type User"));
    }

    #[test]
    fn parses_object_type_fields_with_required_and_list() {
        let src = r#"
type User {
    id: ID!
    name: String
    tags: [String!]!
}
"#;
        let types = parse_declarations(&strip_comments_and_descriptions(src));
        assert_eq!(types.len(), 1);
        let user = &types[0];
        assert_eq!(user.kind, GraphqlTypeKind::Object);
        let id = user.fields.iter().find(|f| f.name == "id").unwrap();
        assert!(id.required);
        assert_eq!(id.field_type, "ID");
        let tags = user.fields.iter().find(|f| f.name == "tags").unwrap();
        assert!(tags.required);
        assert_eq!(tags.field_type, "String");
    }

    #[test]
    fn parses_enum_union_and_scalar() {
        let src = r#"
scalar DateTime

enum Status {
    ACTIVE
    INACTIVE
}

union SearchResult = Book | Movie
"#;
        let types = parse_declarations(&strip_comments_and_descriptions(src));
        assert_eq!(types.len(), 3);
        assert_eq!(types[0].kind, GraphqlTypeKind::Scalar);
        assert!(types[0].fields.is_empty());
        assert_eq!(types[1].kind, GraphqlTypeKind::Enum);
        assert_eq!(types[1].fields.len(), 2);
        assert_eq!(types[2].kind, GraphqlTypeKind::Union);
        let members: Vec<_> = types[2].fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(members, vec!["Book", "Movie"]);
    }

    #[test]
    fn collects_root_operation_fields() {
        let src = r#"
type Query {
    users: [User!]!
    user(id: ID!): User
}

type Mutation {
    createUser(name: String!): User!
}

type User {
    id: ID!
}
"#;
        let types = parse_declarations(&strip_comments_and_descriptions(src));
        let queries = root_field_names(&types, "Query");
        assert_eq!(queries, vec!["users", "user"]);
        let mutations = root_field_names(&types, "Mutation");
        assert_eq!(mutations, vec!["createUser"]);
    }

    #[test]
    fn non_ascii_in_valid_comments_and_descriptions_is_stripped_not_corrupted() {
        let src =
            "# zażółć gęślą jaźń\n\"\"\"Zwraca użytkownika.\"\"\"\ntype User {\n  id: ID!\n}\n";
        let clean = strip_comments_and_descriptions(src);
        assert!(!clean.contains("zażółć"));
        assert!(!clean.contains("użytkownika"));
        let types = parse_declarations(&clean);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "User");
    }

    #[test]
    fn truncated_declaration_does_not_hang() {
        let src = "type Foo"; // no brace, no body, EOF right after the name
        let start = Instant::now();
        let types = parse_declarations(&strip_comments_and_descriptions(src));
        assert!(start.elapsed() < Duration::from_secs(1), "must not hang");
        assert!(types.is_empty() || types.len() == 1);
    }

    #[test]
    fn multiple_truncated_declarations_do_not_hang() {
        let src = "type A\ntype B\ninput C\nenum D\nunion E\nscalar F";
        let start = Instant::now();
        let _ = parse_declarations(&strip_comments_and_descriptions(src));
        assert!(start.elapsed() < Duration::from_secs(1), "must not hang");
    }

    #[test]
    fn stray_non_ascii_outside_strings_does_not_panic() {
        // Malformed on purpose: a stray non-ASCII byte sequence outside any
        // string/comment/description.
        let src = "type Foo { id: ID! }\n// zażółć\ntype Bar { x: Int }\n";
        let clean = strip_comments_and_descriptions(src);
        let types = parse_declarations(&clean);
        assert!(types.iter().any(|t| t.name == "Foo"));
        assert!(types.iter().any(|t| t.name == "Bar"));
    }
}
