// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Treats Docker as a first-class stack. Walks the tree for `Dockerfile`s
//! and Compose files, extracts base images, build stages, exposed ports,
//! and Compose services, then derives semantic tags (runtime base images,
//! backing services, multi-stage builds). Results land on `Project.docker`.

use crate::types::{
    Confidence, DockerArtifact, DockerKind, DockerService, DockerStage, Project, SemanticTag,
    TagSource,
};
use ignore::WalkBuilder;
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
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_dockerfile(name) {
            if let Some(artifact) = parse_dockerfile(path) {
                project.docker.push(artifact);
            }
        } else if is_compose(name) {
            if let Some(artifact) = parse_compose(path) {
                project.docker.push(artifact);
            }
        }
    }
}

fn is_dockerfile(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    // `Dockerfile.md` / `Dockerfile.txt` are documentation, not build files.
    if lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".txt") {
        return false;
    }
    lower == "dockerfile"
        || lower == "containerfile"
        || lower.starts_with("dockerfile.")
        || lower.ends_with(".dockerfile")
}

fn is_compose(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".yml") || lower.ends_with(".yaml")) {
        return false;
    }
    // `docker-compose*.y(a)ml` and `compose*.y(a)ml`, but not `composer.*`.
    lower.starts_with("docker-compose") || lower.starts_with("compose.")
}

fn parse_dockerfile(path: &Path) -> Option<DockerArtifact> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut stages: Vec<DockerStage> = Vec::new();
    let mut exposed_ports: Vec<String> = Vec::new();
    let mut entrypoint: Option<String> = None;
    let mut cmd: Option<String> = None;
    let mut workdir: Option<String> = None;
    let mut user: Option<String> = None;
    let mut env_keys: Vec<String> = Vec::new();
    let mut build_args: Vec<String> = Vec::new();
    let mut volumes: Vec<String> = Vec::new();

    for line in logical_lines(&text) {
        let Some((instruction, rest)) = line.trim().split_once(char::is_whitespace) else {
            continue;
        };
        let rest = rest.trim();
        match instruction.to_ascii_uppercase().as_str() {
            "FROM" => {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                // Skip leading flags such as `--platform=linux/amd64`.
                let mut idx = 0;
                while idx < parts.len() && parts[idx].starts_with("--") {
                    idx += 1;
                }
                let Some(base_image) = parts.get(idx) else {
                    continue;
                };
                let name = match parts.get(idx + 1) {
                    Some(kw) if kw.eq_ignore_ascii_case("as") => {
                        parts.get(idx + 2).map(|s| s.to_string())
                    }
                    _ => None,
                };
                stages.push(DockerStage {
                    base_image: base_image.to_string(),
                    name,
                });
            }
            "EXPOSE" => {
                for port in rest.split_whitespace() {
                    push_unique(&mut exposed_ports, port.to_string());
                }
            }
            // Last ENTRYPOINT/CMD/WORKDIR/USER win, matching Docker semantics.
            "ENTRYPOINT" => entrypoint = Some(exec_or_shell(rest)),
            "CMD" => cmd = Some(exec_or_shell(rest)),
            "WORKDIR" => workdir = Some(rest.to_string()),
            "USER" => user = Some(rest.to_string()),
            "ENV" => {
                for key in env_keys_from(rest) {
                    push_unique(&mut env_keys, key);
                }
            }
            "ARG" => {
                let name = rest.split(['=', ' ']).next().unwrap_or(rest).trim();
                if !name.is_empty() {
                    push_unique(&mut build_args, name.to_string());
                }
            }
            "VOLUME" => {
                for path in exec_or_list(rest) {
                    push_unique(&mut volumes, path);
                }
            }
            _ => {}
        }
    }

    if stages.is_empty() {
        return None;
    }

    let tags = dockerfile_tags(&stages);
    Some(DockerArtifact {
        kind: DockerKind::Dockerfile,
        path: path.to_path_buf(),
        stages,
        exposed_ports,
        services: Vec::new(),
        tags,
        entrypoint,
        cmd,
        workdir,
        user,
        env_keys,
        build_args,
        volumes,
    })
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !value.is_empty() && !list.contains(&value) {
        list.push(value);
    }
}

/// Renders an `ENTRYPOINT`/`CMD` argument. Exec form (`["a","b"]`) is joined
/// with spaces; shell form is returned as written.
fn exec_or_shell(rest: &str) -> String {
    let items = exec_or_list(rest);
    if items.is_empty() {
        rest.to_string()
    } else {
        items.join(" ")
    }
}

/// Parses a Dockerfile JSON-array argument (`["/data", "/cache"]`) into its
/// elements. Falls back to whitespace-splitting the shell form.
fn exec_or_list(rest: &str) -> Vec<String> {
    let trimmed = rest.trim();
    if trimmed.starts_with('[') {
        if let Ok(serde_json::Value::Array(items)) =
            serde_json::from_str::<serde_json::Value>(trimmed)
        {
            return items
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
    }
    trimmed.split_whitespace().map(String::from).collect()
}

/// Extracts variable names from an `ENV` instruction, handling both the
/// `ENV K=v K2=v2` and legacy `ENV K v` forms.
fn env_keys_from(rest: &str) -> Vec<String> {
    if rest.contains('=') {
        rest.split_whitespace()
            .filter_map(|tok| tok.split('=').next())
            .filter(|k| !k.is_empty())
            .map(String::from)
            .collect()
    } else {
        rest.split_whitespace()
            .next()
            .map(|k| vec![k.to_string()])
            .unwrap_or_default()
    }
}

/// Joins backslash-continued lines and drops blank/comment lines so each
/// returned string is one logical Dockerfile instruction.
fn logical_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if current.is_empty() && (trimmed.is_empty() || trimmed.starts_with('#')) {
            continue;
        }
        if let Some(stripped) = line.strip_suffix('\\') {
            current.push_str(stripped.trim_end());
            current.push(' ');
        } else {
            current.push_str(line);
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.trim().is_empty() {
        lines.push(current);
    }
    lines
}

fn parse_compose(path: &Path) -> Option<DockerArtifact> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).ok()?;
    let services_map = value.get("services").and_then(|v| v.as_mapping())?;

    let mut services = Vec::new();
    for (key, svc) in services_map {
        let Some(name) = key.as_str() else {
            continue;
        };
        let image = svc.get("image").and_then(|v| v.as_str()).map(String::from);
        let build = match svc.get("build") {
            Some(b) if b.is_string() => b.as_str().map(String::from),
            Some(b) => b.get("context").and_then(|v| v.as_str()).map(String::from),
            None => None,
        };
        let ports = svc
            .get("ports")
            .and_then(|v| v.as_sequence())
            .map(|seq| seq.iter().filter_map(port_to_string).collect())
            .unwrap_or_default();
        let depends_on = seq_or_map_keys(svc.get("depends_on"));
        let command = svc.get("command").map(render_command);
        let environment = environment_keys(svc.get("environment"));
        let env_file = string_or_seq(svc.get("env_file"));
        let volumes = svc
            .get("volumes")
            .and_then(|v| v.as_sequence())
            .map(|seq| seq.iter().filter_map(volume_to_string).collect())
            .unwrap_or_default();
        let networks = seq_or_map_keys(svc.get("networks"));
        services.push(DockerService {
            name: name.to_string(),
            image,
            build,
            ports,
            depends_on,
            command,
            environment,
            env_file,
            volumes,
            networks,
        });
    }

    if services.is_empty() {
        return None;
    }

    let tags = compose_tags(&services);
    Some(DockerArtifact {
        kind: DockerKind::Compose,
        path: path.to_path_buf(),
        stages: Vec::new(),
        exposed_ports: Vec::new(),
        services,
        tags,
        entrypoint: None,
        cmd: None,
        workdir: None,
        user: None,
        env_keys: Vec::new(),
        build_args: Vec::new(),
        volumes: Vec::new(),
    })
}

/// Renders a Compose `ports` entry as a string. Handles short string form
/// (`"8080:80"`), bare numbers, and the long mapping form (`published`/`target`).
fn port_to_string(value: &serde_yaml_ng::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = value.as_u64() {
        return Some(n.to_string());
    }
    if value.is_mapping() {
        let published = value.get("published").and_then(scalar_to_string);
        let target = value.get("target").and_then(scalar_to_string);
        return match (published, target) {
            (Some(p), Some(t)) => Some(format!("{p}:{t}")),
            (Some(p), None) => Some(p),
            (None, Some(t)) => Some(t),
            (None, None) => None,
        };
    }
    None
}

fn scalar_to_string(value: &serde_yaml_ng::Value) -> Option<String> {
    value
        .as_str()
        .map(String::from)
        .or_else(|| value.as_u64().map(|n| n.to_string()))
}

/// Collects a Compose field that may be a sequence of strings or a mapping
/// whose keys are the values (the two forms `depends_on` and `networks` take).
fn seq_or_map_keys(value: Option<&serde_yaml_ng::Value>) -> Vec<String> {
    match value {
        Some(v) if v.is_sequence() => v
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect(),
        Some(v) => v
            .as_mapping()
            .map(|m| {
                m.keys()
                    .filter_map(|k| k.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Collects a Compose field that is either a single string or a sequence of
/// strings (e.g. `env_file`).
fn string_or_seq(value: Option<&serde_yaml_ng::Value>) -> Vec<String> {
    match value {
        Some(v) if v.is_string() => v.as_str().map(|s| vec![s.to_string()]).unwrap_or_default(),
        Some(v) => v
            .as_sequence()
            .map(|seq| {
                seq.iter()
                    .filter_map(|item| {
                        item.as_str()
                            .map(String::from)
                            .or_else(|| item.get("path").and_then(|p| p.as_str()).map(String::from))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Renders a Compose `command:` (string or sequence) as one string.
fn render_command(value: &serde_yaml_ng::Value) -> String {
    if let Some(s) = value.as_str() {
        return s.to_string();
    }
    value
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// Extracts keys from a Compose `environment:` block. Mapping form yields its
/// keys; list form (`KEY=value`) yields the part before `=`. Values are
/// dropped to avoid leaking secrets into the context.
fn environment_keys(value: Option<&serde_yaml_ng::Value>) -> Vec<String> {
    match value {
        Some(v) if v.is_mapping() => v
            .as_mapping()
            .unwrap()
            .keys()
            .filter_map(|k| k.as_str().map(String::from))
            .collect(),
        Some(v) => v
            .as_sequence()
            .map(|seq| {
                seq.iter()
                    .filter_map(|item| item.as_str())
                    .filter_map(|s| s.split('=').next())
                    .filter(|k| !k.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Renders a Compose service `volumes` entry: short string form as-is, long
/// mapping form as `source:target` (or just `target` for anonymous volumes).
fn volume_to_string(value: &serde_yaml_ng::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if value.is_mapping() {
        let source = value.get("source").and_then(|v| v.as_str());
        let target = value.get("target").and_then(|v| v.as_str());
        return match (source, target) {
            (Some(s), Some(t)) => Some(format!("{s}:{t}")),
            (None, Some(t)) => Some(t.to_string()),
            _ => None,
        };
    }
    None
}

fn dockerfile_tags(stages: &[DockerStage]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();
    for stage in stages {
        if let Some(label) = base_image_tag(&stage.base_image) {
            push_tag(&mut tags, label, "dockerfile-from");
        }
    }
    if stages.len() > 1 {
        push_tag(&mut tags, "docker-multi-stage", "dockerfile-stages");
    }
    tags
}

fn compose_tags(services: &[DockerService]) -> Vec<SemanticTag> {
    let mut tags = Vec::new();
    for service in services {
        if let Some(image) = &service.image {
            if let Some(label) = service_image_tag(image) {
                push_tag(&mut tags, label, "compose-service");
            }
        }
    }
    tags
}

fn push_tag(tags: &mut Vec<SemanticTag>, label: &str, rule: &str) {
    if tags.iter().any(|t| t.label == label) {
        return;
    }
    tags.push(SemanticTag {
        label: label.to_string(),
        source: TagSource::Heuristic {
            rule: rule.to_string(),
        },
        confidence: Confidence::High,
    });
}

/// Maps a Dockerfile base image to a runtime tag. Image references that point
/// at an earlier build stage simply return `None`.
fn base_image_tag(image: &str) -> Option<&'static str> {
    if image.contains("distroless") {
        return Some("base-distroless");
    }
    Some(match canonical_image_name(image).as_str() {
        "node" => "base-node",
        "python" => "base-python",
        "golang" | "go" => "base-go",
        "rust" => "base-rust",
        "php" | "php-fpm" | "php-apache" => "base-php",
        "ruby" => "base-ruby",
        "openjdk" | "eclipse-temurin" | "temurin" | "amazoncorretto" => "base-java",
        "dotnet" | "aspnet" | "sdk" => "base-dotnet",
        "nginx" => "base-nginx",
        "httpd" => "base-apache",
        "caddy" => "base-caddy",
        "alpine" => "base-alpine",
        "debian" => "base-debian",
        "ubuntu" => "base-ubuntu",
        "busybox" => "base-busybox",
        "scratch" => "base-scratch",
        _ => return None,
    })
}

/// Maps a Compose service image to a backing-service tag.
fn service_image_tag(image: &str) -> Option<&'static str> {
    Some(match canonical_image_name(image).as_str() {
        "postgres" | "postgis" => "service-postgres",
        "mysql" => "service-mysql",
        "mariadb" => "service-mariadb",
        "redis" => "service-redis",
        "mongo" | "mongodb" => "service-mongodb",
        "rabbitmq" => "service-rabbitmq",
        "nginx" => "service-nginx",
        "memcached" => "service-memcached",
        "elasticsearch" => "service-elasticsearch",
        "opensearch" => "service-opensearch",
        "kafka" => "service-kafka",
        "zookeeper" => "service-zookeeper",
        "minio" => "service-minio",
        "clickhouse" => "service-clickhouse",
        "cassandra" => "service-cassandra",
        "traefik" => "service-traefik",
        "vault" => "service-vault",
        "prometheus" => "service-prometheus",
        "grafana" => "service-grafana",
        _ => return None,
    })
}

/// Reduces an image reference to its short name: strips the registry host and
/// path, the `:tag`, and the `@digest`. `mcr.microsoft.com/dotnet/sdk:8.0`
/// becomes `sdk`; `library/postgres:16` becomes `postgres`.
fn canonical_image_name(image: &str) -> String {
    let no_digest = image.split('@').next().unwrap_or(image);
    let last_segment_start = no_digest.rfind('/').map(|i| i + 1).unwrap_or(0);
    let last = &no_digest[last_segment_start..];
    let no_tag = last.split(':').next().unwrap_or(last);
    no_tag.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_docker_filenames() {
        assert!(is_dockerfile("Dockerfile"));
        assert!(is_dockerfile("Dockerfile.dev"));
        assert!(is_dockerfile("api.dockerfile"));
        assert!(is_dockerfile("Containerfile"));
        assert!(!is_dockerfile("dockerfile.md"));

        assert!(is_compose("docker-compose.yml"));
        assert!(is_compose("docker-compose.override.yaml"));
        assert!(is_compose("compose.yaml"));
        assert!(!is_compose("composer.yaml"));
        assert!(!is_compose("config.yml"));
    }

    #[test]
    fn extracts_env_keys_both_forms() {
        assert_eq!(env_keys_from("KEY=value"), vec!["KEY"]);
        assert_eq!(env_keys_from("A=1 B=2"), vec!["A", "B"]);
        // Legacy `ENV KEY value` form: only the first token is the key.
        assert_eq!(env_keys_from("KEY some long value"), vec!["KEY"]);
    }

    #[test]
    fn renders_exec_and_shell_forms() {
        assert_eq!(
            exec_or_shell(r#"["nginx", "-g", "daemon off;"]"#),
            "nginx -g daemon off;"
        );
        assert_eq!(
            exec_or_shell("nginx -g 'daemon off;'"),
            "nginx -g 'daemon off;'"
        );
        assert_eq!(exec_or_list(r#"["/a", "/b"]"#), vec!["/a", "/b"]);
    }

    #[test]
    fn canonicalizes_image_references() {
        assert_eq!(canonical_image_name("node:18-alpine"), "node");
        assert_eq!(canonical_image_name("library/postgres:16"), "postgres");
        assert_eq!(
            canonical_image_name("mcr.microsoft.com/dotnet/sdk:8.0"),
            "sdk"
        );
        assert_eq!(canonical_image_name("registry:5000/app:1.2"), "app");
        assert_eq!(canonical_image_name("redis@sha256:abc"), "redis");
    }

    #[test]
    fn parses_multi_stage_from_and_expose() {
        let text = "\
# build stage
FROM --platform=$BUILDPLATFORM node:20 AS build
WORKDIR /app
RUN echo hi \\
    && echo done
FROM nginx:1.27
EXPOSE 80 443/tcp
";
        let lines = logical_lines(text);
        let mut stages = Vec::new();
        let mut ports: Vec<String> = Vec::new();
        for line in lines {
            let mut t = line.split_whitespace();
            match t.next().unwrap().to_ascii_uppercase().as_str() {
                "FROM" => {
                    let rest: Vec<&str> = t.collect();
                    let mut i = 0;
                    while rest[i].starts_with("--") {
                        i += 1;
                    }
                    let name = match rest.get(i + 1) {
                        Some(k) if k.eq_ignore_ascii_case("as") => {
                            rest.get(i + 2).map(|s| s.to_string())
                        }
                        _ => None,
                    };
                    stages.push((rest[i].to_string(), name));
                }
                "EXPOSE" => ports.extend(t.map(String::from)),
                _ => {}
            }
        }
        assert_eq!(stages.len(), 2);
        assert_eq!(
            stages[0],
            ("node:20".to_string(), Some("build".to_string()))
        );
        assert_eq!(stages[1].0, "nginx:1.27");
        assert_eq!(ports, vec!["80", "443/tcp"]);

        let tags = dockerfile_tags(
            &stages
                .iter()
                .map(|(b, n)| DockerStage {
                    base_image: b.clone(),
                    name: n.clone(),
                })
                .collect::<Vec<_>>(),
        );
        let labels: Vec<&str> = tags.iter().map(|t| t.label.as_str()).collect();
        assert!(labels.contains(&"base-node"));
        assert!(labels.contains(&"base-nginx"));
        assert!(labels.contains(&"docker-multi-stage"));
    }
}
