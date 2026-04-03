use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::ai::adapters::{AgentAdapterKind, ConfiguredAgent, adapter_for};
use crate::ai::agent::{
    AgentLogEntry, AgentSession, AgentState, AgentTask, AgentTaskState, ApprovalKind,
    ApprovalRequest, ApprovalState,
};
use crate::ai::chat::{self, ChatRole, ChatSession, HistoryMode, ProviderKind};
use crate::ai::config::AiConfig;
use crate::ai::hook::HookEventPayload;
use crate::ai::ipc::{AgentDecision, write_agent_decision};
use crate::ai::storage::Storage;

#[derive(Debug, Clone)]
struct ApprovalCandidate {
    action: String,
    details: Option<String>,
    kind: ApprovalKind,
}

#[derive(Clone)]
pub struct Runtime {
    config: AiConfig,
    storage: Storage,
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    state: Mutex<RuntimeState>,
    next_id: AtomicU64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeState {
    pub agents: BTreeMap<String, AgentSession>,
    pub approvals: VecDeque<ApprovalRequest>,
    pub logs: VecDeque<AgentLogEntry>,
    pub events: VecDeque<RuntimeEvent>,
    pub hook_event_count: u64,
    pub chat_sessions: BTreeMap<String, ChatSession>,
    pub active_chat_session_id: Option<String>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            agents: BTreeMap::new(),
            approvals: VecDeque::new(),
            logs: VecDeque::new(),
            events: VecDeque::new(),
            hook_event_count: 0,
            chat_sessions: BTreeMap::new(),
            active_chat_session_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    AgentStarted { agent_id: String },
    AgentPaused { agent_id: String },
    AgentResumed { agent_id: String },
    StepStarted { agent_id: String, step: String },
    ToolCalled { agent_id: String, tool: String },
    WaitingApproval { agent_id: String, request_id: String },
    ApprovalResolved { agent_id: String, request_id: String, state: ApprovalState },
    OutputStream { agent_id: String, chunk: String },
    ChatChunk { session_id: String, chunk: String },
    ChatFinished { session_id: String },
    Finished { agent_id: String },
    Error { agent_id: String, message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    pub agents: Vec<AgentSession>,
    pub approvals: Vec<ApprovalRequest>,
    pub logs: Vec<AgentLogEntry>,
    pub events: Vec<RuntimeEvent>,
    pub hook_event_count: u64,
    pub sessions: Vec<ChatSession>,
    pub active_session: Option<ChatSession>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSessionsPayload {
    pub sessions: Vec<ChatSession>,
    pub active_session: Option<ChatSession>,
}

impl Runtime {
    pub fn new(config: AiConfig) -> Self {
        let storage = Storage::new(config.data_dir(), config.persist_chat_transcripts())
            .or_else(|_| {
                Storage::new(
                    std::env::temp_dir().join("ai"),
                    config.persist_chat_transcripts(),
                )
            })
            .unwrap_or_else(|err| panic!("Failed to initialize ai storage: {err}"));
        let runtime = Self {
            config,
            storage,
            inner: Arc::new(RuntimeInner {
                state: Mutex::new(RuntimeState::default()),
                next_id: AtomicU64::new(1),
            }),
        };
        runtime.bootstrap();
        runtime
    }

    pub fn config(&self) -> &AiConfig {
        &self.config
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        let state = self.inner.state.lock().expect("runtime state poisoned");
        let sessions = state.chat_sessions.values().cloned().collect::<Vec<_>>();
        let active_session = state
            .active_chat_session_id
            .as_ref()
            .and_then(|id| state.chat_sessions.get(id))
            .cloned();
        RuntimeSnapshot {
            agents: state.agents.values().cloned().collect(),
            approvals: state.approvals.iter().cloned().collect(),
            logs: state.logs.iter().cloned().collect(),
            events: state.events.iter().cloned().collect(),
            hook_event_count: state.hook_event_count,
            sessions,
            active_session,
        }
    }

    pub fn chat_sessions_payload(&self) -> ChatSessionsPayload {
        let snapshot = self.snapshot();
        ChatSessionsPayload {
            sessions: snapshot.sessions,
            active_session: snapshot.active_session,
        }
    }

    pub fn start_agent(&self, _agent_id: &str) -> Result<(), String> {
        Err("Agent control is hook-driven. Run Codex with hooks enabled instead.".to_string())
    }

    pub fn stop_agent(&self, _agent_id: &str) -> Result<(), String> {
        Err("Stopping a hooked agent is not supported yet.".to_string())
    }

    pub fn pause_agent(&self, _agent_id: &str) -> Result<(), String> {
        Err("Pausing a hooked agent is not supported yet.".to_string())
    }

    pub fn resume_agent(&self, _agent_id: &str) -> Result<(), String> {
        Err("Resuming a hooked agent is not supported yet.".to_string())
    }

    pub fn restart_agent(&self, _agent_id: &str) -> Result<(), String> {
        Err("Restarting a hooked agent is not supported yet.".to_string())
    }

    pub fn agent(&self, agent_id: &str) -> Option<AgentSession> {
        let state = self.inner.state.lock().expect("runtime state poisoned");
        state.agents.get(agent_id).cloned()
    }

    pub fn create_chat_session(
        &self,
        provider: Option<ProviderKind>,
        model: Option<String>,
        system_prompt: Option<String>,
    ) -> Result<ChatSession, String> {
        let provider = provider.unwrap_or(self.config.default_chat_provider());
        let model = model.unwrap_or_else(|| self.config.default_model(provider).to_string());
        let now = now_unix();
        let session = ChatSession::new(
            self.next_identifier("chat"),
            provider,
            model,
            if self.config.persist_chat_transcripts() {
                HistoryMode::Persisted
            } else {
                HistoryMode::InMemory
            },
            system_prompt,
            now,
        );
        self.storage.upsert_chat_session(&session)?;
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        state.active_chat_session_id = Some(session.id().to_string());
        state
            .chat_sessions
            .insert(session.id().to_string(), session.clone());
        Ok(session)
    }

    pub fn ensure_chat_session(
        &self,
        session_id: Option<String>,
        provider: Option<ProviderKind>,
        model: Option<String>,
        system_prompt: Option<String>,
    ) -> Result<String, String> {
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        if let Some(session_id) = session_id {
            if let Some(session) = state.chat_sessions.get_mut(&session_id) {
                if let Some(provider) = provider {
                    session.set_provider(provider);
                }
                if let Some(model) = model {
                    session.set_model(model);
                }
                if system_prompt.is_some() {
                    session.set_system_prompt(system_prompt);
                }
                self.storage.upsert_chat_session(session)?;
                state.active_chat_session_id = Some(session_id.clone());
                return Ok(session_id);
            }
        }
        drop(state);
        self.create_chat_session(provider, model, system_prompt)
            .map(|session| session.id().to_string())
    }

    pub fn send_chat_message(&self, session_id: &str, content: String) -> Result<(), String> {
        {
            let mut state = self.inner.state.lock().expect("runtime state poisoned");
            let session = state
                .chat_sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Chat session '{}' not found.", session_id))?;
            let now = now_unix();
            session.push_message(ChatRole::User, content.clone(), now);
            session.start_stream(now);
            self.storage.upsert_chat_session(session)?;
        }

        let runtime = self.clone();
        let session_id = session_id.to_string();
        thread::spawn(move || {
            let session = {
                let state = runtime.inner.state.lock().expect("runtime state poisoned");
                state.chat_sessions.get(&session_id).cloned()
            };

            let Some(session) = session else {
                return;
            };

            match chat::stream_message(&runtime.config, &session, |chunk| {
                runtime.append_chat_chunk(&session_id, chunk);
            }) {
                Ok(()) => {
                    let _ = runtime.finish_chat_stream(&session_id);
                }
                Err(err) => {
                    let _ = runtime.fail_chat_stream(&session_id, err);
                }
            }
        });

        Ok(())
    }

    pub fn send_chat_to_agent(
        &self,
        session_id: &str,
        agent_id: Option<String>,
    ) -> Result<AgentTask, String> {
        let content = {
            let state = self.inner.state.lock().expect("runtime state poisoned");
            let session = state
                .chat_sessions
                .get(session_id)
                .ok_or_else(|| format!("Chat session '{}' not found.", session_id))?;
            session
                .latest_assistant_message()
                .filter(|content| !content.trim().is_empty())
                .ok_or_else(|| "No assistant message is available to hand off.".to_string())?
                .to_string()
        };

        let agent_id = agent_id.unwrap_or_else(|| "codex".to_string());
        let now = now_unix();
        let task = AgentTask {
            id: self.next_identifier("task"),
            description: content.clone(),
            source: "chat".to_string(),
            state: AgentTaskState::Pending,
            created_at: now,
        };

        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let adapter = AgentAdapterKind::from_config(Some(&agent_id));
        let ensured_id = self.ensure_hook_agent_locked(&mut state, adapter, &agent_id);
        let agent = state
            .agents
            .get_mut(&ensured_id)
            .ok_or_else(|| format!("Agent '{}' is not configured.", ensured_id))?;
        agent.current_session_id = Some(session_id.to_string());
        agent.current_task = Some(task.clone());
        agent.last_event_at = now;
        self.persist_agent(agent);
        self.push_event_locked(
            &mut state,
            RuntimeEvent::StepStarted {
                agent_id: ensured_id.clone(),
                step: task.description.clone(),
            },
        );
        self.push_log_locked(
            &mut state,
            AgentLogEntry {
                timestamp: now,
                agent_id: ensured_id,
                level: "task".to_string(),
                message: format!("Chat handoff queued: {}", task.description),
            },
        );
        Ok(task)
    }

    pub fn add_approval_request(
        &self,
        agent_id: impl Into<String>,
        kind: ApprovalKind,
        action: impl Into<String>,
        details: Option<String>,
    ) -> ApprovalRequest {
        let requested_agent_id = agent_id.into();
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let agent_id = self.resolve_agent_id_locked(&state, &requested_agent_id);
        let request = ApprovalRequest {
            id: self.next_identifier("approval"),
            agent_id: agent_id.clone(),
            kind,
            action: action.into(),
            details,
            state: ApprovalState::Pending,
            created_at: now_unix(),
            resolved_at: None,
        };

        self.push_event_locked(
            &mut state,
            RuntimeEvent::WaitingApproval {
                agent_id: request.agent_id.clone(),
                request_id: request.id.clone(),
            },
        );
        self.push_log_locked(
            &mut state,
            AgentLogEntry {
                timestamp: now_unix(),
                agent_id: request.agent_id.clone(),
                level: "approval".to_string(),
                message: format!("Approval requested: {}", request.action),
            },
        );
        state.approvals.push_back(request.clone());
        if let Some(agent) = state.agents.get_mut(&request.agent_id) {
            agent.state = AgentState::Waiting;
            agent.last_event_at = now_unix();
            self.persist_agent(agent);
        }
        let _ = self.storage.upsert_approval(&request);
        let _ = self.storage.append_audit(&request);
        request
    }

    pub fn approve_request(&self, request_id: &str) -> Result<(), String> {
        self.resolve_request(request_id, ApprovalState::Approved)
    }

    pub fn deny_request(&self, request_id: &str) -> Result<(), String> {
        self.resolve_request(request_id, ApprovalState::Denied)
    }

    pub fn append_external_log(
        &self,
        agent_id: impl Into<String>,
        level: impl Into<String>,
        message: impl Into<String>,
    ) {
        let requested_agent_id = agent_id.into();
        let message = message.into();
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let agent_id = self.resolve_agent_id_locked(&state, &requested_agent_id);
        let entry = AgentLogEntry {
            timestamp: now_unix(),
            agent_id: agent_id.clone(),
            level: level.into(),
            message: message.clone(),
        };

        let pending_approval = maybe_extract_approval_from_output(
            state.agents.get(&agent_id),
            &state.approvals,
            &agent_id,
            &message,
        );

        if let Some(agent) = state.agents.get_mut(&agent_id) {
            if !message.trim().is_empty() {
                agent.last_output = Some(message.clone());
                if let Some(next_capture) = next_approval_capture_state(&message) {
                    agent.pending_approval_action = next_capture;
                }
                if agent.state != AgentState::Waiting {
                    agent.state = AgentState::Running;
                }
                agent.last_event_at = now_unix();
                self.persist_agent(agent);
            }
        }
        self.push_event_locked(
            &mut state,
            RuntimeEvent::OutputStream {
                agent_id: agent_id.clone(),
                chunk: message,
            },
        );
        self.push_log_locked(&mut state, entry);

        if let Some(candidate) = pending_approval {
            let request = ApprovalRequest {
                id: self.next_identifier("approval"),
                agent_id: agent_id.clone(),
                kind: candidate.kind,
                action: candidate.action.clone(),
                details: candidate.details,
                state: ApprovalState::Pending,
                created_at: now_unix(),
                resolved_at: None,
            };
            state.approvals.push_back(request.clone());
            self.push_event_locked(
                &mut state,
                RuntimeEvent::WaitingApproval {
                    agent_id: request.agent_id.clone(),
                    request_id: request.id.clone(),
                },
            );
            self.push_log_locked(
                &mut state,
                AgentLogEntry {
                    timestamp: now_unix(),
                    agent_id: request.agent_id.clone(),
                    level: "approval".to_string(),
                    message: format!("Approval requested: {}", request.action),
                },
            );
            if let Some(agent) = state.agents.get_mut(&agent_id) {
                agent.state = AgentState::Waiting;
                agent.pending_approval_action = None;
                agent.last_event_at = now_unix();
                self.persist_agent(agent);
            }
            let _ = self.storage.upsert_approval(&request);
            let _ = self.storage.append_audit(&request);
        }
    }

    pub fn append_external_event(&self, event: RuntimeEvent) {
        let event = {
            let state = self.inner.state.lock().expect("runtime state poisoned");
            match event {
                RuntimeEvent::AgentStarted { agent_id } => RuntimeEvent::AgentStarted {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                },
                RuntimeEvent::AgentPaused { agent_id } => RuntimeEvent::AgentPaused {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                },
                RuntimeEvent::AgentResumed { agent_id } => RuntimeEvent::AgentResumed {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                },
                RuntimeEvent::StepStarted { agent_id, step } => RuntimeEvent::StepStarted {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                    step,
                },
                RuntimeEvent::ToolCalled { agent_id, tool } => RuntimeEvent::ToolCalled {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                    tool,
                },
                RuntimeEvent::WaitingApproval {
                    agent_id,
                    request_id,
                } => RuntimeEvent::WaitingApproval {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                    request_id,
                },
                RuntimeEvent::ApprovalResolved {
                    agent_id,
                    request_id,
                    state: approval_state,
                } => RuntimeEvent::ApprovalResolved {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                    request_id,
                    state: approval_state,
                },
                RuntimeEvent::OutputStream { agent_id, chunk } => RuntimeEvent::OutputStream {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                    chunk,
                },
                RuntimeEvent::Finished { agent_id } => RuntimeEvent::Finished {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                },
                RuntimeEvent::Error { agent_id, message } => RuntimeEvent::Error {
                    agent_id: self.resolve_agent_id_locked(&state, &agent_id),
                    message,
                },
                RuntimeEvent::ChatChunk { .. } | RuntimeEvent::ChatFinished { .. } => event,
            }
        };
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        self.push_event_locked(&mut state, event);
    }

    pub fn ingest_hook_event(&self, payload: HookEventPayload) -> Result<(), String> {
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        state.hook_event_count = state.hook_event_count.saturating_add(1);
        let adapter_kind = AgentAdapterKind::from_config(Some(&payload.adapter));
        let agent_id = self.ensure_hook_agent_locked(&mut state, adapter_kind, &payload.agent_id);
        let now = now_unix();

        match payload.event_type.as_str() {
            "session_start" => {
                if let Some(agent) = state.agents.get_mut(&agent_id) {
                    agent.state = AgentState::Running;
                    agent.last_event_at = now;
                    self.persist_agent(agent);
                }
                self.push_event_locked(
                    &mut state,
                    RuntimeEvent::AgentStarted {
                        agent_id: agent_id.clone(),
                    },
                );
                Ok(())
            }
            "command_request" => {
                drop(state);
                let request = self.add_approval_request(
                    agent_id.clone(),
                    classify_guardrail(payload.command.as_deref().unwrap_or_default())
                        .unwrap_or(ApprovalKind::ShellCommand),
                    payload.command.unwrap_or_else(|| "command".to_string()),
                    payload.details,
                );
                let mut state = self.inner.state.lock().expect("runtime state poisoned");
                if let Some(agent) = state.agents.get_mut(&agent_id) {
                    agent.state = AgentState::Waiting;
                    agent.current_task = Some(AgentTask {
                        id: self.next_identifier("task"),
                        description: request.action.clone(),
                        source: "hook".to_string(),
                        state: AgentTaskState::Pending,
                        created_at: now,
                    });
                    agent.last_event_at = now;
                    self.persist_agent(agent);
                }
                Ok(())
            }
            "command_start" => {
                if let Some(agent) = state.agents.get_mut(&agent_id) {
                    let description = payload
                        .command
                        .clone()
                        .or(payload.details.clone())
                        .unwrap_or_else(|| "command".to_string());
                    agent.state = AgentState::Running;
                    agent.current_task = Some(AgentTask {
                        id: self.next_identifier("task"),
                        description: description.clone(),
                        source: "hook".to_string(),
                        state: AgentTaskState::Running,
                        created_at: now,
                    });
                    agent.last_event_at = now;
                    self.persist_agent(agent);
                    self.push_event_locked(
                        &mut state,
                        RuntimeEvent::StepStarted {
                            agent_id: agent_id.clone(),
                            step: description,
                        },
                    );
                }
                Ok(())
            }
            "command_finish" => {
                if let Some(agent) = state.agents.get_mut(&agent_id) {
                    agent.state = AgentState::Idle;
                    agent.current_task = None;
                    agent.last_event_at = now;
                    self.persist_agent(agent);
                }
                self.push_event_locked(
                    &mut state,
                    RuntimeEvent::Finished {
                        agent_id: agent_id.clone(),
                    },
                );
                Ok(())
            }
            "stop" => {
                self.cleanup_stopped_agent_locked(&mut state, &agent_id);
                self.push_event_locked(
                    &mut state,
                    RuntimeEvent::Finished {
                        agent_id: agent_id.clone(),
                    },
                );
                Ok(())
            }
            "output" => {
                drop(state);
                if let Some(output) = payload.output.or(payload.command).or(payload.details) {
                    self.append_external_log(agent_id, "info".to_string(), output);
                }
                Ok(())
            }
            "user_prompt_submit" => {
                drop(state);
                if let Some(output) = payload.output.or(payload.command).or(payload.details) {
                    self.append_external_log(agent_id, "info".to_string(), output);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn apply_timeouts(&self) -> Result<(), String> {
        let Some(timeout_secs) = self.config.auto_reject_timeout_secs() else {
            return Ok(());
        };

        let now = now_unix();
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let mut changed = Vec::new();
        for request in state.approvals.iter_mut() {
            if request.state == ApprovalState::Pending
                && now.saturating_sub(request.created_at) >= timeout_secs
            {
                request.state = ApprovalState::Denied;
                request.resolved_at = Some(now);
                changed.push(request.clone());
            }
        }
        for request in changed {
            if let Some(agent) = state.agents.get_mut(&request.agent_id) {
                agent.state = AgentState::Idle;
                agent.current_task = None;
                agent.last_event_at = now;
                self.persist_agent(agent);
            }
            self.push_event_locked(
                &mut state,
                RuntimeEvent::ApprovalResolved {
                    agent_id: request.agent_id.clone(),
                    request_id: request.id.clone(),
                    state: ApprovalState::Denied,
                },
            );
            let _ = self.storage.upsert_approval(&request);
            let _ = self.storage.append_audit(&request);
        }
        Ok(())
    }

    fn bootstrap(&self) {
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let stored = self.storage.load_state().unwrap_or_default();
        // External agent sessions are live hook-driven state, not durable boot state.
        // Starting from a clean in-memory agent/approval view avoids replaying stale CLIs.
        for configured in self.config.agents() {
            state
                .agents
                .insert(configured.id.clone(), bootstrap_agent_session(&configured));
        }
        for session in stored.chat_sessions {
            state.chat_sessions.insert(session.id().to_string(), session);
        }
        if state.chat_sessions.is_empty() {
            let session = ChatSession::new(
                self.next_identifier("chat"),
                self.config.default_chat_provider(),
                self.config
                    .default_model(self.config.default_chat_provider())
                    .to_string(),
                if self.config.persist_chat_transcripts() {
                    HistoryMode::Persisted
                } else {
                    HistoryMode::InMemory
                },
                Some(
                    "You are the ai planning surface. Suggest tools and actions, but do not claim to execute them."
                        .to_string(),
                ),
                now_unix(),
            );
            self.storage.upsert_chat_session(&session).ok();
            state.active_chat_session_id = Some(session.id().to_string());
            state.chat_sessions.insert(session.id().to_string(), session);
        } else {
            state.active_chat_session_id = state.chat_sessions.keys().next().cloned();
        }
    }

    fn resolve_request(&self, request_id: &str, next: ApprovalState) -> Result<(), String> {
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let position = state
            .approvals
            .iter()
            .position(|item| item.id == request_id)
            .ok_or_else(|| format!("Approval request '{}' not found.", request_id))?;
        let request = state
            .approvals
            .get_mut(position)
            .expect("approval request must exist");
        request.state = next;
        request.resolved_at = Some(now_unix());
        let updated = request.clone();
        let agent_id_for_decision = updated.agent_id.clone();
        if let Some(agent) = state.agents.get_mut(&updated.agent_id) {
            agent.state = if next == ApprovalState::Approved {
                AgentState::Running
            } else {
                AgentState::Idle
            };
            if next == ApprovalState::Denied {
                agent.current_task = None;
            }
            agent.last_event_at = now_unix();
            self.persist_agent(agent);
        }
        self.push_event_locked(
            &mut state,
            RuntimeEvent::ApprovalResolved {
                agent_id: updated.agent_id.clone(),
                request_id: updated.id.clone(),
                state: next,
            },
        );
        self.push_log_locked(
            &mut state,
            AgentLogEntry {
                timestamp: now_unix(),
                agent_id: updated.agent_id.clone(),
                level: "approval".to_string(),
                message: format!("Approval {} {}", updated.id, approval_label(next)),
            },
        );
        self.storage.upsert_approval(&updated)?;
        self.storage.append_audit(&updated)?;
        if matches!(next, ApprovalState::Approved | ApprovalState::Denied) {
            let decision_value = match next {
                ApprovalState::Approved => "approve".to_string(),
                ApprovalState::Denied => "deny".to_string(),
                ApprovalState::Pending => "pending".to_string(),
            };
            let decision = AgentDecision {
                agent_id: agent_id_for_decision.clone(),
                request_id: updated.id.clone(),
                decision: decision_value.clone(),
            };
            let _ = write_agent_decision(&decision);
            if let Some((_, raw_id)) = agent_id_for_decision.split_once(':') {
                let raw_decision = AgentDecision {
                    agent_id: raw_id.to_string(),
                    request_id: updated.id.clone(),
                    decision: decision_value,
                };
                let _ = write_agent_decision(&raw_decision);
            }
        }
        Ok(())
    }

    fn append_chat_chunk(&self, session_id: &str, chunk: &str) {
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        if let Some(session) = state.chat_sessions.get_mut(session_id) {
            session.append_stream_chunk(chunk, now_unix());
            let _ = self.storage.upsert_chat_session(session);
        }
        self.push_event_locked(
            &mut state,
            RuntimeEvent::ChatChunk {
                session_id: session_id.to_string(),
                chunk: chunk.to_string(),
            },
        );
    }

    fn finish_chat_stream(&self, session_id: &str) -> Result<(), String> {
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let session = state
            .chat_sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Chat session '{}' not found.", session_id))?;
        session.finish_stream(now_unix());
        self.storage.upsert_chat_session(session)?;
        self.push_event_locked(
            &mut state,
            RuntimeEvent::ChatFinished {
                session_id: session_id.to_string(),
            },
        );
        Ok(())
    }

    fn fail_chat_stream(&self, session_id: &str, err: String) -> Result<(), String> {
        let mut state = self.inner.state.lock().expect("runtime state poisoned");
        let session = state
            .chat_sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Chat session '{}' not found.", session_id))?;
        session.fail_stream(err.clone(), now_unix());
        self.storage.upsert_chat_session(session)?;
        self.push_log_locked(
            &mut state,
            AgentLogEntry {
                timestamp: now_unix(),
                agent_id: "chat".to_string(),
                level: "error".to_string(),
                message: err.clone(),
            },
        );
        self.push_event_locked(
            &mut state,
            RuntimeEvent::Error {
                agent_id: "chat".to_string(),
                message: err,
            },
        );
        Ok(())
    }

    fn next_identifier(&self, prefix: &str) -> String {
        let value = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        format!("{prefix}-{value}")
    }

    fn push_event_locked(&self, state: &mut RuntimeState, event: RuntimeEvent) {
        state.events.push_back(event.clone());
        while state.events.len() > self.config.event_buffer_size() {
            state.events.pop_front();
        }
        let _ = self.storage.append_event(&event);
    }

    fn push_log_locked(&self, state: &mut RuntimeState, entry: AgentLogEntry) {
        state.logs.push_back(entry.clone());
        while state.logs.len() > self.config.log_buffer_size() {
            state.logs.pop_front();
        }
        let _ = self.storage.append_log(&entry);
    }

    fn persist_agent(&self, agent: &AgentSession) {
        let _ = self.storage.upsert_agent(agent);
    }

    fn cleanup_stopped_agent_locked(&self, state: &mut RuntimeState, agent_id: &str) {
        state.approvals.retain(|request| request.agent_id != agent_id);
        if self.is_transient_hook_agent(agent_id) {
            state.agents.remove(agent_id);
            let _ = self.storage.delete_agent(agent_id);
            let _ = self.storage.delete_approvals_for_agent(agent_id);
            return;
        }

        if let Some(agent) = state.agents.get_mut(agent_id) {
            agent.state = AgentState::Idle;
            agent.current_task = None;
            agent.pending_approval_action = None;
            agent.last_output = None;
            agent.last_event_at = now_unix();
            self.persist_agent(agent);
        }
        let _ = self.storage.delete_approvals_for_agent(agent_id);
    }

    fn is_transient_hook_agent(&self, agent_id: &str) -> bool {
        agent_id.contains(':') && !self.config.agents().iter().any(|agent| agent.id == agent_id)
    }

    fn resolve_agent_id_locked(&self, state: &RuntimeState, requested: &str) -> String {
        if state.agents.contains_key(requested) {
            return requested.to_string();
        }

        let normalized = requested.trim().to_ascii_lowercase();
        let adapter = match normalized.as_str() {
            "codex" | "codex cli" => Some("codex"),
            "claude" | "claude code" | "cloud code" | "claude_code" => Some("claude_code"),
            "gemini" | "gemini cli" | "gemini-cli" => Some("gemini"),
            _ => None,
        };

        let Some(adapter) = adapter else {
            return requested.to_string();
        };

        state
            .agents
            .values()
            .filter(|agent| agent.adapter_type == adapter)
            .max_by_key(|agent| agent.last_event_at)
            .map(|agent| agent.id.clone())
            .unwrap_or_else(|| requested.to_string())
    }

    fn ensure_hook_agent_locked(
        &self,
        state: &mut RuntimeState,
        adapter_kind: AgentAdapterKind,
        requested_id: &str,
    ) -> String {
        let adapter_label = adapter_kind.label().to_string();
        let composite_id = format!("{adapter_label}:{requested_id}");
        let resolved = if state.agents.contains_key(&composite_id) {
            composite_id.clone()
        } else {
            self.resolve_agent_id_locked(state, requested_id)
        };
        if state.agents.contains_key(&resolved) {
            let existing = state.agents.get(&resolved).expect("agent must exist");
            if existing.adapter_type == adapter_label {
                return resolved;
            }
        }

        let display_name = adapter_for(adapter_kind).display_name().to_string();
        let command = match adapter_kind {
            AgentAdapterKind::Codex => "codex",
            AgentAdapterKind::ClaudeCode => "claude",
            AgentAdapterKind::Gemini => "gemini",
            AgentAdapterKind::Generic => "",
        }
        .to_string();

        state.agents.insert(
            composite_id.clone(),
            AgentSession {
                id: composite_id.clone(),
                adapter_type: adapter_label,
                display_name,
                command,
                args: Vec::new(),
                cwd: None,
                auto_start: false,
                state: AgentState::Idle,
                pid: None,
                current_session_id: None,
                current_task: None,
                pending_approval_action: None,
                last_output: None,
                last_error: None,
                last_event_at: now_unix(),
            },
        );
        composite_id
    }
}

fn bootstrap_agent_session(configured: &ConfiguredAgent) -> AgentSession {
    AgentSession {
        id: configured.id.clone(),
        adapter_type: configured.adapter.label().to_string(),
        display_name: adapter_for(configured.adapter).display_name().to_string(),
        command: configured.config.command.clone(),
        args: configured.config.args.clone(),
        cwd: configured.config.cwd.clone(),
        auto_start: configured.config.auto_start,
        state: AgentState::Idle,
        pid: None,
        current_session_id: None,
        current_task: None,
        pending_approval_action: None,
        last_output: None,
        last_error: None,
        last_event_at: now_unix(),
    }
}

fn maybe_extract_approval_from_output<'a>(
    agent: Option<&AgentSession>,
    approvals: &'a VecDeque<ApprovalRequest>,
    agent_id: &str,
    message: &str,
) -> Option<ApprovalCandidate> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }

    let has_duplicate = |action: &str| {
        approvals.iter().any(|request| {
            request.agent_id == agent_id
                && request.state == ApprovalState::Pending
                && request.action == action
        })
    };

    if let Some(candidate) = extract_approval_candidate_from_prompt(message) {
        if !has_duplicate(&candidate.action) {
            return Some(candidate);
        }
        return None;
    }

    if agent
        .and_then(|agent| agent.pending_approval_action.as_deref())
        == Some("__awaiting_command__")
    {
        let candidate = approval_candidate_from_command(message)?;
        if !has_duplicate(&candidate.action) {
            return Some(candidate);
        }
    }

    None
}

fn next_approval_capture_state(message: &str) -> Option<Option<String>> {
    if message
        .to_ascii_lowercase()
        .contains("would you like to run the following command?")
        || message
            .to_ascii_lowercase()
            .contains("would you like to run:")
    {
        Some(Some("__awaiting_command__".to_string()))
    } else {
        None
    }
}

fn extract_approval_candidate_from_prompt(message: &str) -> Option<ApprovalCandidate> {
    let lowered = message.to_ascii_lowercase();
    if !lowered.contains("would you like to run") {
        return None;
    }

    if let Some(action) = extract_command_from_compact_prompt(message) {
        let reason = extract_reason_from_compact_prompt(message);
        return Some(ApprovalCandidate {
            kind: classify_approval(&action, reason.as_deref()),
            action,
            details: reason,
        });
    }

    let mut reason = None;
    let mut command = None;
    for line in message.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let lower = line.to_ascii_lowercase();
        if lower.contains("would you like to run") || line == "[Approve] [Deny]" {
            continue;
        }
        if let Some(value) = line.strip_prefix("Reason:") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                reason = Some(trimmed.to_string());
            }
            continue;
        }
        if let Some(value) = extract_command_token(line) {
            command = Some(value);
            break;
        }
    }
    let action = command?;
    Some(ApprovalCandidate {
        kind: classify_approval(&action, reason.as_deref()),
        action,
        details: reason,
    })
}

fn extract_command_from_compact_prompt(message: &str) -> Option<String> {
    let compact = normalize_inline_whitespace(message);
    let dollar_index = compact.find('$')?;
    let after_dollar = compact[dollar_index + 1..].trim_start();
    let command = truncate_before_prompt_options(after_dollar);
    if command.is_empty() {
        None
    } else {
        Some(command.to_string())
    }
}

fn extract_reason_from_compact_prompt(message: &str) -> Option<String> {
    let compact = normalize_inline_whitespace(message);
    let reason_index = compact.find("Reason:")?;
    let after_reason = compact[reason_index + "Reason:".len()..].trim_start();
    let end = after_reason.find('$').unwrap_or(after_reason.len());
    let reason = after_reason[..end].trim();
    if reason.is_empty() {
        None
    } else {
        Some(reason.to_string())
    }
}

fn normalize_inline_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_before_prompt_options(input: &str) -> &str {
    let patterns = [
        "› 1.",
        "> 1.",
        "1. Yes, proceed",
        "2. Yes,",
        "3. No,",
        "Press enter to confirm",
    ];
    patterns
        .iter()
        .filter_map(|pattern| input.find(pattern))
        .min()
        .map(|idx| input[..idx].trim_end())
        .unwrap_or_else(|| input.trim())
}

fn approval_candidate_from_command(message: &str) -> Option<ApprovalCandidate> {
    let action = first_command_line(message)?;
    Some(ApprovalCandidate {
        kind: classify_approval(&action, None),
        action,
        details: Some("Detected from Codex output fallback".to_string()),
    })
}

fn first_command_line(message: &str) -> Option<String> {
    for line in message.lines().map(str::trim) {
        if line.is_empty()
            || line.to_ascii_lowercase().contains("would you like to run")
            || line.starts_with("Reason:")
            || line == "[Approve] [Deny]"
        {
            continue;
        }
        if let Some(value) = extract_command_token(line) {
            return Some(value);
        }
    }
    None
}

fn extract_command_token(line: &str) -> Option<String> {
    let trimmed = line.trim_start_matches('>').trim();
    if let Some(command) = trimmed.strip_prefix('$') {
        let command = command.trim();
        if !command.is_empty() {
            return Some(command.to_string());
        }
    }
    if looks_like_command(trimmed) {
        return Some(trimmed.to_string());
    }
    None
}

fn looks_like_command(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("touch ")
        || lower.starts_with("rm ")
        || lower.starts_with("mv ")
        || lower.starts_with("cp ")
        || lower.starts_with("mkdir ")
        || lower.starts_with("rmdir ")
        || lower.starts_with("chmod ")
        || lower.starts_with("chown ")
        || lower.starts_with("git ")
        || lower.starts_with("cargo ")
        || lower.starts_with("curl ")
        || lower.starts_with("wget ")
        || lower.starts_with("npm ")
        || lower.starts_with("pnpm ")
        || lower.starts_with("yarn ")
        || lower.starts_with("brew ")
        || lower.starts_with("apt ")
        || lower.starts_with("pip ")
        || lower.starts_with("python ")
}

fn classify_approval(action: &str, reason: Option<&str>) -> ApprovalKind {
    if let Some(kind) = reason.and_then(classify_guardrail) {
        return kind;
    }
    classify_guardrail(action).unwrap_or(ApprovalKind::ShellCommand)
}

fn approval_label(state: ApprovalState) -> &'static str {
    match state {
        ApprovalState::Pending => "pending",
        ApprovalState::Approved => "approved",
        ApprovalState::Denied => "denied",
    }
}

fn classify_guardrail(text: &str) -> Option<ApprovalKind> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("brew ")
        || lower.contains("apt ")
        || lower.contains("dnf ")
        || lower.contains("npm install")
        || lower.contains("pnpm add ")
        || lower.contains("yarn add ")
        || lower.contains("pip install")
    {
        Some(ApprovalKind::PackageInstall)
    } else if lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("curl ")
        || lower.contains("wget ")
        || lower.contains("fetch ")
        || lower.contains("download ")
    {
        Some(ApprovalKind::NetworkCall)
    } else if lower.contains("rm ")
        || lower.contains("write ")
        || lower.contains("edit ")
        || lower.contains("touch ")
        || lower.contains("create ")
        || lower.contains("mkdir ")
        || lower.contains("mv ")
        || lower.contains("cp ")
        || lower.contains("chmod ")
        || lower.contains("chown ")
        || lower.contains("delete ")
    {
        Some(ApprovalKind::FileWrite)
    } else if lower.contains("bash") || lower.contains("cargo ") || lower.contains("git ") {
        Some(ApprovalKind::ShellCommand)
    } else {
        None
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
