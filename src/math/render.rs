use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::output::{OutputMode, json_output, output_mode};
use crate::theme::format_box_table;

use super::ast::MathDocument;
use super::graph::DependencyGraph;
use super::symbolic::SolveOutput;

#[derive(Clone, Copy, Debug)]
pub enum ReportTheme {
    Dark,
    Light,
    System,
}

impl ReportTheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }
}

pub fn render_solve(output: &SolveOutput, show_vars: bool, latex: bool) -> Result<String, String> {
    if json_output() {
        return json_string(output);
    }
    if latex {
        return Ok(output
            .results
            .iter()
            .map(|result| result.latex.clone())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n");
    }

    match output_mode() {
        OutputMode::Compact => Ok(render_compact(output)),
        OutputMode::Plain | OutputMode::Color => {
            let mut rows = Vec::new();
            rows.push(("Status".to_string(), output.status.clone()));
            if let Some(result) = output.results.last() {
                rows.push(("Expression".to_string(), result.expression.clone()));
                if let Some(value) = &result.value {
                    rows.push(("Result".to_string(), value.clone()));
                }
            }
            if show_vars {
                rows.push((
                    "Variables".to_string(),
                    output
                        .variables
                        .iter()
                        .map(|var| var.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
            for diagnostic in &output.diagnostics {
                rows.push(("Diagnostic".to_string(), diagnostic.message.clone()));
            }
            Ok(format_box_table("Terminal Info Math", &rows))
        }
    }
}

pub fn render_graph_text(graph: &DependencyGraph, dot: bool) -> String {
    if dot { graph.dot() } else { graph.mermaid() }
}

pub fn render_markdown(
    document: &MathDocument,
    output: &SolveOutput,
    include_trace: bool,
) -> String {
    let mut md = String::new();
    md.push_str("# Terminal Info Math Report\n\n");
    md.push_str(&format!("Status: `{}`\n\n", output.status));
    md.push_str("## Input\n\n");
    for statement in &document.statements {
        md.push_str(&format!("- `{statement}`\n"));
    }
    md.push_str("\n## Results\n\n");
    for result in &output.results {
        let expression = result
            .symbol
            .as_ref()
            .map(|symbol| format!("{symbol} = {}", result.expression))
            .unwrap_or_else(|| result.expression.clone());
        if let Some(value) = &result.value {
            md.push_str(&format!("- `{expression}` -> `{value}`\n"));
        } else {
            md.push_str(&format!("- `{expression}`\n"));
        }
    }
    md.push_str("\n## Variables\n\n");
    for variable in &output.variables {
        let depends_on = if variable.depends_on.is_empty() {
            "none".to_string()
        } else {
            variable.depends_on.join(", ")
        };
        md.push_str(&format!(
            "- `{}` depends on `{}`\n",
            variable.name, depends_on
        ));
    }
    if !output.assumptions.is_empty() {
        md.push_str("\n## Assumptions\n\n");
        for assumption in &output.assumptions {
            md.push_str(&format!(
                "- `{}` `{}`\n",
                assumption.variable, assumption.condition
            ));
        }
    }
    if include_trace && !output.trace.is_empty() {
        md.push_str("\n## Trace\n\n");
        for step in &output.trace {
            md.push_str(&format!("- `{step}`\n"));
        }
    }
    if !output.diagnostics.is_empty() {
        md.push_str("\n## Diagnostics\n\n");
        for diagnostic in &output.diagnostics {
            md.push_str(&format!(
                "- {:?}: {}\n",
                diagnostic.level, diagnostic.message
            ));
        }
    }
    md
}

pub fn render_html(
    document: &MathDocument,
    output: &SolveOutput,
    theme: ReportTheme,
    compact: bool,
    ai_explanation: Option<&str>,
) -> String {
    let trace = output
        .trace
        .iter()
        .map(|line| format!("<li><code>{}</code></li>", escape_html(line)))
        .collect::<Vec<_>>()
        .join("");
    let results = output
        .results
        .iter()
        .map(|result| {
            format!(
                "<li><code>{}</code>{}</li>",
                escape_html(&result.expression),
                result
                    .value
                    .as_ref()
                    .map(|value| format!(" = <code>{}</code>", escape_html(value)))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let statements = document
        .statements
        .iter()
        .map(|statement| {
            format!(
                "<li><code>{}</code></li>",
                escape_html(&statement.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let ai = ai_explanation
        .map(|text| {
            format!(
                "<section><h2>Explanation</h2><p>{}</p></section>",
                escape_html(text)
            )
        })
        .unwrap_or_default();
    let max_width = if compact { "760px" } else { "960px" };

    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>Terminal Info Math Report</title>
<script type="module">import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs'; mermaid.initialize({{startOnLoad:true}});</script>
<script defer src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>
<style>
:root {{ color-scheme: {}; font-family: ui-sans-serif, system-ui, sans-serif; }}
body {{ margin: 0; padding: 32px; background: Canvas; color: CanvasText; }}
main {{ max-width: {}; margin: 0 auto; }}
code, pre {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }}
section {{ border-top: 1px solid color-mix(in srgb, CanvasText 18%, transparent); padding: 18px 0; }}
pre {{ overflow: auto; padding: 12px; background: color-mix(in srgb, CanvasText 8%, transparent); }}
.graph-panel {{ overflow: auto; padding: 16px; border: 1px solid color-mix(in srgb, CanvasText 18%, transparent); border-radius: 12px; background: color-mix(in srgb, CanvasText 4%, transparent); }}
</style>
</head>
<body>
<main>
<h1>Terminal Info Math Report</h1>
<p>Status: <strong>{}</strong></p>
<section><h2>Input</h2><ul>{}</ul></section>
<section><h2>Results</h2><ul>{}</ul></section>
<section><h2>Dependency Graph</h2><div class="mermaid graph-panel">{}</div></section>
<details><summary>Trace</summary><ul>{}</ul></details>
{}
</main>
</body>
</html>
"#,
        theme.label(),
        max_width,
        escape_html(&output.status),
        statements,
        results,
        escape_html(&output.graph.mermaid()),
        trace,
        ai
    )
}

pub fn write_or_print(content: &str, output: Option<&Path>) -> Result<(), String> {
    if let Some(path) = output {
        fs::write(path, content)
            .map_err(|err| format!("Failed to write '{}': {err}", path.display()))?;
        println!("Wrote {}.", path.display());
    } else {
        println!("{content}");
    }
    Ok(())
}

fn render_compact(output: &SolveOutput) -> String {
    let result = output
        .results
        .last()
        .and_then(|result| result.value.as_deref())
        .unwrap_or("none");
    let vars = output
        .variables
        .iter()
        .map(|var| var.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "status={} result=\"{}\" vars=\"{}\"\n",
        output.status, result, vars
    )
}

fn json_string<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value)
        .map(|json| format!("{json}\n"))
        .map_err(|err| format!("Failed to serialize math JSON output: {err}"))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
