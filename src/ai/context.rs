use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_SECTION_CHARS: usize = 6_000;
const MAX_LOG_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug)]
pub struct ContextRequest {
    pub cwd: PathBuf,
    pub explicit_file: Option<PathBuf>,
    pub primary_input_present: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ContextBundle {
    pub display_messages: Vec<String>,
    pub prompt_block: String,
}

pub fn gather_context(request: &ContextRequest) -> ContextBundle {
    let collectors: [&dyn ContextCollector; 5] = [
        &EnvContextCollector,
        &ProjectContextCollector,
        &GitContextCollector,
        &LogContextCollector,
        &FileContextCollector,
    ];

    let mut sections = Vec::new();
    let mut display_messages = vec!["Gathering context...".to_string()];
    for collector in collectors {
        if let Some(section) = collector.collect(request, &mut display_messages) {
            sections.push(section);
        }
    }

    let prompt_block = if sections.is_empty() {
        String::new()
    } else {
        let mut prompt = String::from("---\nContext:\n\n");
        for section in sections {
            prompt.push_str(&section.title);
            prompt.push_str(":\n");
            prompt.push_str(section.body.trim_end());
            prompt.push_str("\n\n");
        }
        prompt.push_str("---");
        prompt
    };

    ContextBundle {
        display_messages,
        prompt_block,
    }
}

trait ContextCollector {
    fn collect(&self, request: &ContextRequest, display_messages: &mut Vec<String>) -> Option<ContextSection>;
}

struct ContextSection {
    title: &'static str,
    body: String,
}

struct EnvContextCollector;
struct ProjectContextCollector;
struct GitContextCollector;
struct LogContextCollector;
struct FileContextCollector;

impl ContextCollector for EnvContextCollector {
    fn collect(&self, request: &ContextRequest, display_messages: &mut Vec<String>) -> Option<ContextSection> {
        let shell = env::var("SHELL")
            .or_else(|_| env::var("ComSpec"))
            .unwrap_or_else(|_| "unknown".to_string());
        let body = format!(
            "Current working directory: {}\nOS: {}\nShell: {}",
            request.cwd.display(),
            env::consts::OS,
            shell
        );
        display_messages.push("✓ Including environment info".to_string());
        Some(ContextSection {
            title: "Environment",
            body,
        })
    }
}

impl ContextCollector for ProjectContextCollector {
    fn collect(&self, request: &ContextRequest, display_messages: &mut Vec<String>) -> Option<ContextSection> {
        let project = detect_project_type(&request.cwd);
        let mut lines = vec![format!("Detected project type: {}", project.as_str())];
        if let Some(toolchain) = detect_toolchain_details(&request.cwd, project) {
            lines.push(toolchain);
        }
        display_messages.push(format!("✓ Detected project: {}", project.as_str()));
        Some(ContextSection {
            title: "Project",
            body: lines.join("\n"),
        })
    }
}

impl ContextCollector for GitContextCollector {
    fn collect(&self, request: &ContextRequest, display_messages: &mut Vec<String>) -> Option<ContextSection> {
        let git_root = run_command(&request.cwd, &["git", "rev-parse", "--show-toplevel"]).ok()?;
        let branch = run_command(&request.cwd, &["git", "rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "unknown".to_string());
        let status = run_command(&request.cwd, &["git", "status", "--short"])
            .unwrap_or_default();
        let diff = run_command(
            &request.cwd,
            &["git", "diff", "--stat", "--compact-summary", "HEAD"],
        )
        .unwrap_or_default();

        let mut body = format!("Repository root: {git_root}\nBranch: {branch}");
        if !status.trim().is_empty() {
            body.push_str("\n\nGit status:\n");
            body.push_str(&truncate_chars(&status, MAX_SECTION_CHARS / 2));
        }
        if !diff.trim().is_empty() {
            body.push_str("\n\nRecent changes:\n");
            body.push_str(&truncate_chars(&diff, MAX_SECTION_CHARS / 2));
        }

        display_messages.push("✓ Found git repo".to_string());
        if !diff.trim().is_empty() || !status.trim().is_empty() {
            display_messages.push("✓ Including recent changes".to_string());
        }
        Some(ContextSection {
            title: "Git",
            body,
        })
    }
}

impl ContextCollector for LogContextCollector {
    fn collect(&self, request: &ContextRequest, display_messages: &mut Vec<String>) -> Option<ContextSection> {
        if request.primary_input_present {
            return None;
        }

        let path = find_recent_log_file(&request.cwd)?;
        let bytes = fs::read(&path).ok()?;
        if bytes.iter().any(|byte| *byte == 0) {
            return None;
        }
        let visible = if bytes.len() > MAX_LOG_BYTES {
            &bytes[bytes.len() - MAX_LOG_BYTES..]
        } else {
            &bytes[..]
        };
        let content = String::from_utf8_lossy(visible).to_string();
        if content.trim().is_empty() {
            return None;
        }

        display_messages.push(format!("✓ Including recent log: {}", display_path(&path)));
        Some(ContextSection {
            title: "Recent logs",
            body: format!(
                "File: {}\n\n{}",
                display_path(&path),
                truncate_chars(content.trim(), MAX_SECTION_CHARS)
            ),
        })
    }
}

impl ContextCollector for FileContextCollector {
    fn collect(&self, request: &ContextRequest, display_messages: &mut Vec<String>) -> Option<ContextSection> {
        let path = request.explicit_file.as_ref()?;
        let metadata = fs::metadata(path).ok()?;
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("unknown");
        let mut body = format!(
            "Primary file: {}\nExtension: {}\nSize: {} bytes",
            display_path(path),
            extension,
            metadata.len()
        );

        if let Some(parent) = path.parent() {
            let siblings = sibling_files(parent, path);
            if !siblings.is_empty() {
                body.push_str("\nNearby files:");
                for sibling in siblings {
                    body.push_str(&format!("\n- {sibling}"));
                }
            }
        }

        display_messages.push(format!("✓ Including file context: {}", display_path(path)));
        Some(ContextSection {
            title: "File metadata",
            body,
        })
    }
}

#[derive(Clone, Copy)]
enum ProjectType {
    Rust,
    Swift,
    Node,
    Python,
    Go,
    Unknown,
}

impl ProjectType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Swift => "Swift",
            Self::Node => "Node",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Unknown => "Unknown",
        }
    }
}

fn detect_project_type(cwd: &Path) -> ProjectType {
    if cwd.join("Cargo.toml").exists() {
        ProjectType::Rust
    } else if cwd.join("Package.json").exists() || cwd.join("package.json").exists() {
        ProjectType::Node
    } else if cwd.join("Package.swift").exists() || cwd.join("project.yml").exists() {
        ProjectType::Swift
    } else if cwd.join("pyproject.toml").exists() || cwd.join("requirements.txt").exists() {
        ProjectType::Python
    } else if cwd.join("go.mod").exists() {
        ProjectType::Go
    } else {
        ProjectType::Unknown
    }
}

fn detect_toolchain_details(cwd: &Path, project: ProjectType) -> Option<String> {
    match project {
        ProjectType::Rust => Some("Toolchain markers: Cargo.toml".to_string()),
        ProjectType::Swift => Some("Toolchain markers: Package.swift / Xcode project".to_string()),
        ProjectType::Node => Some("Toolchain markers: package.json".to_string()),
        ProjectType::Python => Some("Toolchain markers: pyproject.toml / requirements.txt".to_string()),
        ProjectType::Go => Some("Toolchain markers: go.mod".to_string()),
        ProjectType::Unknown => {
            let entries = fs::read_dir(cwd).ok()?;
            let mut hints = Vec::new();
            for entry in entries.flatten().take(8) {
                let name = entry.file_name().to_string_lossy().to_string();
                hints.push(name);
            }
            if hints.is_empty() {
                None
            } else {
                Some(format!("Top-level files: {}", hints.join(", ")))
            }
        }
    }
}

fn find_recent_log_file(cwd: &Path) -> Option<PathBuf> {
    let git_root = run_command(cwd, &["git", "rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from);
    let mut candidates = Vec::new();
    visit_dirs(cwd, 0, &mut |path| {
        let name = path.file_name().and_then(|v| v.to_str()).unwrap_or_default();
        let ext = path.extension().and_then(|v| v.to_str()).unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        if git_root
            .as_deref()
            .is_some_and(|root| git_ignored(root, path))
        {
            return;
        }
        if ext.eq_ignore_ascii_case("log") || lower.contains("log") {
            candidates.push(path.to_path_buf());
        }
    });

    candidates.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    });
    candidates.pop()
}

fn git_ignored(git_root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(git_root) else {
        return false;
    };
    Command::new("git")
        .arg("check-ignore")
        .arg(relative)
        .current_dir(git_root)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn visit_dirs(dir: &Path, depth: usize, on_file: &mut dyn FnMut(&Path)) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|v| v.to_str()).unwrap_or_default();
            if name == ".git" || name == "target" || name == "node_modules" {
                continue;
            }
            visit_dirs(&path, depth + 1, on_file);
        } else {
            on_file(&path);
        }
    }
}

fn sibling_files(dir: &Path, primary: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path == primary || !path.is_file() {
                return None;
            }
            Some(entry.file_name().to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    names.sort();
    names.truncate(6);
    names
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let truncated = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        format!("{truncated}\n... [truncated]")
    } else {
        truncated
    }
}

fn display_path(path: &Path) -> String {
    env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(PathBuf::from))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

fn run_command(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let (program, rest) = args.split_first().ok_or_else(|| "Missing command".to_string())?;
    let output = Command::new(program)
        .args(rest)
        .current_dir(cwd)
        .output()
        .map_err(|err| format!("Failed to run {}: {err}", program))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
