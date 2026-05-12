// Copyright 2026 Horizon LLC
// SPDX-License-Identifier: Apache-2.0

//! Output formatting for Squick.
//!
//! Two formats are first-class: markdown for `.squick/context.md`, the
//! universal artifact every AI agent can read; and JSON for programmatic
//! consumers such as the VS Code extension and custom agents.

pub mod json;
pub mod markdown;
pub mod schemas;

pub use json::format_json;
pub use markdown::format_markdown;
pub use schemas::format_schemas;
