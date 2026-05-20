// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Architecture and stack conventions detected from manifests + file
//! layout. Captures decisions the LLM cannot infer from a single file:
//! which i18n library is used, where API routes live, whether the repo
//! is a monorepo, etc.

use squick_core::{EndpointSource, Project};
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

    if !detected.libraries.is_empty() {
        let _ = writeln!(out, "\n## Library choices");
        for (category, items) in &detected.libraries {
            let _ = writeln!(out, "- **{category}**: {}", items.iter().cloned().collect::<Vec<_>>().join(", "));
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
    libraries: BTreeMap<String, BTreeSet<String>>,
    api_surface: Vec<String>,
}

fn detect_conventions(project: &Project) -> Detected {
    let mut d = Detected::default();
    detect_layout(project, &mut d);
    detect_stack(project, &mut d);
    detect_libraries(project, &mut d);
    detect_api_surface(project, &mut d);
    d
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
            d.layout
                .push(format!("`{}` -> {identity}", if rel.is_empty() { "(root)".to_string() } else { rel }));
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

    if framework_tags.contains("framework-nextjs") {
        d.stack.insert("Frontend framework".into(), "Next.js".into());
    }
    if framework_tags.contains("framework-strapi") {
        d.stack.insert("Backend framework".into(), "Strapi".into());
    }
    if framework_tags.contains("framework-django") {
        d.stack.insert("Backend framework".into(), "Django".into());
    }
    if framework_tags.contains("framework-fastapi") {
        d.stack.insert("Backend framework".into(), "FastAPI".into());
    }
    if framework_tags.contains("framework-flask") {
        d.stack.insert("Backend framework".into(), "Flask".into());
    }
    if framework_tags.contains("framework-express") {
        d.stack.insert("Backend framework".into(), "Express".into());
    }
    if framework_tags.contains("framework-nestjs") {
        d.stack.insert("Backend framework".into(), "NestJS".into());
    }

    let mut languages: BTreeSet<&str> = project.files.iter().map(|f| f.language.as_str()).collect();
    languages.remove("");
    if !languages.is_empty() {
        d.stack.insert(
            "Languages".into(),
            languages.iter().cloned().collect::<Vec<_>>().join(", "),
        );
    }
}

fn detect_libraries(project: &Project, d: &mut Detected) {
    let all_deps: BTreeSet<String> = project
        .manifests
        .iter()
        .flat_map(|m| m.dependencies.iter().cloned())
        .collect();

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
            ],
        ),
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
        (
            "email",
            &[("nodemailer", "nodemailer")],
        ),
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

    for (category, rules) in categories {
        for (dep, label) in *rules {
            if all_deps.contains(*dep) {
                d.libraries
                    .entry(category.to_string())
                    .or_default()
                    .insert(label.to_string());
            }
        }
    }
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

fn endpoint_source_label(source: &EndpointSource) -> &'static str {
    match source {
        EndpointSource::PythonDecorator => "Python decorators (FastAPI/Flask)",
        EndpointSource::PythonUrlpatterns => "Django urlpatterns",
        EndpointSource::JsMethodCall => "JS member-calls (Express/Koa)",
        EndpointSource::NextjsRouteHandler => "Next.js App Router",
    }
}
