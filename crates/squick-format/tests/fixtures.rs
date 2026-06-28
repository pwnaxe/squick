// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! End-to-end pipeline tests: scan a bundled fixture, then assert the facts
//! and rendered artifacts. Assertions are semantic (presence of frameworks,
//! endpoints, languages) rather than byte-for-byte golden files, so they hold
//! across the Linux/macOS/Windows CI matrix where path separators differ.

use squick_core::{
    DockerKind, EndpointSource, HttpMethod, ManifestKind, Project, ScanOptions, Scanner,
};
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
fn multi_framework_parses_dockerfile_and_compose() {
    let project = fixture("multi-framework");

    let dockerfile = project
        .docker
        .iter()
        .find(|a| a.kind == DockerKind::Dockerfile)
        .expect("Dockerfile artifact");
    // Multi-stage: node build stage + nginx runtime.
    assert_eq!(dockerfile.stages.len(), 2);
    assert_eq!(dockerfile.stages[0].name.as_deref(), Some("build"));
    assert!(dockerfile.stages[0].base_image.starts_with("node:"));
    assert!(dockerfile.exposed_ports.contains(&"80".to_string()));
    let docker_labels: Vec<&str> = dockerfile.tags.iter().map(|t| t.label.as_str()).collect();
    assert!(docker_labels.contains(&"base-node"));
    assert!(docker_labels.contains(&"base-nginx"));
    assert!(docker_labels.contains(&"docker-multi-stage"));

    // Tier 1 runtime + config surface.
    assert_eq!(dockerfile.entrypoint.as_deref(), Some("nginx"));
    assert_eq!(dockerfile.cmd.as_deref(), Some("-g daemon off;"));
    assert_eq!(dockerfile.workdir.as_deref(), Some("/app"));
    assert_eq!(dockerfile.user.as_deref(), Some("nginx"));
    assert!(dockerfile.build_args.contains(&"NODE_ENV".to_string()));
    assert!(dockerfile.env_keys.contains(&"PORT".to_string()));
    assert!(dockerfile
        .env_keys
        .contains(&"NEXT_TELEMETRY_DISABLED".to_string()));
    assert!(dockerfile.volumes.contains(&"/var/cache/nginx".to_string()));

    let compose = project
        .docker
        .iter()
        .find(|a| a.kind == DockerKind::Compose)
        .expect("Compose artifact");
    assert!(compose.services.iter().any(|s| s.name == "db"));
    let web = compose
        .services
        .iter()
        .find(|s| s.name == "web")
        .expect("web service");
    assert_eq!(web.build.as_deref(), Some("."));
    assert!(web.depends_on.contains(&"db".to_string()));
    let compose_labels: Vec<&str> = compose.tags.iter().map(|t| t.label.as_str()).collect();
    assert!(compose_labels.contains(&"service-postgres"));
    assert!(compose_labels.contains(&"service-redis"));

    // Tier 1 per-service config: command, environment (keys), volumes, networks.
    let api = compose
        .services
        .iter()
        .find(|s| s.name == "api")
        .expect("api service");
    assert_eq!(
        api.command.as_deref(),
        Some("uvicorn main:app --host 0.0.0.0")
    );
    assert!(api.environment.contains(&"DATABASE_URL".to_string()));
    assert!(api.environment.contains(&"REDIS_URL".to_string()));
    // Values are dropped; only keys are retained.
    assert!(!api.environment.iter().any(|e| e.contains('=')));
    assert!(api.env_file.contains(&".env".to_string()));
    assert!(api.volumes.contains(&"./api:/code".to_string()));
    assert!(api.networks.contains(&"backend".to_string()));

    // Mapping-form environment (db service) yields its keys too.
    let db = compose
        .services
        .iter()
        .find(|s| s.name == "db")
        .expect("db service");
    assert!(db.environment.contains(&"POSTGRES_PASSWORD".to_string()));
}

#[test]
fn multi_framework_conventions_report_containerization() {
    let project = fixture("multi-framework");
    let conventions = squick_format::format_conventions(&project);

    assert!(
        conventions.contains("## Containerization"),
        "conventions: {conventions}"
    );
    assert!(
        conventions.contains("Docker Compose"),
        "conventions: {conventions}"
    );
    assert!(
        conventions.contains("postgres"),
        "conventions: {conventions}"
    );

    // Docker facts must also reach the NDJSON and triple emitters.
    let ndjson = squick_format::format_ndjson(&project);
    assert!(
        ndjson.lines().any(|l| l.contains("\"k\":\"dock\"")),
        "ndjson should contain docker facts"
    );
    let triples = squick_format::format_triples(&project);
    assert!(
        triples.contains("type compose"),
        "triples should describe the compose file"
    );
}

#[test]
fn compact_format_is_aligned_and_smaller() {
    let project = fixture("multi-framework");
    let compact = squick_format::format_compact(&project);

    // Legend first, then sectioned records.
    assert!(compact.starts_with("# squick compact v1"));
    assert!(compact.contains("\n@file\t"));
    assert!(compact.contains("\n@ep\t"));
    assert!(compact.contains("\n@dock\t"));

    // Every data row must have exactly as many fields as its section header.
    let mut header_cols: Option<usize> = None;
    for line in compact.lines() {
        if line.starts_with('#') {
            continue;
        }
        let cols = line.split('\t').count();
        if let Some(name) = line.strip_prefix('@') {
            // Header declares the column set (minus the `@type` marker cell).
            header_cols = Some(cols - 1);
            assert!(!name.is_empty());
        } else {
            let expected = header_cols.expect("row before any section header");
            assert_eq!(
                cols, expected,
                "row has {cols} fields, header declared {expected}: {line}"
            );
        }
    }

    // The whole point: denser than the JSON view.
    let ndjson = squick_format::format_ndjson(&project);
    assert!(
        compact.len() < ndjson.len(),
        "compact ({}) should be smaller than ndjson ({})",
        compact.len(),
        ndjson.len()
    );
}

#[test]
fn monorepo_splits_into_per_subproject_areas() {
    let project = fixture("monorepo");
    let areas = squick_format::detect_areas(&project);

    let titles: Vec<&str> = areas.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(areas.len(), 2, "areas: {titles:?}");
    assert!(titles.contains(&"frontend"));
    assert!(titles.contains(&"backend"));

    // Files land in the right area.
    let backend = areas.iter().find(|a| a.title == "backend").unwrap();
    let backend_md = squick_format::format_area(&project, backend);
    assert!(backend_md.contains("FastAPI"), "backend: {backend_md}");
    assert!(backend_md.contains("/products"), "backend: {backend_md}");
    let frontend = areas.iter().find(|a| a.title == "frontend").unwrap();
    let frontend_md = squick_format::format_area(&project, frontend);
    assert!(frontend_md.contains("Next.js"), "frontend: {frontend_md}");

    // Navigation routes to each area file.
    let nav = squick_format::format_navigation(&project, &areas, true, true);
    assert!(nav.contains("area-frontend.md"), "nav: {nav}");
    assert!(nav.contains("area-backend.md"), "nav: {nav}");

    // Docker stays cross-cutting in infra, not severed into an area.
    let infra = squick_format::format_infra(&project).expect("infra doc");
    assert!(infra.contains("Compose"), "infra: {infra}");
    assert!(infra.contains("postgres"), "infra: {infra}");
}

#[test]
fn polyglot_single_root_does_not_split() {
    // multi-framework holds three manifests in the same root directory; that
    // is one polyglot project, not a monorepo, so it must not split.
    let project = fixture("multi-framework");
    assert!(squick_format::detect_areas(&project).is_empty());
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
