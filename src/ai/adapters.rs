use serde::Deserialize;

use crate::ai::agent::ApprovalKind;
use crate::config::AiAgentCliConfig;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AgentAdapterKind {
    Codex,
    ClaudeCode,
    Gemini,
    Generic,
}

impl AgentAdapterKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
            Self::Gemini => "gemini",
            Self::Generic => "generic",
        }
    }

    pub fn from_config(value: Option<&str>) -> Self {
        match value.unwrap_or("generic") {
            "codex" => Self::Codex,
            "claude_code" | "claude" => Self::ClaudeCode,
            "gemini" => Self::Gemini,
            _ => Self::Generic,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfiguredAgent {
    pub id: String,
    pub adapter: AgentAdapterKind,
    pub config: AiAgentCliConfig,
}

pub trait AgentAdapter: Send + Sync {
    fn kind(&self) -> AgentAdapterKind;
    fn display_name(&self) -> &'static str;
    fn default_command(&self) -> &'static str;
    fn supports_pause_resume(&self) -> bool {
        true
    }
    fn supports_local_intercept(&self) -> bool {
        false
    }
    fn parse_line(&self, line: &str) -> Vec<AdapterFrame>;
}

#[derive(Debug, Clone)]
pub enum AdapterFrame {
    Log {
        level: String,
        message: String,
    },
    Event {
        event_type: String,
        message: Option<String>,
    },
    Approval {
        kind: ApprovalKind,
        action: String,
        details: Option<String>,
    },
    Task {
        description: String,
    },
}

#[derive(Debug, Deserialize)]
struct StructuredLine {
    #[serde(rename = "type")]
    kind: String,
    level: Option<String>,
    event_type: Option<String>,
    message: Option<String>,
    action: Option<String>,
    details: Option<String>,
    approval_kind: Option<String>,
    description: Option<String>,
}

struct GenericAdapter {
    kind: AgentAdapterKind,
    display_name: &'static str,
    default_command: &'static str,
    local_intercept: bool,
}

impl AgentAdapter for GenericAdapter {
    fn kind(&self) -> AgentAdapterKind {
        self.kind
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn default_command(&self) -> &'static str {
        self.default_command
    }

    fn supports_local_intercept(&self) -> bool {
        self.local_intercept
    }

    fn parse_line(&self, line: &str) -> Vec<AdapterFrame> {
        parse_structured_line(line)
    }
}

pub fn adapter_for(kind: AgentAdapterKind) -> Box<dyn AgentAdapter> {
    match kind {
        AgentAdapterKind::Codex => Box::new(GenericAdapter {
            kind,
            display_name: "Codex CLI",
            default_command: "codex",
            local_intercept: true,
        }),
        AgentAdapterKind::ClaudeCode => Box::new(GenericAdapter {
            kind,
            display_name: "Claude Code",
            default_command: "claude",
            local_intercept: true,
        }),
        AgentAdapterKind::Gemini => Box::new(GenericAdapter {
            kind,
            display_name: "Gemini CLI",
            default_command: "gemini",
            local_intercept: true,
        }),
        AgentAdapterKind::Generic => Box::new(GenericAdapter {
            kind,
            display_name: "Generic Agent CLI",
            default_command: "",
            local_intercept: false,
        }),
    }
}

pub fn parse_structured_line(line: &str) -> Vec<AdapterFrame> {
    let payload = line
        .strip_prefix("TINFO:")
        .or_else(|| line.strip_prefix("[tinfo]"))
        .unwrap_or(line)
        .trim();

    let Ok(parsed) = serde_json::from_str::<StructuredLine>(payload) else {
        return Vec::new();
    };

    match parsed.kind.as_str() {
        "log" => vec![AdapterFrame::Log {
            level: parsed.level.unwrap_or_else(|| "info".to_string()),
            message: parsed.message.unwrap_or_default(),
        }],
        "event" => vec![AdapterFrame::Event {
            event_type: parsed.event_type.unwrap_or_else(|| "output_stream".to_string()),
            message: parsed.message,
        }],
        "approval" => vec![AdapterFrame::Approval {
            kind: parse_approval_kind(parsed.approval_kind.as_deref()),
            action: parsed.action.unwrap_or_else(|| "approval requested".to_string()),
            details: parsed.details,
        }],
        "task" => vec![AdapterFrame::Task {
            description: parsed.description.unwrap_or_else(|| "task".to_string()),
        }],
        _ => Vec::new(),
    }
}

fn parse_approval_kind(value: Option<&str>) -> ApprovalKind {
    match value.unwrap_or("other") {
        "shell_command" => ApprovalKind::ShellCommand,
        "file_write" => ApprovalKind::FileWrite,
        "network_call" => ApprovalKind::NetworkCall,
        "package_install" => ApprovalKind::PackageInstall,
        _ => ApprovalKind::Other,
    }
}
