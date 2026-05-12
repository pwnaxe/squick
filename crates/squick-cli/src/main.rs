// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

mod mcp;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use squick_core::{Project, ScanOptions, Scanner};
use squick_dict::{load_directory, Matcher};
use std::path::{Path, PathBuf};

const DEFAULT_DICTIONARY_DIR: &str = "dictionaries";

#[derive(Parser)]
#[command(name = "squick", version, about = "Pre-computed LLM context for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// One-shot scan; writes context to `.squick/context.<fmt>`.
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
        } => cmd_scan(
            &root,
            format,
            out.as_deref(),
            dict_dir.as_deref(),
            &ScanFilters {
                includes,
                excludes,
                no_schemas,
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
    eprintln!("squick: wrote {}", out_path.display());

    if matches!(format, OutputFormat::Markdown) && out.is_none() && !filters.no_schemas {
        if let Some(schemas) = squick_format::format_schemas(&project) {
            let schemas_path = root.join(".squick").join("schemas.md");
            std::fs::write(&schemas_path, schemas)?;
            eprintln!("squick: wrote {}", schemas_path.display());
        }
    }

    Ok(())
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
/// default location. Skips silently if no dictionary directory is found,
/// so `squick scan` works on bare projects.
fn apply_dictionaries(project: &mut Project, dict_dir: Option<&Path>) -> Result<()> {
    let resolved = match dict_dir {
        Some(p) => Some(p.to_path_buf()),
        None => default_dictionary_dir(),
    };
    let Some(path) = resolved else {
        return Ok(());
    };
    if !path.exists() {
        return Ok(());
    }
    let dicts = load_directory(&path)
        .with_context(|| format!("loading dictionaries from {}", path.display()))?;
    if dicts.is_empty() {
        return Ok(());
    }
    let matcher = Matcher::from_dictionaries(dicts);
    matcher.apply(project);
    Ok(())
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
