use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::ai::agent::{AgentSession, ApprovalRequest};
use crate::ai::chat::ChatSession;

#[derive(Debug, Clone)]
pub struct Storage {
    root: PathBuf,
    db_path: PathBuf,
    logs_path: PathBuf,
    events_path: PathBuf,
    audit_path: PathBuf,
    persist_chat_transcripts: bool,
}

#[derive(Debug, Default)]
pub struct StoredState {
    pub agents: Vec<AgentSession>,
    pub approvals: Vec<ApprovalRequest>,
    pub chat_sessions: Vec<ChatSession>,
}

impl Storage {
    pub fn new(root: PathBuf, persist_chat_transcripts: bool) -> Result<Self, String> {
        fs::create_dir_all(&root)
            .map_err(|err| format!("Failed to create AI data directory {}: {err}", root.display()))?;
        let storage = Self {
            db_path: root.join("runtime.sqlite3"),
            logs_path: root.join("logs.jsonl"),
            events_path: root.join("events.jsonl"),
            audit_path: root.join("audit.jsonl"),
            root,
            persist_chat_transcripts,
        };
        storage.ensure_schema()?;
        Ok(storage)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load_state(&self) -> Result<StoredState, String> {
        let conn = self.open()?;
        let mut state = StoredState::default();

        {
            let mut stmt = conn
                .prepare(
                    "SELECT payload FROM agents ORDER BY id",
                )
                .map_err(|err| format!("Failed to read agents: {err}"))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|err| format!("Failed to query agents: {err}"))?;
            for row in rows {
                let payload = row.map_err(|err| format!("Failed to read agent row: {err}"))?;
                if let Ok(agent) = serde_json::from_str::<AgentSession>(&payload) {
                    state.agents.push(agent);
                }
            }
        }

        {
            let mut stmt = conn
                .prepare(
                    "SELECT payload FROM approvals WHERE state = 'pending' ORDER BY created_at",
                )
                .map_err(|err| format!("Failed to read approvals: {err}"))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|err| format!("Failed to query approvals: {err}"))?;
            for row in rows {
                let payload =
                    row.map_err(|err| format!("Failed to read approval row: {err}"))?;
                if let Ok(approval) = serde_json::from_str::<ApprovalRequest>(&payload) {
                    state.approvals.push(approval);
                }
            }
        }

        {
            let mut stmt = conn
                .prepare("SELECT payload FROM chat_sessions ORDER BY updated_at DESC")
                .map_err(|err| format!("Failed to read chat sessions: {err}"))?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|err| format!("Failed to query chat sessions: {err}"))?;
            for row in rows {
                let payload =
                    row.map_err(|err| format!("Failed to read chat session row: {err}"))?;
                if let Ok(chat) = serde_json::from_str::<ChatSession>(&payload) {
                    state.chat_sessions.push(chat);
                }
            }
        }

        Ok(state)
    }

    pub fn upsert_agent(&self, agent: &AgentSession) -> Result<(), String> {
        self.upsert_json("agents", &agent.id, agent.last_event_at, agent)
    }

    pub fn upsert_approval(&self, approval: &ApprovalRequest) -> Result<(), String> {
        self.upsert_json("approvals", &approval.id, approval.created_at, approval)
    }

    pub fn upsert_chat_session(&self, session: &ChatSession) -> Result<(), String> {
        self.upsert_json("chat_sessions", session.id(), session.updated_at(), session)
    }

    pub fn delete_agent(&self, agent_id: &str) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("DELETE FROM agents WHERE id = ?1", params![agent_id])
            .map_err(|err| format!("Failed to delete agent state: {err}"))?;
        Ok(())
    }

    pub fn delete_approvals_for_agent(&self, agent_id: &str) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("DELETE FROM approvals WHERE json_extract(payload, '$.agent_id') = ?1", params![agent_id])
            .map_err(|err| format!("Failed to delete approvals for agent: {err}"))?;
        Ok(())
    }

    pub fn append_log<T: Serialize>(&self, value: &T) -> Result<(), String> {
        append_jsonl(&self.logs_path, value)
    }

    pub fn append_event<T: Serialize>(&self, value: &T) -> Result<(), String> {
        append_jsonl(&self.events_path, value)
    }

    pub fn append_audit<T: Serialize>(&self, value: &T) -> Result<(), String> {
        append_jsonl(&self.audit_path, value)
    }

    pub fn persist_chat_transcripts(&self) -> bool {
        self.persist_chat_transcripts
    }

    fn ensure_schema(&self) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                updated_at INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                state TEXT NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chat_sessions (
                id TEXT PRIMARY KEY,
                updated_at INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            ",
        )
        .map_err(|err| format!("Failed to initialize AI runtime storage: {err}"))
    }

    fn open(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path)
            .map_err(|err| format!("Failed to open AI runtime database {}: {err}", self.db_path.display()))
    }

    fn upsert_json<T: Serialize>(
        &self,
        table: &str,
        id: &str,
        timestamp: u64,
        value: &T,
    ) -> Result<(), String> {
        let payload = serde_json::to_string(value)
            .map_err(|err| format!("Failed to serialize runtime state: {err}"))?;
        let conn = self.open()?;
        match table {
            "agents" => conn
                .execute(
                    "INSERT INTO agents (id, updated_at, payload) VALUES (?1, ?2, ?3)
                     ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at, payload = excluded.payload",
                    params![id, timestamp as i64, payload],
                )
                .map_err(|err| format!("Failed to persist agent state: {err}"))?,
            "approvals" => {
                let state = serde_json::to_value(value)
                    .ok()
                    .and_then(|json| json.get("state").and_then(|v| v.as_str()).map(str::to_string))
                    .unwrap_or_else(|| "pending".to_string());
                conn.execute(
                    "INSERT INTO approvals (id, created_at, state, payload) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(id) DO UPDATE SET created_at = excluded.created_at, state = excluded.state, payload = excluded.payload",
                    params![id, timestamp as i64, state, payload],
                )
                .map_err(|err| format!("Failed to persist approval state: {err}"))?
            }
            "chat_sessions" => conn
                .execute(
                    "INSERT INTO chat_sessions (id, updated_at, payload) VALUES (?1, ?2, ?3)
                     ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at, payload = excluded.payload",
                    params![id, timestamp as i64, payload],
                )
                .map_err(|err| format!("Failed to persist chat session: {err}"))?,
            _ => return Err(format!("Unknown storage table '{table}'.")),
        };
        Ok(())
    }
}

fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let line = serde_json::to_string(value)
        .map_err(|err| format!("Failed to serialize JSONL value: {err}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("Failed to open {}: {err}", path.display()))?;
    writeln!(file, "{line}")
        .map_err(|err| format!("Failed to write {}: {err}", path.display()))
}
