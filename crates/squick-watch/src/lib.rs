// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Debounced file watcher.
//!
//! Invokes a callback with each batch of changed paths after a debounce
//! window. The watcher does no scanning; the consumer decides what to
//! re-scan, which keeps semantics in `squick-core`.

use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct WatchOptions {
    pub debounce: Duration,
    pub recursive: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(250),
            recursive: true,
        }
    }
}

pub fn watch<F>(root: &Path, options: WatchOptions, mut on_change: F) -> Result<()>
where
    F: FnMut(Vec<PathBuf>) + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<DebounceEventResult>();
    let mut debouncer = new_debouncer(options.debounce, move |res| {
        let _ = tx.send(res);
    })?;

    let mode = if options.recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    debouncer.watcher().watch(root, mode)?;

    while let Ok(events) = rx.recv() {
        match events {
            Ok(events) => {
                let paths = events.into_iter().map(|e| e.path).collect::<Vec<_>>();
                if !paths.is_empty() {
                    on_change(paths);
                }
            }
            Err(err) => eprintln!("squick-watch: {err}"),
        }
    }
    Ok(())
}
