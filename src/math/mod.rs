use std::path::{Path, PathBuf};
use std::process::Command;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use terminal_info::ai::chat::ProviderKind;

pub mod ast;
mod explain;
mod graph;
mod input;
mod parser;
mod render;
mod repl;
mod symbolic;

use ast::MathDocument;
use graph::build_dependency_graph;
use input::load_math_input;
use parser::parse_document;
use render::{
    ReportTheme, render_graph_text, render_html, render_markdown, render_solve, write_or_print,
};
use symbolic::{SolveOptions, solve_document};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum MathThemeArg {
    Dark,
    Light,
    System,
}

impl From<MathThemeArg> for ReportTheme {
    fn from(value: MathThemeArg) -> Self {
        match value {
            MathThemeArg::Dark => Self::Dark,
            MathThemeArg::Light => Self::Light,
            MathThemeArg::System => Self::System,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    Student,
    Developer,
    Researcher,
}

impl Audience {
    pub fn label(self) -> &'static str {
        match self {
            Self::Student => "student",
            Self::Developer => "developer",
            Self::Researcher => "researcher",
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplainStyle {
    Concise,
    Educational,
    Formal,
}

impl ExplainStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Concise => "concise",
            Self::Educational => "educational",
            Self::Formal => "formal",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SolveArgs {
    pub input: Option<PathBuf>,
    pub steps: bool,
    pub latex: bool,
    pub precision: usize,
    pub solve_for: Option<String>,
    pub vars: bool,
    pub trace: bool,
    pub debug: bool,
}

#[derive(Clone, Debug)]
pub struct GraphArgs {
    pub input: Option<PathBuf>,
    pub html: bool,
    pub mermaid: bool,
    pub dot: bool,
    pub open: bool,
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct RenderArgs {
    pub input: Option<PathBuf>,
    pub html: bool,
    pub pdf: bool,
    pub md: bool,
    pub theme: MathThemeArg,
    pub compact: bool,
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ExplainArgs {
    pub input: Option<PathBuf>,
    pub ai: bool,
    pub audience: Audience,
    pub style: ExplainStyle,
    pub html: bool,
    pub pdf: bool,
    pub model: Option<ProviderKind>,
    pub output: Option<PathBuf>,
}

pub fn handle_solve(args: SolveArgs) -> Result<(), String> {
    let (document, output) = load_and_solve(
        args.input,
        SolveOptions {
            precision: args.precision,
            solve_for: args.solve_for,
            include_steps: args.steps,
            include_trace: args.trace,
            debug: args.debug,
        },
    )?;
    let mut rendered = render_solve(&output, args.vars, args.latex)?;
    if args.steps && !output.steps.is_empty() {
        rendered.push_str(&format!("Steps\n{}\n", output.steps.join("\n")));
    }
    if args.trace && !output.trace.is_empty() {
        rendered.push_str(&format!("Trace\n{}\n", output.trace.join("\n")));
    }
    if args.debug {
        rendered.push_str(&format!(
            "Debug\nstatements={} source={}\n",
            document.statements.len(),
            document.source_format.label()
        ));
    }
    print!("{rendered}");
    Ok(())
}

pub fn handle_graph(args: GraphArgs) -> Result<(), String> {
    let input = load_math_input(args.input)?;
    let document = parse_document(&input.content, input.format)?;
    let graph = build_dependency_graph(&document);
    let content = if args.html {
        graph_html(&graph)
    } else if args.dot {
        render_graph_text(&graph, args.dot)
    } else if args.mermaid || !args.dot {
        render_graph_text(&graph, false)
    } else {
        render_graph_text(&graph, false)
    };
    write_or_print(&content, args.output.as_deref())?;
    if args.open {
        let path = args
            .output
            .as_deref()
            .ok_or_else(|| "--open requires --output.".to_string())?;
        open_file(path)?;
    }
    Ok(())
}

pub fn handle_render(args: RenderArgs) -> Result<(), String> {
    if args.pdf {
        return Err("PDF export is not supported yet. Generate HTML with --html.".to_string());
    }
    let (document, output) = load_and_solve(args.input, SolveOptions::default())?;
    let content = if args.html || !args.md {
        render_html(&document, &output, args.theme.into(), args.compact, None)
    } else {
        render_markdown(&document, &output, true)
    };
    write_or_print(&content, args.output.as_deref())
}

pub fn handle_explain(args: ExplainArgs) -> Result<(), String> {
    if args.pdf {
        return Err("PDF export is not supported yet. Generate HTML with --html.".to_string());
    }
    let (document, output) = load_and_solve(
        args.input,
        SolveOptions {
            include_steps: true,
            include_trace: true,
            ..SolveOptions::default()
        },
    )?;
    let write_to_file = args.output.is_some();
    let explanation = if args.ai {
        explain::ai_explanation(
            &document,
            &output,
            args.audience,
            args.style,
            args.model,
            !args.html && !write_to_file,
        )?
    } else {
        explain::deterministic_explanation(&document, &output, args.audience, args.style)
    };
    let content = if args.html {
        render_html(
            &document,
            &output,
            ReportTheme::System,
            false,
            Some(&explanation),
        )
    } else {
        explanation
    };
    if args.ai && !args.html && !write_to_file {
        return Ok(());
    }
    write_or_print(&content, args.output.as_deref())
}

pub fn run_repl() -> Result<(), String> {
    repl::run_repl()
}

fn load_and_solve(
    path: Option<PathBuf>,
    options: SolveOptions,
) -> Result<(MathDocument, symbolic::SolveOutput), String> {
    let input = load_math_input(path)?;
    let mut document = parse_document(&input.content, input.format)?;
    if document.statements.is_empty() {
        return Err(format!("No math statements found in {}.", input.origin));
    }
    let output = solve_document(&document, &options);
    document.diagnostics = output.diagnostics.clone();
    Ok((document, output))
}

fn graph_html(graph: &graph::DependencyGraph) -> String {
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Terminal Info Math Graph</title>
<script type="module">import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs'; mermaid.initialize({{startOnLoad:true}});</script>
<style>body{{font-family:ui-sans-serif,system-ui,sans-serif;margin:32px;}}main{{max-width:960px;margin:0 auto;}}.graph{{padding:18px;border:1px solid #ddd;border-radius:12px;}}</style>
</head><body><main><h1>Terminal Info Math Graph</h1><div class="mermaid graph">{}</div></main></body></html>"#,
        escape_html(&graph.mermaid())
    )
}

fn open_file(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg("start");
        cmd
    };

    command
        .arg(path)
        .status()
        .map_err(|err| format!("Failed to open '{}': {err}", path.display()))?;
    Ok(())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
