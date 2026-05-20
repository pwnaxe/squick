// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Output emitters: NDJSON (LLM-primary), triples (graph), conventions
//! (architecture summary), markdown (human-readable summary), JSON
//! (full structured snapshot).

pub mod conventions;
pub mod json;
pub mod markdown;
pub mod ndjson;
pub mod schemas;
pub mod triples;

pub use conventions::format_conventions;
pub use json::format_json;
pub use markdown::format_markdown;
pub use ndjson::format_ndjson;
pub use schemas::format_schemas;
pub use triples::format_triples;
