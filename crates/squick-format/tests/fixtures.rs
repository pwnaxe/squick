// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! End-to-end pipeline tests: scan a bundled fixture, then assert the facts
//! and rendered artifacts. Assertions are semantic (presence of frameworks,
//! endpoints, languages) rather than byte-for-byte golden files, so they hold
//! across the Linux/macOS/Windows CI matrix where path separators differ.

use squick_core::{EndpointSource, HttpMethod, ManifestKind, Project, ScanOptions, Scanner};
use std::path::PathBuf;

fn fixture(name: &str) -> Project {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    Scanner::new(ScanOptions::default())
        .scan_project(&path)
        .unwrap_or_else(|e| panic!("scanning fixture {name}: {e}"))
}

fn endpoints(project: &Project) -> Vec<&squick_core::Endpoint> {
    project
        .files
        .iter()
        .flat_map(|f| f.endpoints.iter())
        .collect()
}

#[test]
fn multi_framework_parses_composer_manifest() {
    let project = fixture("multi-framework");

    let composer = project
        .manifests
        .iter()
        .find(|m| m.kind == ManifestKind::PhpComposer)
        .expect("composer.json manifest");

    assert_eq!(composer.name.as_deref(), Some("acme/storefront"));
    assert!(
        composer
            .dependencies
            .iter()
            .any(|d| d == "laravel/framework"),
        "composer deps should include laravel/framework"
    );
    // `php` and `ext-*` platform requirements are filtered out.
    assert!(!composer.dependencies.iter().any(|d| d == "php"));
    assert!(
        composer
            .framework_tags
            .iter()
            .any(|t| t.label == "framework-laravel"),
        "composer should infer framework-laravel"
    );
}

#[test]
fn multi_framework_detects_php_language() {
    let project = fixture("multi-framework");
    assert!(
        project.files.iter().any(|f| f.language.as_str() == "php"),
        "fixture should contain PHP files"
    );
}

#[test]
fn multi_framework_extracts_laravel_and_symfony_routes() {
    let project = fixture("multi-framework");
    let eps = endpoints(&project);

    let laravel_get = eps
        .iter()
        .find(|e| e.source == EndpointSource::PhpRoute && e.path == "/users")
        .expect("Laravel Route::get('/users')");
    assert_eq!(laravel_get.method, HttpMethod::Get);

    let symfony = eps
        .iter()
        .find(|e| e.source == EndpointSource::PhpAttributeRoute)
        .expect("Symfony #[Route] attribute endpoint");
    assert_eq!(symfony.path, "/api/users/{id}");
    assert_eq!(symfony.method, HttpMethod::Get);
}

#[test]
fn multi_framework_conventions_name_the_stack() {
    let project = fixture("multi-framework");
    let conventions = squick_format::format_conventions(&project);

    assert!(
        conventions.contains("Laravel"),
        "conventions: {conventions}"
    );
    assert!(conventions.contains("php"), "conventions: {conventions}");
    assert!(
        conventions.contains("HTTP endpoint(s) detected"),
        "conventions: {conventions}"
    );
}

#[test]
fn multi_framework_renders_schemas_and_markdown() {
    let project = fixture("multi-framework");

    let schemas = squick_format::format_schemas(&project).expect("schemas output");
    assert!(schemas.contains("/api/users/{id}"), "schemas: {schemas}");

    let markdown = squick_format::format_markdown(&project);
    assert!(!markdown.trim().is_empty());
}

#[test]
fn sample_fixture_scans_clean() {
    let project = fixture("sample");
    assert!(
        !project.files.is_empty(),
        "sample fixture should yield files"
    );
    // Mixed Python + TSX fixture; the markdown view must always render.
    let markdown = squick_format::format_markdown(&project);
    assert!(!markdown.trim().is_empty());
}
