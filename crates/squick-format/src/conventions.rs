// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Architecture and stack conventions detected from manifests + file
//! layout. Captures decisions the LLM cannot infer from a single file:
//! which i18n library is used, where API routes live, whether the repo
//! is a monorepo, etc.

use squick_core::{DockerKind, EndpointSource, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

pub fn format_conventions(project: &Project) -> String {
    let detected = detect_conventions(project);

    let mut out = String::with_capacity(2048);
    let _ = writeln!(out, "# Squick conventions");
    let _ = writeln!(
        out,
        "\nArchitectural decisions and stack choices detected from manifests, \
         dependencies, and file layout. Consult this artifact when answering \
         \"how is X organized\" or \"which library does this project use for Y\" \
         instead of scanning the codebase."
    );

    if !detected.layout.is_empty() {
        let _ = writeln!(out, "\n## Repository layout");
        for line in &detected.layout {
            let _ = writeln!(out, "- {line}");
        }
    }

    if !detected.stack.is_empty() {
        let _ = writeln!(out, "\n## Stack");
        for (k, v) in &detected.stack {
            let _ = writeln!(out, "- **{k}**: {v}");
        }
    }

    if !detected.containerization.is_empty() {
        let _ = writeln!(out, "\n## Containerization");
        for line in &detected.containerization {
            // Nested lines arrive pre-indented; top-level lines get a bullet.
            if line.starts_with(' ') {
                let _ = writeln!(out, "{line}");
            } else {
                let _ = writeln!(out, "- {line}");
            }
        }
    }

    if !detected.libraries.is_empty() {
        let _ = writeln!(out, "\n## Library choices");
        for (category, items) in &detected.libraries {
            let _ = writeln!(
                out,
                "- **{category}**: {}",
                items.iter().cloned().collect::<Vec<_>>().join(", ")
            );
        }
    }

    if !detected.api_surface.is_empty() {
        let _ = writeln!(out, "\n## API surface");
        for line in &detected.api_surface {
            let _ = writeln!(out, "- {line}");
        }
    }

    out
}

#[derive(Default)]
struct Detected {
    layout: Vec<String>,
    stack: BTreeMap<String, String>,
    containerization: Vec<String>,
    libraries: BTreeMap<String, BTreeSet<String>>,
    api_surface: Vec<String>,
}

fn detect_conventions(project: &Project) -> Detected {
    let mut d = Detected::default();
    detect_layout(project, &mut d);
    detect_stack(project, &mut d);
    detect_containerization(project, &mut d);
    detect_libraries(project, &mut d);
    detect_api_surface(project, &mut d);
    d
}

fn detect_containerization(project: &Project, d: &mut Detected) {
    if let Some((stack_value, lines)) = containerization_summary(project) {
        d.stack.insert("Containerization".into(), stack_value);
        d.containerization = lines;
    }
}

/// Builds the containerization summary: the stack value (`Docker`,
/// `Docker Compose`) and the detail lines (base images, ports, services,
/// backing stores). Shared by the global conventions view and `infra.md`.
/// Returns `None` when the repo has no container files.
pub(crate) fn containerization_summary(project: &Project) -> Option<(String, Vec<String>)> {
    if project.docker.is_empty() {
        return None;
    }

    let dockerfiles: Vec<_> = project
        .docker
        .iter()
        .filter(|a| a.kind == DockerKind::Dockerfile)
        .collect();
    let compose: Vec<_> = project
        .docker
        .iter()
        .filter(|a| a.kind == DockerKind::Compose)
        .collect();

    let mut stack_parts: Vec<&str> = Vec::new();
    if !dockerfiles.is_empty() {
        stack_parts.push("Docker");
    }
    if !compose.is_empty() {
        stack_parts.push("Docker Compose");
    }
    let stack_value = stack_parts.join(" + ");

    let mut lines: Vec<String> = Vec::new();

    if !dockerfiles.is_empty() {
        let mut base_images: BTreeSet<String> = BTreeSet::new();
        let mut ports: BTreeSet<String> = BTreeSet::new();
        let mut entrypoints: BTreeSet<String> = BTreeSet::new();
        let mut commands: BTreeSet<String> = BTreeSet::new();
        let mut users: BTreeSet<String> = BTreeSet::new();
        let mut build_args: BTreeSet<String> = BTreeSet::new();
        let mut env_keys: BTreeSet<String> = BTreeSet::new();
        let mut volumes: BTreeSet<String> = BTreeSet::new();
        let mut multi_stage = false;
        for art in &dockerfiles {
            for stage in &art.stages {
                base_images.insert(stage.base_image.clone());
            }
            if art.stages.len() > 1 {
                multi_stage = true;
            }
            ports.extend(art.exposed_ports.iter().cloned());
            entrypoints.extend(art.entrypoint.clone());
            commands.extend(art.cmd.clone());
            users.extend(art.user.clone());
            build_args.extend(art.build_args.iter().cloned());
            env_keys.extend(art.env_keys.iter().cloned());
            volumes.extend(art.volumes.iter().cloned());
        }
        lines.push(format!(
            "{} Dockerfile(s){}",
            dockerfiles.len(),
            if multi_stage {
                "; multi-stage build"
            } else {
                ""
            }
        ));
        push_joined(&mut lines, "Base images", base_images);
        if !entrypoints.is_empty() {
            lines.push(format!(
                "Entrypoint: {}",
                entrypoints.into_iter().collect::<Vec<_>>().join(" | ")
            ));
        }
        if !commands.is_empty() {
            lines.push(format!(
                "Default command: {}",
                commands.into_iter().collect::<Vec<_>>().join(" | ")
            ));
        }
        if !users.is_empty() {
            lines.push(format!(
                "Runs as user: {}",
                users.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        push_joined(&mut lines, "Build args", build_args);
        push_joined(&mut lines, "Env vars", env_keys);
        push_joined(&mut lines, "Declared volumes", volumes);
        push_joined(&mut lines, "Exposed ports", ports);
    }

    if !compose.is_empty() {
        let mut count = 0usize;
        let mut names: Vec<String> = Vec::new();
        let mut backing: BTreeSet<String> = BTreeSet::new();
        for art in &compose {
            for tag in &art.tags {
                if let Some(name) = tag.label.strip_prefix("service-") {
                    backing.insert(name.to_string());
                }
            }
            for svc in &art.services {
                count += 1;
                names.push(svc.name.clone());
            }
        }
        lines.push(format!(
            "Docker Compose: {count} service(s) ({})",
            names.join(", ")
        ));
        // One nested line per service that carries configuration worth noting.
        for art in &compose {
            for svc in &art.services {
                let mut bits: Vec<String> = Vec::new();
                if let Some(cmd) = &svc.command {
                    bits.push(format!("command `{cmd}`"));
                }
                if !svc.environment.is_empty() {
                    bits.push(format!("env {}", svc.environment.join(", ")));
                }
                if !svc.env_file.is_empty() {
                    bits.push(format!("env_file {}", svc.env_file.join(", ")));
                }
                if !svc.volumes.is_empty() {
                    bits.push(format!("volumes {}", svc.volumes.join(", ")));
                }
                if !svc.networks.is_empty() {
                    bits.push(format!("networks {}", svc.networks.join(", ")));
                }
                if !bits.is_empty() {
                    lines.push(format!("  - {}: {}", svc.name, bits.join("; ")));
                }
            }
        }
        if !backing.is_empty() {
            lines.push(format!(
                "Backing services: {}",
                backing.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    Some((stack_value, lines))
}

fn push_joined(out: &mut Vec<String>, label: &str, values: BTreeSet<String>) {
    if !values.is_empty() {
        out.push(format!(
            "{label}: {}",
            values.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
}

fn detect_layout(project: &Project, d: &mut Detected) {
    if project.manifests.len() > 1 {
        d.layout.push(format!(
            "Monorepo with {} sub-projects",
            project.manifests.len()
        ));
        for manifest in &project.manifests {
            let rel = manifest
                .path
                .parent()
                .and_then(|p| p.strip_prefix(&project.root).ok())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(root)".to_string());
            let identity = match (&manifest.name, &manifest.version) {
                (Some(n), Some(v)) => format!("{n}@{v}"),
                (Some(n), None) => n.clone(),
                _ => "unnamed".to_string(),
            };
            d.layout.push(format!(
                "`{}` -> {identity}",
                if rel.is_empty() {
                    "(root)".to_string()
                } else {
                    rel
                }
            ));
        }
    }
}

fn detect_stack(project: &Project, d: &mut Detected) {
    let mut framework_tags: BTreeSet<String> = BTreeSet::new();
    for manifest in &project.manifests {
        for tag in &manifest.framework_tags {
            framework_tags.insert(tag.label.clone());
        }
    }
    d.stack.extend(framework_stack(&framework_tags));

    let mut languages: BTreeSet<&str> = project.files.iter().map(|f| f.language.as_str()).collect();
    languages.remove("");
    if !languages.is_empty() {
        d.stack.insert(
            "Languages".into(),
            languages.iter().cloned().collect::<Vec<_>>().join(", "),
        );
    }
}

/// Maps framework tags to stack entries (frontend/backend framework labels).
/// Shared by the global conventions view and per-area views.
pub(crate) fn framework_stack(tags: &BTreeSet<String>) -> BTreeMap<String, String> {
    let mut stack = BTreeMap::new();
    let mappings: &[(&str, &str, &str)] = &[
        ("framework-nextjs", "Frontend framework", "Next.js"),
        ("framework-strapi", "Backend framework", "Strapi"),
        ("framework-django", "Backend framework", "Django"),
        ("framework-fastapi", "Backend framework", "FastAPI"),
        ("framework-flask", "Backend framework", "Flask"),
        ("framework-express", "Backend framework", "Express"),
        ("framework-nestjs", "Backend framework", "NestJS"),
        ("framework-laravel", "Backend framework", "Laravel"),
        ("framework-symfony", "Backend framework", "Symfony"),
    ];
    for (tag, slot, label) in mappings {
        if tags.contains(*tag) {
            stack.insert(slot.to_string(), label.to_string());
        }
    }
    stack
}

fn detect_libraries(project: &Project, d: &mut Detected) {
    let all_deps: BTreeSet<String> = project
        .manifests
        .iter()
        .flat_map(|m| m.dependencies.iter().cloned())
        .collect();
    d.libraries = library_choices(&all_deps);
}

/// Maps a dependency set to detected library choices grouped by category.
/// Shared by the global conventions view and per-area views.
pub(crate) fn library_choices(deps: &BTreeSet<String>) -> BTreeMap<String, BTreeSet<String>> {
    let categories: &[(&str, &[(&str, &str)])] = &[
        (
            "i18n",
            &[
                ("next-intl", "next-intl"),
                ("react-intl", "react-intl"),
                ("i18next", "i18next"),
                ("react-i18next", "react-i18next"),
                ("formatjs", "formatjs"),
            ],
        ),
        (
            "state management",
            &[
                ("redux", "Redux"),
                ("@reduxjs/toolkit", "Redux Toolkit"),
                ("zustand", "Zustand"),
                ("jotai", "Jotai"),
                ("mobx", "MobX"),
                ("recoil", "Recoil"),
                ("valtio", "Valtio"),
            ],
        ),
        (
            "styling",
            &[
                ("tailwindcss", "Tailwind CSS"),
                ("styled-components", "styled-components"),
                ("@emotion/react", "Emotion"),
                ("sass", "Sass"),
                ("postcss", "PostCSS"),
            ],
        ),
        (
            "data fetching",
            &[
                ("swr", "SWR"),
                ("@tanstack/react-query", "TanStack Query"),
                ("react-query", "React Query"),
                ("@apollo/client", "Apollo Client"),
                ("urql", "urql"),
                ("axios", "axios"),
            ],
        ),
        (
            "forms",
            &[
                ("react-hook-form", "react-hook-form"),
                ("formik", "Formik"),
                ("@hookform/resolvers", "react-hook-form resolvers"),
            ],
        ),
        (
            "validation",
            &[
                ("zod", "Zod"),
                ("yup", "Yup"),
                ("joi", "Joi"),
                ("valibot", "Valibot"),
                ("pydantic", "Pydantic"),
            ],
        ),
        (
            "testing",
            &[
                ("vitest", "Vitest"),
                ("jest", "Jest"),
                ("mocha", "Mocha"),
                ("pytest", "pytest"),
                ("playwright", "Playwright"),
                ("cypress", "Cypress"),
                ("phpunit/phpunit", "PHPUnit"),
                ("pestphp/pest", "Pest"),
            ],
        ),
        (
            "ORM / DB",
            &[
                ("prisma", "Prisma"),
                ("@prisma/client", "Prisma client"),
                ("drizzle-orm", "Drizzle"),
                ("typeorm", "TypeORM"),
                ("sequelize", "Sequelize"),
                ("mongoose", "Mongoose"),
                ("sqlalchemy", "SQLAlchemy"),
                ("pg", "node-postgres"),
                ("pymysql", "PyMySQL"),
                ("doctrine/orm", "Doctrine"),
                ("illuminate/database", "Eloquent"),
            ],
        ),
        (
            "templating",
            &[("twig/twig", "Twig"), ("symfony/twig-bundle", "Twig")],
        ),
        ("HTTP client", &[("guzzlehttp/guzzle", "Guzzle")]),
        (
            "monorepo tooling",
            &[
                ("turbo", "Turborepo"),
                ("nx", "Nx"),
                ("@nx/workspace", "Nx"),
                ("lerna", "Lerna"),
            ],
        ),
        (
            "PDF / docs",
            &[
                ("jspdf", "jsPDF"),
                ("html2canvas-pro", "html2canvas"),
                ("react-markdown", "react-markdown"),
                ("remark-gfm", "remark-gfm"),
            ],
        ),
        ("email", &[("nodemailer", "nodemailer")]),
        (
            "date / time",
            &[
                ("dayjs", "Day.js"),
                ("date-fns", "date-fns"),
                ("moment", "Moment.js"),
                ("luxon", "Luxon"),
            ],
        ),
    ];

    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (category, rules) in categories {
        for (dep, label) in *rules {
            if deps.contains(*dep) {
                out.entry(category.to_string())
                    .or_default()
                    .insert(label.to_string());
            }
        }
    }
    out
}

fn detect_api_surface(project: &Project, d: &mut Detected) {
    let endpoint_count: usize = project.files.iter().map(|f| f.endpoints.len()).sum();
    if endpoint_count > 0 {
        d.api_surface
            .push(format!("{endpoint_count} HTTP endpoint(s) detected"));

        let mut sources: BTreeSet<&'static str> = BTreeSet::new();
        for file in &project.files {
            for ep in &file.endpoints {
                sources.insert(endpoint_source_label(&ep.source));
            }
        }
        d.api_surface.push(format!(
            "Endpoint declaration styles: {}",
            sources.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    if !project.strapi_schemas.is_empty() {
        d.api_surface.push(format!(
            "{} Strapi content type(s); see `schemas.md` or `context.ndjson` for fields",
            project.strapi_schemas.len()
        ));
    }
}

pub(crate) fn endpoint_source_label(source: &EndpointSource) -> &'static str {
    match source {
        EndpointSource::PythonDecorator => "Python decorators (FastAPI/Flask)",
        EndpointSource::PythonUrlpatterns => "Django urlpatterns",
        EndpointSource::JsMethodCall => "JS member-calls (Express/Koa)",
        EndpointSource::NextjsRouteHandler => "Next.js App Router",
        EndpointSource::PhpRoute => "PHP route calls (Laravel/Slim)",
        EndpointSource::PhpAttributeRoute => "PHP attributes (Symfony)",
    }
}
