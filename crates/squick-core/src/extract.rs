// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Tree-sitter AST to [`FileSummary`] extraction.
//!
//! A single recursive walker handles every supported language. Per-language
//! constructs are dispatched by node kind so that the language-by-construct
//! matrix stays in one place.

use crate::language::Language;
use crate::types::{
    CallKind, CallSite, Comment, Confidence, Endpoint, EndpointSource, FileSummary, HttpMethod,
    SemanticTag, Symbol, SymbolKind, TagSource,
};
use std::collections::HashSet;
use tree_sitter::{Node, TreeCursor};

pub fn extract(language: Language, root: Node<'_>, source: &[u8], out: &mut FileSummary) {
    let mut ctx = Ctx {
        language,
        source,
        comments: Vec::new(),
        jsx_tags: HashSet::new(),
        in_class: false,
    };
    walk(&mut ctx, root, out);
    attach_doc_comments(&ctx.comments, out);
    add_jsx_tags(&ctx.jsx_tags, out);
    detect_nextjs_route_handlers(out);
}

struct Ctx<'a> {
    language: Language,
    source: &'a [u8],
    comments: Vec<Comment>,
    jsx_tags: HashSet<String>,
    in_class: bool,
}

fn walk(ctx: &mut Ctx<'_>, node: Node<'_>, out: &mut FileSummary) {
    match node.kind() {
        "comment" => {
            if let Some(c) = make_comment(node, ctx.source) {
                ctx.comments.push(c);
            }
        }

        "import_statement" => {
            if let Some(spec) = extract_import_specifier(ctx.language, node, ctx.source) {
                out.imports.push(spec);
            }
        }
        "import_from_statement" if ctx.language == Language::Python => {
            if let Some(child) = node.child_by_field_name("module_name") {
                out.imports.push(text(child, ctx.source).to_string());
            }
        }

        "function_declaration" | "function_definition" => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                let kind = if ctx.in_class {
                    SymbolKind::Method
                } else {
                    SymbolKind::Function
                };
                out.symbols.push(make_symbol(name, kind, node));
            }
        }
        "method_definition" | "method_signature" => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols.push(make_symbol(name, SymbolKind::Method, node));
            }
        }

        "class_declaration" | "class_definition" => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols.push(make_symbol(name, SymbolKind::Class, node));
            }
            let was_in_class = ctx.in_class;
            ctx.in_class = true;
            recurse(ctx, node, out);
            ctx.in_class = was_in_class;
            return;
        }
        "interface_declaration" => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols.push(make_symbol(name, SymbolKind::Interface, node));
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols.push(make_symbol(name, SymbolKind::TypeAlias, node));
            }
        }

        "variable_declarator" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Some(value) = node.child_by_field_name("value") {
                    let is_function = matches!(
                        value.kind(),
                        "arrow_function" | "function_expression" | "function"
                    );
                    if is_function {
                        let name = text(name_node, ctx.source).to_string();
                        out.symbols
                            .push(make_symbol(name, SymbolKind::Function, name_node));
                    }
                }
            }
        }

        "jsx_opening_element" | "jsx_self_closing_element" => {
            if let Some(name) = node.child_by_field_name("name") {
                let t = text(name, ctx.source).to_string();
                if !t.is_empty() && is_jsx_tag_worth_keeping(&t) {
                    ctx.jsx_tags.insert(t.clone());
                    if is_component_name(&t) {
                        out.call_sites.push(CallSite {
                            name: rightmost_segment(&t),
                            line: name.start_position().row + 1,
                            kind: CallKind::JsxComponent,
                        });
                    }
                }
            }
        }

        "call_expression" => {
            if let Some(callee) = node.child_by_field_name("function") {
                if is_free_callee(callee) {
                    if let Some(name) = extract_callee_name(callee, ctx.source) {
                        out.call_sites.push(CallSite {
                            name,
                            line: callee.start_position().row + 1,
                            kind: CallKind::Call,
                        });
                    }
                } else if callee.kind() == "member_expression" {
                    if let Some(ep) = extract_js_method_call_endpoint(node, callee, ctx.source) {
                        out.endpoints.push(ep);
                    }
                }
            }
        }
        "new_expression" => {
            if let Some(constructor) = node.child_by_field_name("constructor") {
                if is_free_callee(constructor) {
                    if let Some(name) = extract_callee_name(constructor, ctx.source) {
                        out.call_sites.push(CallSite {
                            name,
                            line: constructor.start_position().row + 1,
                            kind: CallKind::New,
                        });
                    }
                }
            }
        }
        "call" if ctx.language == Language::Python => {
            if let Some(func) = node.child_by_field_name("function") {
                if is_free_callee(func) {
                    if let Some(name) = extract_callee_name(func, ctx.source) {
                        if matches!(name.as_str(), "path" | "url" | "re_path") {
                            if let Some(ep) = extract_django_urlpattern(node, ctx.source) {
                                out.endpoints.push(ep);
                            }
                        }
                        out.call_sites.push(CallSite {
                            name,
                            line: func.start_position().row + 1,
                            kind: CallKind::Call,
                        });
                    }
                }
            }
        }
        "decorated_definition" if ctx.language == Language::Python => {
            extract_python_decorated_endpoints(node, ctx.source, out);
            recurse(ctx, node, out);
            return;
        }

        _ => {}
    }

    recurse(ctx, node, out);
}

fn recurse(ctx: &mut Ctx<'_>, node: Node<'_>, out: &mut FileSummary) {
    let mut cursor: TreeCursor<'_> = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk(ctx, cursor.node(), out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn make_symbol(name: String, kind: SymbolKind, node: Node<'_>) -> Symbol {
    let pos = node.start_position();
    Symbol {
        name,
        kind,
        file: std::path::PathBuf::new(),
        line: pos.row + 1,
        column: pos.column + 1,
        doc_comment: None,
        inline_comments: Vec::new(),
        references: Vec::new(),
        semantic_tags: Vec::new(),
        confidence: Confidence::High,
    }
}

fn make_comment(node: Node<'_>, source: &[u8]) -> Option<Comment> {
    let raw = text(node, source).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    Some(Comment {
        line: node.start_position().row + 1,
        text: raw,
        is_doc: false,
    })
}

fn extract_import_specifier(language: Language, node: Node<'_>, source: &[u8]) -> Option<String> {
    match language {
        Language::Python => node
            .named_child(0)
            .map(|c| text(c, source).to_string()),
        _ => {
            let s = node.child_by_field_name("source")?;
            let raw = text(s, source);
            Some(raw.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string())
        }
    }
}

fn attach_doc_comments(comments: &[Comment], out: &mut FileSummary) {
    if comments.is_empty() {
        return;
    }
    for sym in out.symbols.iter_mut() {
        let target_line = sym.line.saturating_sub(1);
        if target_line == 0 {
            continue;
        }
        if let Some(c) = comments.iter().find(|c| c.line == target_line) {
            sym.doc_comment = Some(c.text.clone());
        }
    }
}

fn add_jsx_tags(jsx_tags: &HashSet<String>, out: &mut FileSummary) {
    for tag in jsx_tags {
        out.semantic_tags.push(SemanticTag {
            label: format!("jsx:{tag}"),
            source: TagSource::Heuristic {
                rule: "jsx-tag".to_string(),
            },
            confidence: Confidence::Medium,
        });
    }
}

fn field_text(node: Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| text(n, source).to_string())
}

fn text<'s>(node: Node<'_>, source: &'s [u8]) -> &'s str {
    node.utf8_text(source).unwrap_or("")
}

/// Returns the rightmost identifier of a callee expression, walking
/// `member_expression` (JS/TS) or `attribute` (Python) chains.
fn extract_callee_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "property_identifier" | "private_property_identifier" => {
            Some(text(node, source).to_string())
        }
        "member_expression" => node
            .child_by_field_name("property")
            .and_then(|p| extract_callee_name(p, source)),
        "attribute" => node
            .child_by_field_name("attribute")
            .and_then(|p| extract_callee_name(p, source)),
        _ => None,
    }
}

fn is_component_name(name: &str) -> bool {
    rightmost_segment(name)
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
}

/// Keeps PascalCase JSX components and semantic HTML5 region tags;
/// drops generic container tags whose presence carries no useful signal.
fn is_jsx_tag_worth_keeping(name: &str) -> bool {
    if is_component_name(name) {
        return true;
    }
    const SEMANTIC_TAGS: &[&str] = &[
        "nav",
        "header",
        "footer",
        "main",
        "aside",
        "article",
        "section",
        "form",
        "dialog",
        "summary",
        "details",
        "address",
        "blockquote",
        "figure",
        "figcaption",
        "hgroup",
        "mark",
        "time",
    ];
    SEMANTIC_TAGS.contains(&name)
}

/// True when the callee is a bare identifier (free function call) rather
/// than a member or attribute access. Member calls require type-aware
/// resolution that the name-based resolver cannot provide, so they are
/// excluded from the call graph.
fn is_free_callee(node: Node<'_>) -> bool {
    matches!(node.kind(), "identifier")
}

fn string_literal_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "string" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let kind = child.kind();
                if kind == "string_content" || kind == "string_fragment" {
                    return Some(text(child, source).to_string());
                }
            }
            let raw = text(node, source);
            Some(raw.trim_matches(|c| c == '"' || c == '\'').to_string())
        }
        "template_string" => {
            let raw = text(node, source);
            Some(raw.trim_matches('`').to_string())
        }
        _ => None,
    }
}

fn first_named_arg<'tree>(call: Node<'tree>) -> Option<Node<'tree>> {
    call.child_by_field_name("arguments")?.named_child(0)
}

fn nth_named_arg<'tree>(call: Node<'tree>, n: usize) -> Option<Node<'tree>> {
    call.child_by_field_name("arguments")?.named_child(n)
}

/// Extracts `app.get("/path", handler)` and similar Express/Koa/Fastify
/// style endpoint declarations.
fn extract_js_method_call_endpoint(
    call: Node<'_>,
    callee: Node<'_>,
    source: &[u8],
) -> Option<Endpoint> {
    let property = callee.child_by_field_name("property")?;
    let method_name = text(property, source);
    let method = HttpMethod::from_token(method_name)?;
    let first_arg = first_named_arg(call)?;
    let path = string_literal_value(first_arg, source)?;
    if !path.starts_with('/') && !path.starts_with(':') {
        return None;
    }
    let handler = nth_named_arg(call, 1).and_then(|n| match n.kind() {
        "identifier" => Some(text(n, source).to_string()),
        _ => None,
    });
    Some(Endpoint {
        method,
        path,
        handler,
        line: call.start_position().row + 1,
        source: EndpointSource::JsMethodCall,
    })
}

/// Extracts `path("about/", views.about)` style Django urlpatterns entries.
fn extract_django_urlpattern(call: Node<'_>, source: &[u8]) -> Option<Endpoint> {
    let first_arg = first_named_arg(call)?;
    let raw_path = string_literal_value(first_arg, source)?;
    let path = normalize_django_path(&raw_path);
    let handler = nth_named_arg(call, 1).map(|n| text(n, source).to_string());
    Some(Endpoint {
        method: HttpMethod::Any,
        path,
        handler,
        line: call.start_position().row + 1,
        source: EndpointSource::PythonUrlpatterns,
    })
}

/// Recognizes Next.js App Router `route.{ts,js,tsx,jsx}` files and, when
/// they export uppercase HTTP-method symbols, records each as an
/// endpoint whose path is derived from the directory layout under
/// `app/`. Catch-all and dynamic segments are translated to the
/// `:param` / `*param` form for readability.
fn detect_nextjs_route_handlers(out: &mut FileSummary) {
    let Some(filename) = out.path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    if !is_nextjs_route_file(filename) {
        return;
    }
    let Some(route_path) = nextjs_route_path(&out.path) else {
        return;
    };
    let candidates: Vec<(HttpMethod, String, usize)> = out
        .symbols
        .iter()
        .filter_map(|s| {
            nextjs_method_from_name(&s.name).map(|m| (m, s.name.clone(), s.line))
        })
        .collect();
    for (method, handler, line) in candidates {
        out.endpoints.push(Endpoint {
            method,
            path: route_path.clone(),
            handler: Some(handler),
            line,
            source: EndpointSource::NextjsRouteHandler,
        });
    }
}

fn is_nextjs_route_file(filename: &str) -> bool {
    matches!(
        filename,
        "route.ts" | "route.tsx" | "route.js" | "route.jsx" | "route.mjs"
    )
}

fn nextjs_method_from_name(name: &str) -> Option<HttpMethod> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    HttpMethod::from_token(name)
}

fn nextjs_route_path(path: &std::path::Path) -> Option<String> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let after_app = normalized.rfind("/app/").map(|i| &normalized[i + 5..])?;
    let directory = after_app.rsplit_once('/').map(|(prefix, _)| prefix).unwrap_or("");
    let segments: Vec<String> = directory
        .split('/')
        .filter(|seg| !seg.is_empty())
        .filter(|seg| !(seg.starts_with('(') && seg.ends_with(')')))
        .map(translate_nextjs_segment)
        .collect();
    if segments.is_empty() {
        Some("/".to_string())
    } else {
        Some(format!("/{}", segments.join("/")))
    }
}

fn translate_nextjs_segment(segment: &str) -> String {
    if let Some(inner) = segment
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
    {
        if let Some(rest) = inner.strip_prefix("...") {
            return format!("*{rest}");
        }
        return format!(":{inner}");
    }
    segment.to_string()
}

fn normalize_django_path(raw: &str) -> String {
    if raw.is_empty() {
        return "/".to_string();
    }
    if is_regex_path(raw) || raw.starts_with('/') {
        return raw.to_string();
    }
    format!("/{raw}")
}

/// `re_path()` accepts a regular expression rather than a path literal.
/// Heuristic: a regex usually anchors with `^` or contains escaped
/// metacharacters that wouldn't appear in a real URL.
fn is_regex_path(raw: &str) -> bool {
    raw.starts_with('^')
        || raw.contains("\\d")
        || raw.contains("\\w")
        || raw.contains("(?P<")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn nextjs_route_path_simple() {
        let p = PathBuf::from("frontend/src/app/api/blogs/route.ts");
        assert_eq!(nextjs_route_path(&p), Some("/api/blogs".to_string()));
    }

    #[test]
    fn nextjs_route_path_dynamic_segment() {
        let p = PathBuf::from("frontend/src/app/api/users/[id]/route.ts");
        assert_eq!(nextjs_route_path(&p), Some("/api/users/:id".to_string()));
    }

    #[test]
    fn nextjs_route_path_catchall() {
        let p = PathBuf::from("frontend/src/app/data/[...slug]/route.ts");
        assert_eq!(nextjs_route_path(&p), Some("/data/*slug".to_string()));
    }

    #[test]
    fn nextjs_route_path_drops_route_group() {
        let p = PathBuf::from("src/app/(marketing)/about/route.ts");
        assert_eq!(nextjs_route_path(&p), Some("/about".to_string()));
    }

    #[test]
    fn nextjs_route_path_root() {
        let p = PathBuf::from("src/app/route.ts");
        assert_eq!(nextjs_route_path(&p), Some("/".to_string()));
    }

    #[test]
    fn nextjs_route_path_normalises_backslashes() {
        let p = PathBuf::from(r"src\app\api\users\route.ts");
        assert_eq!(nextjs_route_path(&p), Some("/api/users".to_string()));
    }

    #[test]
    fn nextjs_route_path_rejects_non_app_router() {
        let p = PathBuf::from("src/pages/api/users.ts");
        assert_eq!(nextjs_route_path(&p), None);
    }

    #[test]
    fn django_path_normalisation() {
        assert_eq!(normalize_django_path(""), "/");
        assert_eq!(normalize_django_path("about/"), "/about/");
        assert_eq!(normalize_django_path("/already-slashed"), "/already-slashed");
    }

    #[test]
    fn django_regex_paths_are_preserved() {
        let regex = r"^api/posts/(?P<slug>[\w-]+)/$";
        assert_eq!(normalize_django_path(regex), regex);
        assert!(is_regex_path(regex));
    }

    #[test]
    fn jsx_filter_keeps_components_and_semantic_tags() {
        assert!(is_jsx_tag_worth_keeping("Navbar"));
        assert!(is_jsx_tag_worth_keeping("MyComponent"));
        assert!(is_jsx_tag_worth_keeping("nav"));
        assert!(is_jsx_tag_worth_keeping("header"));
        assert!(is_jsx_tag_worth_keeping("main"));
    }

    #[test]
    fn jsx_filter_drops_generic_html() {
        assert!(!is_jsx_tag_worth_keeping("div"));
        assert!(!is_jsx_tag_worth_keeping("span"));
        assert!(!is_jsx_tag_worth_keeping("p"));
        assert!(!is_jsx_tag_worth_keeping("button"));
        assert!(!is_jsx_tag_worth_keeping("h1"));
    }

    #[test]
    fn nextjs_method_requires_uppercase_only() {
        assert_eq!(nextjs_method_from_name("GET"), Some(HttpMethod::Get));
        assert_eq!(nextjs_method_from_name("POST"), Some(HttpMethod::Post));
        assert_eq!(nextjs_method_from_name("Get"), None);
        assert_eq!(nextjs_method_from_name("get"), None);
        assert_eq!(nextjs_method_from_name("HANDLER"), None);
    }
}

/// Extracts FastAPI / Flask / NestJS-Python style decorator endpoints.
fn extract_python_decorated_endpoints(node: Node<'_>, source: &[u8], out: &mut FileSummary) {
    let mut handler_name: Option<String> = None;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "function_definition") {
            handler_name = field_text(child, "name", source);
            break;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "decorator" {
            continue;
        }
        let Some(call) = first_call_child(child) else {
            continue;
        };
        let Some((method, path)) = parse_python_route_call(call, source) else {
            continue;
        };
        out.endpoints.push(Endpoint {
            method,
            path,
            handler: handler_name.clone(),
            line: child.start_position().row + 1,
            source: EndpointSource::PythonDecorator,
        });
    }
}

fn first_call_child<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    for i in 0..node.named_child_count() {
        let child = node.named_child(i)?;
        if child.kind() == "call" {
            return Some(child);
        }
    }
    None
}

fn parse_python_route_call(call: Node<'_>, source: &[u8]) -> Option<(HttpMethod, String)> {
    let func = call.child_by_field_name("function")?;
    let method_token = match func.kind() {
        "attribute" => {
            let attr = func.child_by_field_name("attribute")?;
            text(attr, source).to_string()
        }
        "identifier" => text(func, source).to_string(),
        _ => return None,
    };
    let method = HttpMethod::from_token(&method_token)?;
    let first_arg = first_named_arg(call)?;
    let path = string_literal_value(first_arg, source)?;
    if !path.starts_with('/') {
        return None;
    }
    Some((method, path))
}

fn rightmost_segment(name: &str) -> String {
    name.rsplit('.').next().unwrap_or(name).to_string()
}
