// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use squick_core::Project;

pub fn format_json(project: &Project) -> serde_json::Result<String> {
    serde_json::to_string_pretty(project)
}
