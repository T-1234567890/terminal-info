use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    Running,
    Waiting,
    Idle,
    Error,
    Paused,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentTaskState {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentTask {
    pub id: String,
    pub description: String,
    pub source: String,
    pub state: AgentTaskState,
    pub created_at: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentSession {
    pub id: String,
    pub adapter_type: String,
    pub display_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub auto_start: bool,
    pub state: AgentState,
    pub pid: Option<u32>,
    pub current_session_id: Option<String>,
    pub current_task: Option<AgentTask>,
    pub pending_approval_action: Option<String>,
    pub last_output: Option<String>,
    pub last_error: Option<String>,
    pub last_event_at: u64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    ShellCommand,
    FileWrite,
    NetworkCall,
    PackageInstall,
    Other,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub agent_id: String,
    pub kind: ApprovalKind,
    pub action: String,
    pub details: Option<String>,
    pub state: ApprovalState,
    pub created_at: u64,
    pub resolved_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentLogEntry {
    pub timestamp: u64,
    pub agent_id: String,
    pub level: String,
    pub message: String,
}
