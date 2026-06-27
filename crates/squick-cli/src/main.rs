// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

mod mcp;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use include_dir::{include_dir, Dir};
use squick_core::{Project, ScanOptions, Scanner};
use squick_dict::{load_directory, load_str, Dictionary, Matcher};
use std::path::{Path, PathBuf};

const DEFAULT_DICTIONARY_DIR: &str = "dictionaries";

/// Dictionaries baked into the binary at build time. An on-disk directory
/// (`SQUICK_DICT_DIR`, `./dictionaries`, or next to the executable) still
/// wins when present, but this fallback lets a `cargo install`-ed binary
/// detect frameworks without the source tree alongside it.
static EMBEDDED_DICTIONARIES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../dictionaries");

#[derive(Parser)]
#[command(
    name = "squick",
    version,
    about = "Pre-computed LLM context for AI agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// One-shot scan; writes `.squick/conventions.md` + `.squick/schemas.md`.
    Scan {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        dict_dir: Option<PathBuf>,
        /// Glob patterns of files or directories to include. Repeatable.
        #[arg(long = "include")]
        includes: Vec<String>,
        /// Glob patterns of files or directories to exclude. Repeatable.
        #[arg(long = "exclude")]
        excludes: Vec<String>,
        /// Skip writing the auxiliary `.squick/schemas.md` file.
        #[arg(long)]
        no_schemas: bool,
        /// Also write the tool-only artifacts (`context.txt`,
        /// `context.ndjson`, `graph.txt`) alongside the chat-attachable ones.
        #[arg(long)]
        full: bool,
    },
    /// Watch the filesystem and rewrite context on change.
    Watch {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
        format: OutputFormat,
        #[arg(long)]
        dict_dir: Option<PathBuf>,
        #[arg(long = "include")]
        includes: Vec<String>,
        #[arg(long = "exclude")]
        excludes: Vec<String>,
        #[arg(long)]
        no_schemas: bool,
    },
    /// Initialize a `.squick/` directory in the current project.
    Init {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Run an MCP (Model Context Protocol) server on stdio.
    Mcp {
        #[arg(long)]
        dict_dir: Option<PathBuf>,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum OutputFormat {
    Markdown,
    Json,
}

#[derive(Clone, Default)]
struct ScanFilters {
    includes: Vec<String>,
    excludes: Vec<String>,
    no_schemas: bool,
    full: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan {
            root,
            format,
            out,
            dict_dir,
            includes,
            excludes,
            no_schemas,
            full,
        } => cmd_scan(
            &root,
            format,
            out.as_deref(),
            dict_dir.as_deref(),
            &ScanFilters {
                includes,
                excludes,
                no_schemas,
                full,
            },
        ),
        Command::Watch {
            root,
            format,
            dict_dir,
            includes,
            excludes,
            no_schemas,
        } => cmd_watch(
            &root,
            format,
            dict_dir.as_deref(),
            ScanFilters {
                includes,
                excludes,
                no_schemas,
                full: false,
            },
        ),
        Command::Init { root } => cmd_init(&root),
        Command::Mcp { dict_dir } => {
            let dict = dict_dir.or_else(default_dictionary_dir);
            mcp::run(dict)
        }
    }
}

fn build_scan_options(filters: &ScanFilters) -> ScanOptions {
    ScanOptions {
        includes: filters.includes.clone(),
        excludes: filters.excludes.clone(),
        ..ScanOptions::default()
    }
}

fn cmd_scan(
    root: &Path,
    format: OutputFormat,
    out: Option<&Path>,
    dict_dir: Option<&Path>,
    filters: &ScanFilters,
) -> Result<()> {
    let mut scanner = Scanner::new(build_scan_options(filters));
    let mut project = scanner
        .scan_project(root)
        .with_context(|| format!("scanning {}", root.display()))?;

    apply_dictionaries(&mut project, dict_dir)?;

    let body = match format {
        OutputFormat::Markdown => squick_format::format_markdown(&project),
        OutputFormat::Json => squick_format::format_json(&project)?,
    };

    let out_path = match out {
        Some(p) => p.to_path_buf(),
        None => {
            let dir = root.join(".squick");
            std::fs::create_dir_all(&dir)?;
            dir.join(match format {
                OutputFormat::Markdown => "context.md",
                OutputFormat::Json => "context.json",
            })
        }
    };
    std::fs::write(&out_path, body)?;

    if matches!(format, OutputFormat::Markdown) && out.is_none() {
        let squick_dir = root.join(".squick");
        std::fs::create_dir_all(&squick_dir)?;

        let conventions_path = squick_dir.join("conventions.md");
        std::fs::write(
            &conventions_path,
            squick_format::format_conventions(&project),
        )?;

        let schemas_written = if !filters.no_schemas {
            if let Some(schemas) = squick_format::format_schemas(&project) {
                let schemas_path = squick_dir.join("schemas.md");
                std::fs::write(&schemas_path, schemas)?;
                true
            } else {
                false
            }
        } else {
            false
        };

        if filters.full {
            let compact_path = squick_dir.join("context.txt");
            std::fs::write(&compact_path, squick_format::format_compact(&project))?;
            let ndjson_path = squick_dir.join("context.ndjson");
            std::fs::write(&ndjson_path, squick_format::format_ndjson(&project))?;
            let graph_path = squick_dir.join("graph.txt");
            std::fs::write(&graph_path, squick_format::format_triples(&project))?;
        }

        report_outputs(root, schemas_written, filters.full);
    } else {
        eprintln!("squick: wrote {}", out_path.display());
    }

    Ok(())
}

fn report_outputs(root: &Path, schemas_written: bool, full: bool) {
    let dir = root.join(".squick").display().to_string();
    eprintln!("squick: wrote {dir}/");
    eprintln!(
        "  conventions.md  - primary; attach to your AI chat for stack/architecture questions"
    );
    if schemas_written {
        eprintln!("  schemas.md      - attach to your AI chat for data/API questions");
    }
    eprintln!("  context.md      - index pointing at the files above");
    if full {
        eprintln!("  context.txt     - compact columnar facts (AI-primary, tool-only)");
        eprintln!("  context.ndjson  - structured facts as JSON (tool-only)");
        eprintln!("  graph.txt       - dependency triples (tool-only)");
    } else {
        eprintln!();
        eprintln!(
            "  Tip: re-run with --full to also emit context.txt, context.ndjson, and graph.txt"
        );
        eprintln!("       for MCP servers and scripts.");
    }
}

fn cmd_watch(
    root: &Path,
    format: OutputFormat,
    dict_dir: Option<&Path>,
    filters: ScanFilters,
) -> Result<()> {
    cmd_scan(root, format, None, dict_dir, &filters)?;
    let root_owned = root.to_path_buf();
    let dict_owned = dict_dir.map(|p| p.to_path_buf());
    let filters_owned = filters;
    squick_watch::watch(
        root,
        squick_watch::WatchOptions::default(),
        move |_changed| {
            if let Err(e) = cmd_scan(
                &root_owned,
                format,
                None,
                dict_owned.as_deref(),
                &filters_owned,
            ) {
                eprintln!("squick: rescan failed: {e}");
            }
        },
    )?;
    Ok(())
}

fn cmd_init(root: &Path) -> Result<()> {
    let dir = root.join(".squick");
    std::fs::create_dir_all(&dir)?;
    let placeholder = dir.join(".gitkeep");
    if !placeholder.exists() {
        std::fs::write(&placeholder, "")?;
    }
    eprintln!("squick: initialised {}", dir.display());
    Ok(())
}

/// Loads dictionaries from `dict_dir` if provided, otherwise from the
/// default location, falling back to the embedded set. Applies them to the
/// project so framework/file-role tags land on symbols and files.
fn apply_dictionaries(project: &mut Project, dict_dir: Option<&Path>) -> Result<()> {
    let dicts = resolve_dictionaries(dict_dir)?;
    if dicts.is_empty() {
        return Ok(());
    }
    Matcher::from_dictionaries(dicts).apply(project);
    Ok(())
}

/// Resolves the dictionaries to apply. An on-disk directory wins when it
/// exists and is non-empty (the development override); otherwise the
/// dictionaries embedded at build time are used so installed binaries are
/// self-contained.
pub(crate) fn resolve_dictionaries(dict_dir: Option<&Path>) -> Result<Vec<Dictionary>> {
    let on_disk = match dict_dir {
        Some(p) => Some(p.to_path_buf()),
        None => default_dictionary_dir(),
    };
    if let Some(path) = on_disk {
        if path.exists() {
            let dicts = load_directory(&path)
                .with_context(|| format!("loading dictionaries from {}", path.display()))?;
            if !dicts.is_empty() {
                return Ok(dicts);
            }
        }
    }
    Ok(embedded_dictionaries())
}

fn embedded_dictionaries() -> Vec<Dictionary> {
    let mut out = Vec::new();
    collect_embedded(&EMBEDDED_DICTIONARIES, &mut out);
    out
}

fn collect_embedded(dir: &Dir<'_>, out: &mut Vec<Dictionary>) {
    for file in dir.files() {
        let path = file.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yaml" && ext != "yml" {
            continue;
        }
        let Some(text) = file.contents_utf8() else {
            continue;
        };
        let name = path.with_extension("").to_string_lossy().replace('\\', "/");
        match load_str(&name, text) {
            Ok(dict) => out.push(dict),
            Err(e) => eprintln!("squick: skip embedded dictionary {name}: {e}"),
        }
    }
    for sub in dir.dirs() {
        collect_embedded(sub, out);
    }
}

fn default_dictionary_dir() -> Option<PathBuf> {
    if let Ok(env) = std::env::var("SQUICK_DICT_DIR") {
        let p = PathBuf::from(env);
        if p.exists() {
            return Some(p);
        }
    }
    let cwd = PathBuf::from(DEFAULT_DICTIONARY_DIR);
    if cwd.exists() {
        return Some(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join(DEFAULT_DICTIONARY_DIR);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}
