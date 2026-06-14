// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Name-based call site resolver. Prefers same-file matches; drops
//! cross-file calls with more than [`AMBIGUITY_THRESHOLD`] candidates.

use crate::types::{CallSite, Project, Reference};
use std::collections::HashMap;

const AMBIGUITY_THRESHOLD: usize = 3;

pub fn resolve_references(project: &mut Project) {
    let index = build_index(project);
    let calls = drain_call_sites(project);
    for (from_idx, call) in calls {
        let Some(targets) = index.get(&call.name) else {
            continue;
        };
        let chosen = select_targets(targets, from_idx, &call, project);
        for (target_idx, sym_idx) in chosen {
            let from_path = project.files[from_idx].path.clone();
            project.files[target_idx].symbols[sym_idx]
                .references
                .push(Reference {
                    from_file: from_path,
                    from_symbol: None,
                    line: call.line,
                });
        }
    }
}

fn build_index(project: &Project) -> HashMap<String, Vec<(usize, usize)>> {
    let mut index: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    for (file_idx, file) in project.files.iter().enumerate() {
        for (sym_idx, symbol) in file.symbols.iter().enumerate() {
            index
                .entry(symbol.name.clone())
                .or_default()
                .push((file_idx, sym_idx));
        }
    }
    index
}

fn drain_call_sites(project: &mut Project) -> Vec<(usize, CallSite)> {
    let mut calls = Vec::new();
    for (file_idx, file) in project.files.iter_mut().enumerate() {
        for cs in file.call_sites.drain(..) {
            calls.push((file_idx, cs));
        }
    }
    calls
}

fn select_targets(
    candidates: &[(usize, usize)],
    from_idx: usize,
    call: &CallSite,
    project: &Project,
) -> Vec<(usize, usize)> {
    let same_file: Vec<(usize, usize)> = candidates
        .iter()
        .copied()
        .filter(|&(fi, _)| fi == from_idx)
        .filter(|&(fi, si)| !is_definition_site(project, fi, si, from_idx, call))
        .collect();

    if !same_file.is_empty() {
        return same_file;
    }

    if candidates.len() > AMBIGUITY_THRESHOLD {
        return Vec::new();
    }

    candidates
        .iter()
        .copied()
        .filter(|&(fi, si)| !is_definition_site(project, fi, si, from_idx, call))
        .collect()
}

fn is_definition_site(
    project: &Project,
    target_file: usize,
    target_symbol: usize,
    from_file: usize,
    call: &CallSite,
) -> bool {
    if target_file != from_file {
        return false;
    }
    let target_line = project.files[target_file].symbols[target_symbol].line;
    target_line == call.line
}
