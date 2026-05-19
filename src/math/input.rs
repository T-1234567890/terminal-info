use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use super::ast::SourceFormat;

#[derive(Clone, Debug)]
pub struct MathInput {
    pub content: String,
    pub format: SourceFormat,
    pub origin: String,
}

pub fn load_math_input(path: Option<PathBuf>) -> Result<MathInput, String> {
    if let Some(path) = path {
        if !io::stdin().is_terminal() {
            return Err("Use either a math input file or piped stdin, not both.".to_string());
        }
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read math input '{}': {err}", path.display()))?;
        return Ok(MathInput {
            format: format_from_path(&path),
            origin: path.display().to_string(),
            content,
        });
    }

    if io::stdin().is_terminal() {
        return Err("Provide a math input file or pipe input on stdin.".to_string());
    }

    let mut content = String::new();
    io::stdin()
        .read_to_string(&mut content)
        .map_err(|err| format!("Failed to read stdin: {err}"))?;
    let format = if looks_like_latex(&content) {
        SourceFormat::Latex
    } else if content.trim_start().starts_with('{') || content.trim_start().starts_with('[') {
        SourceFormat::Json
    } else {
        SourceFormat::Text
    };
    Ok(MathInput {
        content,
        format,
        origin: "stdin".to_string(),
    })
}

fn format_from_path(path: &Path) -> SourceFormat {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "math" => SourceFormat::Math,
        "txt" => SourceFormat::Text,
        "json" => SourceFormat::Json,
        "tex" | "latex" => SourceFormat::Latex,
        _ => SourceFormat::Unknown,
    }
}

fn looks_like_latex(content: &str) -> bool {
    content.contains("\\[")
        || content.contains("\\]")
        || content.contains("$$")
        || content.contains("\\begin{equation")
}
