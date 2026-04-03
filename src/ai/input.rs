use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use crate::ai::connections::ConnectionConfig;

const MAX_FILE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct LoadedFileContext {
    pub display_name: String,
    pub size_bytes: usize,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessedChatInput {
    pub display_messages: Vec<String>,
    pub prompt: String,
}

pub fn read_piped_stdin() -> Result<Option<String>, String> {
    if io::stdin().is_terminal() {
        return Ok(None);
    }

    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|err| format!("Failed to read stdin: {err}"))?;
    if buffer.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(buffer))
}

pub fn build_stdin_analysis_prompt(
    stdin_input: &str,
    connection: Option<&ConnectionConfig>,
) -> ProcessedChatInput {
    let size_kb = stdin_input.len() as f64 / 1024.0;
    let kind = classify_stdin_input(stdin_input);
    let mut display_messages = vec![
        format!("Input detected ({kind}, {size_kb:.1}KB)"),
        "Analyzing...".to_string(),
    ];
    if let Some(connection) = connection {
        display_messages.push(format!("Attached connection: {}", connection.url));
    }

    let prompt = format!(
        "---\nUser input:\n\n{}\n\n---\n\nPlease explain what this is, identify any issues, and suggest fixes.",
        stdin_input.trim()
    );

    ProcessedChatInput {
        display_messages,
        prompt,
    }
}

fn classify_stdin_input(input: &str) -> &'static str {
    let sample = input.trim();
    let lower = sample.to_ascii_lowercase();
    let non_empty_lines = sample
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if looks_like_toml(&non_empty_lines)
        || looks_like_json(sample)
        || looks_like_yaml(&non_empty_lines)
    {
        return "config";
    }

    if lower.contains("traceback")
        || lower.contains("exception")
        || lower.contains("stack trace")
        || lower.contains("error:")
        || lower.contains("warn")
        || lower.contains("failed")
        || sample.lines().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with('[')
                || trimmed.starts_with("ERROR")
                || trimmed.starts_with("WARN")
                || trimmed.starts_with("INFO")
        })
    {
        return "log";
    }

    if lower.contains("fn ")
        || lower.contains("class ")
        || lower.contains("import ")
        || lower.contains("const ")
        || lower.contains("let ")
        || lower.contains("#include")
        || lower.contains("public static")
    {
        return "code";
    }

    "text"
}

fn looks_like_toml(lines: &[&str]) -> bool {
    if lines.is_empty() {
        return false;
    }

    let mut section_headers = 0usize;
    let mut assignments = 0usize;
    for line in lines.iter().take(40) {
        if line.starts_with('[') && line.ends_with(']') {
            section_headers += 1;
            continue;
        }
        if line.contains(" = ") || line.contains("=\"") || line.contains(" =\"") {
            assignments += 1;
        }
    }

    section_headers >= 1 || assignments >= 3
}

fn looks_like_json(sample: &str) -> bool {
    let trimmed = sample.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

fn looks_like_yaml(lines: &[&str]) -> bool {
    if lines.is_empty() {
        return false;
    }

    let mut mappings = 0usize;
    for line in lines.iter().take(40) {
        if line.starts_with("- ") || line.starts_with("---") {
            return true;
        }
        if line.contains(':') && !line.starts_with("http://") && !line.starts_with("https://") {
            mappings += 1;
        }
    }

    mappings >= 3
}

pub fn process_chat_input(
    input: &str,
    connection_name: Option<&str>,
    connection: Option<&ConnectionConfig>,
) -> Result<ProcessedChatInput, String> {
    let file_refs = extract_file_references(input);
    if file_refs.is_empty() {
        return Ok(ProcessedChatInput {
            display_messages: Vec::new(),
            prompt: input.trim().to_string(),
        });
    }

    let mut loaded = Vec::new();
    let mut display_messages = Vec::new();
    for reference in &file_refs {
        let file = load_file_context(reference)?;
        let size_kb = file.size_bytes as f64 / 1024.0;
        if file.truncated {
            display_messages.push(format!(
                "Loaded file: {} ({size_kb:.1} KB, truncated)",
                file.display_name
            ));
        } else {
            display_messages.push(format!("Loaded file: {} ({size_kb:.1} KB)", file.display_name));
        }
        loaded.push(file);
    }

    if let Some(name) = connection_name {
        display_messages.push(format!("Attached connection: {name}"));
    } else if let Some(conn) = connection {
        display_messages.push(format!("Attached connection: {}", conn.url));
    }

    let stripped_question = strip_file_references(input).trim().to_string();
    let question = if stripped_question.is_empty() {
        "Please analyze the referenced file(s).".to_string()
    } else {
        stripped_question
    };

    let mut prompt = String::new();
    for file in &loaded {
        prompt.push_str("---\n");
        prompt.push_str(&format!("File: {}\n\n", file.display_name));
        prompt.push_str(file.content.trim_end());
        prompt.push_str("\n\n");
    }
    prompt.push_str("---\n");
    prompt.push_str(&format!("User question:\n{}\n\n---", question));

    Ok(ProcessedChatInput {
        display_messages,
        prompt,
    })
}

fn extract_file_references(input: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for token in input.split_whitespace() {
        if let Some(reference) = token.strip_prefix('@') {
            let cleaned = reference
                .trim_matches(|ch: char| matches!(ch, '"' | '\'' | ',' | ';' | ')' | '('));
            if !cleaned.is_empty() {
                refs.push(cleaned.to_string());
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn strip_file_references(input: &str) -> String {
    input.split_whitespace()
        .filter(|token| !token.starts_with('@'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn load_file_context(reference: &str) -> Result<LoadedFileContext, String> {
    let path = resolve_reference_path(reference)?;
    let bytes = fs::read(&path)
        .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
    let truncated = bytes.len() > MAX_FILE_BYTES;
    let visible = if truncated {
        &bytes[..MAX_FILE_BYTES]
    } else {
        &bytes[..]
    };
    let content = String::from_utf8_lossy(visible).to_string();

    Ok(LoadedFileContext {
        display_name: display_path(&path),
        size_bytes: bytes.len(),
        content,
        truncated,
    })
}

fn resolve_reference_path(reference: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(reference);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|err| format!("Failed to resolve current directory: {err}"))?
            .join(path)
    };
    if !absolute.exists() {
        return Err(format!("Referenced file '{}' was not found.", reference));
    }
    if !absolute.is_file() {
        return Err(format!("Referenced path '{}' is not a file.", reference));
    }
    Ok(absolute)
}

fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(PathBuf::from))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}
