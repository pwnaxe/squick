// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Recursive walker that turns a Tree-sitter tree into a `FileSummary`.

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
                out.symbols
                    .push(make_symbol(name, SymbolKind::Method, node));
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
                out.symbols
                    .push(make_symbol(name, SymbolKind::Interface, node));
            }
        }
        "type_alias_declaration" => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols
                    .push(make_symbol(name, SymbolKind::TypeAlias, node));
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

        "method_declaration" if ctx.language == Language::Php => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols
                    .push(make_symbol(name.clone(), SymbolKind::Method, node));
                extract_php_attribute_routes(node, ctx.source, Some(name), out);
            }
        }
        "trait_declaration" if ctx.language == Language::Php => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols.push(make_symbol(name, SymbolKind::Trait, node));
            }
            let was_in_class = ctx.in_class;
            ctx.in_class = true;
            recurse(ctx, node, out);
            ctx.in_class = was_in_class;
            return;
        }
        "enum_declaration" if ctx.language == Language::Php => {
            if let Some(name) = field_text(node, "name", ctx.source) {
                out.symbols.push(make_symbol(name, SymbolKind::Enum, node));
            }
            let was_in_class = ctx.in_class;
            ctx.in_class = true;
            recurse(ctx, node, out);
            ctx.in_class = was_in_class;
            return;
        }
        "namespace_use_declaration" if ctx.language == Language::Php => {
            collect_php_use_imports(node, ctx.source, out);
        }
        "scoped_call_expression" if ctx.language == Language::Php => {
            if let Some(ep) = extract_php_scoped_route(node, ctx.source) {
                out.endpoints.push(ep);
            }
        }
        "member_call_expression" if ctx.language == Language::Php => {
            if let Some(ep) = extract_php_member_route(node, ctx.source) {
                out.endpoints.push(ep);
            }
        }
        "function_call_expression" if ctx.language == Language::Php => {
            if let Some(func) = node.child_by_field_name("function") {
                if func.kind() == "name" {
                    out.call_sites.push(CallSite {
                        name: text(func, ctx.source).to_string(),
                        line: func.start_position().row + 1,
                        kind: CallKind::Call,
                    });
                }
            }
        }
        "object_creation_expression" if ctx.language == Language::Php => {
            if let Some(name) = php_new_class_name(node, ctx.source) {
                out.call_sites.push(CallSite {
                    name,
                    line: node.start_position().row + 1,
                    kind: CallKind::New,
                });
            }
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
        Language::Python => node.named_child(0).map(|c| text(c, source).to_string()),
        _ => {
            let s = node.child_by_field_name("source")?;
            let raw = text(s, source);
            Some(
                raw.trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    .to_string(),
            )
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

// Rightmost identifier of `foo.bar.baz()` / `foo.bar.baz()` (Python).
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

// PascalCase components + a small HTML5 semantic whitelist. Skips noise
// like <div>, <span>, <p>.
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

// Only bare `foo()` calls; member calls need type info we don't have.
fn is_free_callee(node: Node<'_>) -> bool {
    matches!(node.kind(), "identifier")
}

fn string_literal_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "string" | "encapsed_string" => {
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

// Express / Koa / Fastify: `app.get("/path", handler)`.
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

// Django: `path("about/", views.about)`.
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

// Next.js App Router: `export async function GET(req)` etc. inside
// `app/.../route.ts`. The URL is reconstructed from the file path.
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
        .filter_map(|s| nextjs_method_from_name(&s.name).map(|m| (m, s.name.clone(), s.line)))
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
    let directory = after_app
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .unwrap_or("");
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
    if let Some(inner) = segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
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

// Heuristic for Django `re_path` arguments vs ordinary paths.
fn is_regex_path(raw: &str) -> bool {
    raw.starts_with('^') || raw.contains("\\d") || raw.contains("\\w") || raw.contains("(?P<")
}

// FastAPI / Flask / NestJS-Py decorators: `@app.get("/x")`, `@router.post(...)`.
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

// ---- PHP ----------------------------------------------------------------

// `use App\Models\User;` / `use Foo\Bar as Baz;`. Stores the fully
// qualified name so dictionaries can match on namespace prefixes.
fn collect_php_use_imports(node: Node<'_>, source: &[u8], out: &mut FileSummary) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "namespace_use_clause" {
            continue;
        }
        if let Some(path) = php_use_clause_path(child, source) {
            out.imports.push(path);
        }
    }
}

fn php_use_clause_path(clause: Node<'_>, source: &[u8]) -> Option<String> {
    let first = clause.named_child(0)?;
    match first.kind() {
        "qualified_name" | "name" => Some(text(first, source).to_string()),
        _ => None,
    }
}

fn php_new_class_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "name" | "qualified_name") {
            return Some(php_rightmost(text(child, source)));
        }
    }
    None
}

fn php_rightmost(name: &str) -> String {
    name.rsplit('\\').next().unwrap_or(name).to_string()
}

// Laravel / Slim facade routes: `Route::get('/users', [Ctrl::class, 'show'])`.
fn extract_php_scoped_route(node: Node<'_>, source: &[u8]) -> Option<Endpoint> {
    let scope = node.child_by_field_name("scope")?;
    if !matches!(text(scope, source), "Route" | "Router") {
        return None;
    }
    let name = node.child_by_field_name("name")?;
    let method = HttpMethod::from_token(text(name, source))?;
    let args = node.child_by_field_name("arguments")?;
    let raw = php_positional_arg(args, 0).and_then(|n| string_literal_value(n, source))?;
    let handler = php_positional_arg(args, 1).and_then(|n| php_route_handler(n, source));
    Some(Endpoint {
        method,
        path: normalize_php_path(&raw),
        handler,
        line: node.start_position().row + 1,
        source: EndpointSource::PhpRoute,
    })
}

// Router-object routes: `$app->get('/ping', fn () => ...)`. Gated on a
// leading slash so ordinary method calls like `$bag->get('key')` are
// not mistaken for endpoints.
fn extract_php_member_route(node: Node<'_>, source: &[u8]) -> Option<Endpoint> {
    let name = node.child_by_field_name("name")?;
    let method = HttpMethod::from_token(text(name, source))?;
    let args = node.child_by_field_name("arguments")?;
    let raw = php_positional_arg(args, 0).and_then(|n| string_literal_value(n, source))?;
    if !raw.starts_with('/') {
        return None;
    }
    let handler = php_positional_arg(args, 1).and_then(|n| php_route_handler(n, source));
    Some(Endpoint {
        method,
        path: normalize_php_path(&raw),
        handler,
        line: node.start_position().row + 1,
        source: EndpointSource::PhpRoute,
    })
}

// Symfony attribute routes on a controller action:
// `#[Route('/users/{id}', methods: ['GET'])]`.
fn extract_php_attribute_routes(
    node: Node<'_>,
    source: &[u8],
    handler: Option<String>,
    out: &mut FileSummary,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "attribute_list" {
            continue;
        }
        let mut group_cursor = child.walk();
        for group in child.children(&mut group_cursor) {
            if group.kind() != "attribute_group" {
                continue;
            }
            let mut attr_cursor = group.walk();
            for attr in group.children(&mut attr_cursor) {
                if attr.kind() == "attribute" {
                    push_php_attribute_route(attr, source, &handler, out);
                }
            }
        }
    }
}

fn push_php_attribute_route(
    attr: Node<'_>,
    source: &[u8],
    handler: &Option<String>,
    out: &mut FileSummary,
) {
    let name = attr
        .named_child(0)
        .filter(|n| matches!(n.kind(), "name" | "qualified_name"));
    let Some(name) = name else { return };
    if php_rightmost(text(name, source)) != "Route" {
        return;
    }
    let Some(args) = attr.child_by_field_name("parameters") else {
        return;
    };
    let raw = php_positional_arg(args, 0)
        .or_else(|| php_named_arg(args, "path", source))
        .and_then(|n| string_literal_value(n, source));
    let Some(raw) = raw else { return };
    let path = normalize_php_path(&raw);

    let methods = php_named_arg(args, "methods", source)
        .map(|n| php_string_array(n, source))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| vec![HttpMethod::Any]);

    for method in methods {
        out.endpoints.push(Endpoint {
            method,
            path: path.clone(),
            handler: handler.clone(),
            line: attr.start_position().row + 1,
            source: EndpointSource::PhpAttributeRoute,
        });
    }
}

// Handler from a Laravel route action: a `'Ctrl@method'` string or a
// `[Ctrl::class, 'method']` array.
fn php_route_handler(value: Node<'_>, source: &[u8]) -> Option<String> {
    match value.kind() {
        "string" | "encapsed_string" => string_literal_value(value, source),
        "array_creation_expression" => {
            let mut elements = Vec::new();
            let mut cursor = value.walk();
            for child in value.children(&mut cursor) {
                if child.kind() == "array_element_initializer" {
                    elements.push(child);
                }
            }
            let class = elements
                .first()
                .and_then(|e| e.named_child(0))
                .filter(|n| n.kind() == "class_constant_access_expression")
                .and_then(|n| n.named_child(0))
                .map(|n| php_rightmost(text(n, source)));
            let method = elements
                .get(1)
                .and_then(|e| e.named_child(0))
                .and_then(|n| string_literal_value(n, source));
            match (class, method) {
                (Some(c), Some(m)) => Some(format!("{c}@{m}")),
                (Some(c), None) => Some(c),
                _ => None,
            }
        }
        _ => None,
    }
}

// nth positional (unnamed) argument value inside an `arguments` node.
fn php_positional_arg(args: Node<'_>, index: usize) -> Option<Node<'_>> {
    let mut seen = 0;
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() != "argument" {
            continue;
        }
        if child.child_by_field_name("name").is_some() {
            continue;
        }
        if seen == index {
            return php_argument_value(child);
        }
        seen += 1;
    }
    None
}

fn php_named_arg<'tree>(args: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if child.kind() != "argument" {
            continue;
        }
        if let Some(label) = child.child_by_field_name("name") {
            if text(label, source) == name {
                return php_argument_value(child);
            }
        }
    }
    None
}

fn php_argument_value(argument: Node<'_>) -> Option<Node<'_>> {
    let count = argument.named_child_count();
    if count == 0 {
        return None;
    }
    argument.named_child(count - 1)
}

fn php_string_array(node: Node<'_>, source: &[u8]) -> Vec<HttpMethod> {
    let mut methods = Vec::new();
    if node.kind() != "array_creation_expression" {
        return methods;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "array_element_initializer" {
            continue;
        }
        if let Some(token) = child
            .named_child(0)
            .and_then(|n| string_literal_value(n, source))
        {
            if let Some(method) = HttpMethod::from_token(&token) {
                methods.push(method);
            }
        }
    }
    methods
}

fn normalize_php_path(raw: &str) -> String {
    if raw.is_empty() {
        return "/".to_string();
    }
    if raw.starts_with('/') {
        raw.to_string()
    } else {
        format!("/{raw}")
    }
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
        assert_eq!(
            normalize_django_path("/already-slashed"),
            "/already-slashed"
        );
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

    fn parse_php(src: &str) -> FileSummary {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&Language::Php.ts_language())
            .expect("load PHP grammar");
        let tree = parser.parse(src, None).expect("parse PHP");
        let mut summary = FileSummary {
            path: PathBuf::from("test.php"),
            language: Language::Php,
            symbols: Vec::new(),
            imports: Vec::new(),
            semantic_tags: Vec::new(),
            endpoints: Vec::new(),
            line_count: src.lines().count(),
            call_sites: Vec::new(),
        };
        extract(
            Language::Php,
            tree.root_node(),
            src.as_bytes(),
            &mut summary,
        );
        summary
    }

    fn symbol_kind<'a>(summary: &'a FileSummary, name: &str) -> Option<&'a SymbolKind> {
        summary
            .symbols
            .iter()
            .find(|s| s.name == name)
            .map(|s| &s.kind)
    }

    #[test]
    fn php_extracts_declaration_kinds() {
        let src = r#"<?php
function greet() {}
class UserController { public function index() {} }
interface Repo {}
trait Sluggable {}
enum Status: string { case Active = 'active'; }
"#;
        let summary = parse_php(src);
        assert_eq!(symbol_kind(&summary, "greet"), Some(&SymbolKind::Function));
        assert_eq!(
            symbol_kind(&summary, "UserController"),
            Some(&SymbolKind::Class)
        );
        assert_eq!(symbol_kind(&summary, "index"), Some(&SymbolKind::Method));
        assert_eq!(symbol_kind(&summary, "Repo"), Some(&SymbolKind::Interface));
        assert_eq!(symbol_kind(&summary, "Sluggable"), Some(&SymbolKind::Trait));
        assert_eq!(symbol_kind(&summary, "Status"), Some(&SymbolKind::Enum));
    }

    #[test]
    fn php_collects_use_imports() {
        let src = "<?php\nuse App\\Models\\User;\nuse Illuminate\\Support\\Facades\\Route as R;\n";
        let summary = parse_php(src);
        assert!(summary.imports.iter().any(|i| i == "App\\Models\\User"));
        assert!(summary
            .imports
            .iter()
            .any(|i| i == "Illuminate\\Support\\Facades\\Route"));
    }

    #[test]
    fn php_laravel_facade_routes() {
        let src = r#"<?php
Route::get('/users', [UserController::class, 'index']);
Route::post('users', 'UserController@store');
"#;
        let summary = parse_php(src);
        let get = summary
            .endpoints
            .iter()
            .find(|e| e.method == HttpMethod::Get)
            .expect("GET endpoint");
        assert_eq!(get.path, "/users");
        assert_eq!(get.handler.as_deref(), Some("UserController@index"));
        assert_eq!(get.source, EndpointSource::PhpRoute);

        let post = summary
            .endpoints
            .iter()
            .find(|e| e.method == HttpMethod::Post)
            .expect("POST endpoint");
        assert_eq!(post.path, "/users");
        assert_eq!(post.handler.as_deref(), Some("UserController@store"));
    }

    #[test]
    fn php_member_route_requires_leading_slash() {
        let src = r#"<?php
$app->get('/ping', fn () => 'pong');
$bag->get('cache_key');
"#;
        let summary = parse_php(src);
        assert_eq!(summary.endpoints.len(), 1);
        assert_eq!(summary.endpoints[0].path, "/ping");
        assert_eq!(summary.endpoints[0].method, HttpMethod::Get);
    }

    #[test]
    fn php_symfony_attribute_route() {
        let src = r#"<?php
class BlogController {
    #[Route('/posts/{id}', methods: ['GET', 'HEAD'])]
    public function show() {}
}
"#;
        let summary = parse_php(src);
        let methods: Vec<HttpMethod> = summary.endpoints.iter().map(|e| e.method).collect();
        assert!(methods.contains(&HttpMethod::Get));
        assert!(methods.contains(&HttpMethod::Head));
        let show = summary
            .endpoints
            .iter()
            .find(|e| e.method == HttpMethod::Get)
            .unwrap();
        assert_eq!(show.path, "/posts/{id}");
        assert_eq!(show.handler.as_deref(), Some("show"));
        assert_eq!(show.source, EndpointSource::PhpAttributeRoute);
    }

    #[test]
    fn php_attribute_route_without_methods_defaults_to_any() {
        let src = r#"<?php
class HomeController {
    #[Route('/')]
    public function index() {}
}
"#;
        let summary = parse_php(src);
        assert_eq!(summary.endpoints.len(), 1);
        assert_eq!(summary.endpoints[0].method, HttpMethod::Any);
        assert_eq!(summary.endpoints[0].path, "/");
    }

    #[test]
    fn php_path_normalisation() {
        assert_eq!(normalize_php_path(""), "/");
        assert_eq!(normalize_php_path("users"), "/users");
        assert_eq!(normalize_php_path("/users"), "/users");
    }
}
