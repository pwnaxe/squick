// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    Calls,
    Imports,
    Extends,
    Implements,
    References,
}

/// A symbol- and module-level call/dependency graph.
///
/// Nodes are addressed by a stable string id of the form
/// `path::to::file.ts#symbol_name`, or by the file path alone for
/// module-level edges.
#[derive(Debug, Default)]
pub struct CallGraph {
    graph: DiGraph<String, EdgeKind>,
    index: HashMap<String, NodeIndex>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_node(&mut self, id: impl Into<String>) -> NodeIndex {
        let id = id.into();
        if let Some(&n) = self.index.get(&id) {
            return n;
        }
        let n = self.graph.add_node(id.clone());
        self.index.insert(id, n);
        n
    }

    pub fn add_edge(&mut self, from: &str, to: &str, kind: EdgeKind) {
        let a = self.ensure_node(from);
        let b = self.ensure_node(to);
        self.graph.add_edge(a, b, kind);
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn inner(&self) -> &DiGraph<String, EdgeKind> {
        &self.graph
    }
}
