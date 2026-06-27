// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

use squick_core::Project;
use std::fmt::Write;

pub fn format_markdown(project: &Project) -> String {
    let endpoint_count: usize = project.files.iter().map(|f| f.endpoints.len()).sum();
    let schema_count = project.strapi_schemas.len();

    let mut out = String::new();
    let _ = writeln!(out, "# Squick context index");
    let _ = writeln!(
        out,
        "\nRoot: `{}` ({} files scanned).\n",
        project.root.display(),
        project.files.len()
    );

    let _ = writeln!(out, "## When asking an AI a question about this project");
    let _ = writeln!(out, "\nAttach one or both of these files to your chat:\n");
    let _ = writeln!(
        out,
        "- **`conventions.md`** - stack, library choices, repository layout. \
         Use this for any \"how is X organised\" or \"what does this project use for Y\" \
         question."
    );
    if schema_count > 0 || endpoint_count > 0 {
        let _ = writeln!(
            out,
            "- **`schemas.md`** - {} HTTP endpoint(s){}{}. Attach this for backend, \
             data model, or API questions.",
            endpoint_count,
            if schema_count > 0 { " and " } else { "" },
            if schema_count > 0 {
                format!("{schema_count} content schema(s)")
            } else {
                String::new()
            }
        );
    }

    let _ = writeln!(out, "\n## Tool-only artifacts");
    let _ = writeln!(
        out,
        "\nGenerated only with `squick scan --full`. Designed for MCP servers and \
         scripts that parse Squick data programmatically, not for chat attachment:\n"
    );
    let _ = writeln!(out, "- `context.txt` - compact columnar facts (AI-primary): one `@type` header per record kind, then TAB-delimited rows. Densest format; lowest token cost.");
    let _ = writeln!(out, "- `context.ndjson` - the same facts as JSON, one per line (project, file, symbol, reference, endpoint, schema, container).");
    let _ = writeln!(
        out,
        "- `graph.txt` - subject-predicate-object triples for graph traversal."
    );

    out
}
