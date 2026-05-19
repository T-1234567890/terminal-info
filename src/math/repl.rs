use std::io::{self, Write};

use super::ast::{MathDocument, SourceFormat};
use super::graph::build_dependency_graph;
use super::parser::parse_document;
use super::render::render_solve;
use super::symbolic::{SolveOptions, solve_document};

pub fn run_repl() -> Result<(), String> {
    let mut lines = Vec::new();
    println!("Terminal Info Math REPL. Type exit, clear, solve, vars, graph, or latex.");
    loop {
        print!("math> ");
        io::stdout()
            .flush()
            .map_err(|err| format!("Failed to flush prompt: {err}"))?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|err| format!("Failed to read REPL input: {err}"))?;
        let input = input.trim();
        match input {
            "" => continue,
            "exit" | "quit" => return Ok(()),
            "clear" => {
                lines.clear();
                println!("Cleared math context.");
            }
            "solve" => {
                let doc = context_document(&lines)?;
                let output = solve_document(&doc, &SolveOptions::default());
                print!("{}", render_solve(&output, true, false)?);
            }
            "vars" => {
                let doc = context_document(&lines)?;
                let graph = build_dependency_graph(&doc);
                for variable in graph.variables {
                    println!("{} -> {}", variable.name, variable.depends_on.join(", "));
                }
            }
            "graph" => {
                let doc = context_document(&lines)?;
                println!("{}", build_dependency_graph(&doc).mermaid());
            }
            "latex" => {
                let doc = context_document(&lines)?;
                for statement in doc.statements {
                    println!("{}", statement.to_latex());
                }
            }
            _ if input.starts_with("solve ") => {
                lines.push(input.trim_start_matches("solve ").trim().to_string());
                let doc = context_document(&lines)?;
                let output = solve_document(&doc, &SolveOptions::default());
                print!("{}", render_solve(&output, true, false)?);
            }
            _ => {
                parse_document(input, SourceFormat::Math)?;
                lines.push(input.to_string());
            }
        }
    }
}

fn context_document(lines: &[String]) -> Result<MathDocument, String> {
    parse_document(&lines.join("\n"), SourceFormat::Math)
}
