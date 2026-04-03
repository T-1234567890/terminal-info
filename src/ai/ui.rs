use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::ai::agent::{AgentState, ApprovalRequest, ApprovalState};
use crate::ai::api::LocalApi;
use crate::ai::chat::ProviderKind;
use crate::ai::ipc::start_event_log_consumer;
use crate::ai::runtime::{Runtime, RuntimeSnapshot};
use crate::live::run_live_loop_with_event_handler;
use crate::output::{OutputMode, set_output_mode};
use crate::theme::format_box_table_with_width;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum FocusPane {
    Agents,
    Approvals,
    Chat,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ViewMode {
    Dashboard,
    Agent,
    Chat,
}

struct UiState {
    focus: FocusPane,
    selected_agent: usize,
    selected_approval: usize,
    chat_input: String,
    scroll_offset: usize,
    snapshot: RuntimeSnapshot,
    agent_phase: AgentScreenPhase,
    empty_since: Option<Instant>,
    last_non_empty_snapshot: Option<RuntimeSnapshot>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AgentScreenPhase {
    Loading,
    Waiting,
    Empty,
    Active,
}

const EMPTY_STATE_DEBOUNCE: Duration = Duration::from_millis(600);

pub fn run_dashboard(
    runtime: Runtime,
    initial_focus: FocusPane,
    view_mode: ViewMode,
) -> Result<(), String> {
    start_event_log_consumer(runtime.clone());
    let api = LocalApi::new(
        runtime.config().api_bind(),
        runtime.config().web_enabled(),
        runtime.config().ui_refresh_ms(),
    );
    let (_api_server, api_status) = match api.start(runtime.clone()) {
        Ok(server) => (Some(server), runtime.config().api_bind().to_string()),
        Err(err) => (None, format!("disabled ({err})")),
    };

    if !io::stdout().is_terminal() {
        print_non_interactive_snapshot(&runtime.snapshot(), &api_status, view_mode);
        return Ok(());
    }

    set_output_mode(OutputMode::Color);

    let state = RefCell::new(UiState {
        focus: initial_focus,
        selected_agent: 0,
        selected_approval: 0,
        chat_input: String::new(),
        scroll_offset: 0,
        snapshot: canonicalize_snapshot(runtime.snapshot()),
        agent_phase: AgentScreenPhase::Loading,
        empty_since: None,
        last_non_empty_snapshot: None,
    });
    let refresh_interval = Duration::from_millis(runtime.config().ui_refresh_ms().max(100));

    run_live_loop_with_event_handler(
        refresh_interval,
        false,
        || {
            let mut ui = state.borrow_mut();
            refresh_ui_state(&runtime, &mut ui);
            let snapshot = ui.snapshot.clone();
            ui.selected_agent = clamp_index(ui.selected_agent, snapshot.agents.len());
            ui.selected_approval =
                clamp_index(ui.selected_approval, pending_requests(snapshot.approvals.as_slice()).len());
            Ok(render_dashboard(
                &snapshot,
                &ui.focus,
                ui.selected_agent,
                ui.selected_approval,
                &api_status,
                &ui.chat_input,
                view_mode,
                ui.scroll_offset,
                ui.agent_phase,
            ))
        },
        || {
            runtime.apply_timeouts()?;
            Ok(true)
        },
        |next| {
            let mut ui = state.borrow_mut();
            refresh_ui_state(&runtime, &mut ui);
            let snapshot = ui.snapshot.clone();
            ui.selected_agent = clamp_index(ui.selected_agent, snapshot.agents.len());
            ui.selected_approval =
                clamp_index(ui.selected_approval, pending_requests(snapshot.approvals.as_slice()).len());

            if let Event::Key(key) = next {
                if key.kind == KeyEventKind::Release {
                    return Ok(false);
                }
                match key.code {
                    KeyCode::Char('q') => return Ok(true),
                    KeyCode::Esc => return Ok(true),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(true);
                    }
                    KeyCode::PageUp if view_mode == ViewMode::Agent => {
                        ui.scroll_offset = ui.scroll_offset.saturating_sub(10);
                    }
                    KeyCode::PageDown if view_mode == ViewMode::Agent => {
                        ui.scroll_offset = ui.scroll_offset.saturating_add(10);
                    }
                    KeyCode::Home if view_mode == ViewMode::Agent => {
                        ui.scroll_offset = 0;
                    }
                    KeyCode::End if view_mode == ViewMode::Agent => {
                        ui.scroll_offset = usize::MAX / 4;
                    }
                    KeyCode::Tab => {
                        ui.focus = match ui.focus {
                            FocusPane::Agents => FocusPane::Approvals,
                            FocusPane::Approvals => FocusPane::Chat,
                            FocusPane::Chat => FocusPane::Agents,
                        };
                    }
                    KeyCode::Up | KeyCode::Char('k') if ui.focus == FocusPane::Agents => {
                        ui.selected_agent = ui.selected_agent.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') if ui.focus == FocusPane::Agents => {
                        if ui.selected_agent + 1 < snapshot.agents.len() {
                            ui.selected_agent += 1;
                        }
                    }
                    KeyCode::Char('s') => {
                        if let Some(agent) = snapshot.agents.get(ui.selected_agent) {
                            let _ = runtime.start_agent(&agent.id);
                        }
                    }
                    KeyCode::Char('x') => {
                        if let Some(agent) = snapshot.agents.get(ui.selected_agent) {
                            let _ = runtime.stop_agent(&agent.id);
                        }
                    }
                    KeyCode::Char('r') => {
                        if let Some(agent) = snapshot.agents.get(ui.selected_agent) {
                            let _ = runtime.restart_agent(&agent.id);
                        }
                    }
                    KeyCode::Char('p') => {
                        if let Some(agent) = snapshot.agents.get(ui.selected_agent) {
                            let _ = runtime.pause_agent(&agent.id);
                        }
                    }
                    KeyCode::Char('u') => {
                        if let Some(agent) = snapshot.agents.get(ui.selected_agent) {
                            let _ = runtime.resume_agent(&agent.id);
                        }
                    }
                    KeyCode::Enter if ui.focus == FocusPane::Chat => {
                        handle_chat_submit(&runtime, &mut ui.chat_input)?;
                    }
                    KeyCode::Backspace if ui.focus == FocusPane::Chat => {
                        ui.chat_input.pop();
                    }
                    KeyCode::Char('g') if ui.focus == FocusPane::Chat => {
                        if let Some(session) = snapshot.active_session.as_ref() {
                            let selected =
                                snapshot.agents.get(ui.selected_agent).map(|agent| agent.id.clone());
                            let _ = runtime.send_chat_to_agent(session.id(), selected);
                        }
                    }
                    KeyCode::Char('n') if ui.focus == FocusPane::Chat => {
                        let _ = runtime.create_chat_session(None, None, None);
                    }
                    KeyCode::Char('1') if ui.focus == FocusPane::Chat => {
                        if let Some(session) = snapshot.active_session.as_ref() {
                            let _ = runtime.ensure_chat_session(
                                Some(session.id().to_string()),
                                Some(ProviderKind::OpenAi),
                                None,
                                None,
                            );
                        }
                    }
                    KeyCode::Char('2') if ui.focus == FocusPane::Chat => {
                        if let Some(session) = snapshot.active_session.as_ref() {
                            let _ = runtime.ensure_chat_session(
                                Some(session.id().to_string()),
                                Some(ProviderKind::Anthropic),
                                None,
                                None,
                            );
                        }
                    }
                    KeyCode::Char('3') if ui.focus == FocusPane::Chat => {
                        if let Some(session) = snapshot.active_session.as_ref() {
                            let _ = runtime.ensure_chat_session(
                                Some(session.id().to_string()),
                                Some(ProviderKind::OpenRouter),
                                None,
                                None,
                            );
                        }
                    }
                    KeyCode::Char(ch)
                        if ui.focus == FocusPane::Chat
                            && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        ui.chat_input.push(ch);
                    }
                    _ => {}
                }
            }
            Ok(false)
        },
    )
}

fn handle_chat_submit(runtime: &Runtime, input: &mut String) -> Result<(), String> {
    let value = input.trim().to_string();
    if value.is_empty() {
        return Ok(());
    }

    let payload = std::mem::take(input);
    if let Some(command) = payload.trim().strip_prefix("/provider ") {
        let provider = ProviderKind::from_label(command.trim());
        let snapshot = runtime.snapshot();
        if let Some(session) = snapshot.active_session.as_ref() {
            runtime.ensure_chat_session(Some(session.id().to_string()), Some(provider), None, None)?;
        } else {
            let _ = runtime.create_chat_session(Some(provider), None, None)?;
        }
        return Ok(());
    }
    if let Some(model) = payload.trim().strip_prefix("/model ") {
        let snapshot = runtime.snapshot();
        if let Some(session) = snapshot.active_session.as_ref() {
            runtime.ensure_chat_session(
                Some(session.id().to_string()),
                None,
                Some(model.trim().to_string()),
                None,
            )?;
        }
        return Ok(());
    }
    if let Some(prompt) = payload.trim().strip_prefix("/system ") {
        let snapshot = runtime.snapshot();
        if let Some(session) = snapshot.active_session.as_ref() {
            runtime.ensure_chat_session(
                Some(session.id().to_string()),
                None,
                None,
                Some(prompt.trim().to_string()),
            )?;
        }
        return Ok(());
    }

    let snapshot = runtime.snapshot();
    let session_id = if let Some(session) = snapshot.active_session.as_ref() {
        session.id().to_string()
    } else {
        runtime.create_chat_session(None, None, None)?.id().to_string()
    };
    runtime.send_chat_message(&session_id, payload)
}

fn render_dashboard(
    snapshot: &RuntimeSnapshot,
    focus: &FocusPane,
    selected_agent: usize,
    selected_approval: usize,
    api_bind: &str,
    chat_input: &str,
    view_mode: ViewMode,
    scroll_offset: usize,
    agent_phase: AgentScreenPhase,
) -> String {
    match view_mode {
        ViewMode::Agent => {
            return render_agent_manager(
                snapshot,
                selected_approval,
                api_bind,
                scroll_offset,
                agent_phase,
            );
        }
        ViewMode::Chat => {
            return render_chat_manager(snapshot, focus, selected_agent, chat_input, api_bind);
        }
        ViewMode::Dashboard => {}
    }

    let active_chat = snapshot
        .active_session
        .as_ref()
        .map(|session| format!("{} / {}", session.provider().label(), session.model()))
        .unwrap_or_else(|| "no session".to_string());
    let agent_count = snapshot.agents.len().to_string();
    let pending_count = snapshot
        .approvals
        .iter()
        .filter(|item| item.state == ApprovalState::Pending)
        .count()
        .to_string();
    let header = [
        ("Dashboard", "ai control layer"),
        ("Agents", agent_count.as_str()),
        ("Approvals", pending_count.as_str()),
        ("Chat", active_chat.as_str()),
        ("API", api_bind),
    ];

    let mut body = String::new();
    body.push_str(&format_box_table_with_width("ai", &header, None));

    let width = terminal::size()
        .ok()
        .map(|(w, _)| w as usize)
        .unwrap_or(120);
    let agents = render_agents(snapshot, *focus == FocusPane::Agents, selected_agent, width);
    let approvals = render_approvals(
        snapshot,
        *focus == FocusPane::Approvals,
        selected_approval,
        width,
    );
    let status = render_status(snapshot, width);
    let activity = render_activity(snapshot, selected_agent, width);
    let chat = render_chat(snapshot, *focus == FocusPane::Chat, chat_input, width);
    let logs = render_logs(snapshot, width);

    if width >= 120 {
        body.push_str(&render_two_column_sections(&agents, &approvals, 3));
    } else {
        body.push_str(&agents);
        body.push('\n');
        body.push_str(&approvals);
    }
    body.push('\n');
    if width >= 120 {
        body.push_str(&render_two_column_sections(&status, &activity, 3));
    } else {
        body.push_str(&status);
        body.push('\n');
        body.push_str(&activity);
    }
    body.push('\n');
    body.push_str(&chat);
    body.push('\n');
    body.push_str(&logs);
    body.push('\n');
    body.push_str(
        "Keys: Tab pane · s start · x stop · p pause · u resume · a approve · d deny · Enter send · g send to agent · 1/2/3 provider · n new chat",
    );
    body.push('\n');
    body.push_str("Press q or Ctrl+C to exit");
    body
}

fn render_agent_manager(
    snapshot: &RuntimeSnapshot,
    _selected_approval: usize,
    _api_bind: &str,
    scroll_offset: usize,
    phase: AgentScreenPhase,
) -> String {
    if phase == AgentScreenPhase::Loading {
        return [
            "ai agent".to_string(),
            String::new(),
            "Loading agents...".to_string(),
            String::new(),
            "Press q or Ctrl+C to exit".to_string(),
        ]
        .join("\n");
    }

    if phase == AgentScreenPhase::Waiting {
        return [
            "ai agent".to_string(),
            String::new(),
            "Waiting for agent...".to_string(),
            "Tip: Start `tinfo codex` or `tinfo claude-code` while this screen is open.".to_string(),
            String::new(),
            "Press q or Ctrl+C to exit".to_string(),
        ]
        .join("\n");
    }

    if phase == AgentScreenPhase::Empty {
        return [
            "ai agent".to_string(),
            String::new(),
            "No agent available, try running Codex, Gemini, or Claude".to_string(),
            String::new(),
            "Press q or Ctrl+C to exit".to_string(),
        ]
        .join("\n");
    }

    let mut lines = Vec::new();
    lines.push("ai agent".to_string());
    lines.push("Manage external CLI agents with live status, approvals, and activity.".to_string());
    lines.push("Tip: Start `tinfo codex` or `tinfo claude-code` while this screen is open.".to_string());
    lines.push(String::new());

    if snapshot
        .agents
        .iter()
        .any(|agent| agent.adapter_type == "gemini")
    {
        lines.push("[Experimental] Gemini does not support hooks. Behavior may be unstable.".to_string());
        lines.push(String::new());
    }

    let pending = pending_requests(snapshot.approvals.as_slice());
    if pending.is_empty() {
        lines.push("No pending approvals.".to_string());
    } else {
        for request in pending {
            let agent_label = snapshot
                .agents
                .iter()
                .enumerate()
                .find(|(_, agent)| agent.id == request.agent_id)
                .map(|(idx, agent)| agent_instance_label(agent, idx))
                .unwrap_or(request.agent_id.clone());
            lines.push(format!("Agent is waiting for approval in {agent_label}"));
            lines.push("→ Go to terminal to approve".to_string());
            lines.push(request.action.clone());
            if let Some(details) = request.details.as_deref().filter(|value| !value.trim().is_empty()) {
                lines.push(details.to_string());
            }
            lines.push(String::new());
        }
    }

    for (idx, agent) in snapshot.agents.iter().enumerate() {
        lines.push(render_agent_summary_line(snapshot, agent, idx));
        lines.push(String::new());
    }

    lines.push("Keys: PgUp/PgDn scroll".to_string());
    lines.push("Press q or Ctrl+C to exit".to_string());
    render_scrolling_lines(lines, scroll_offset)
}

fn refresh_ui_state(runtime: &Runtime, ui: &mut UiState) {
    let latest = canonicalize_snapshot(runtime.snapshot());
    let now = Instant::now();

    if latest.hook_event_count == 0 {
        ui.snapshot = latest;
        ui.agent_phase = AgentScreenPhase::Waiting;
        ui.empty_since = None;
        return;
    }

    if latest.agents.is_empty() {
        let empty_since = ui.empty_since.get_or_insert(now);
        if let Some(previous) = ui.last_non_empty_snapshot.clone() {
            if now.duration_since(*empty_since) >= EMPTY_STATE_DEBOUNCE {
                ui.snapshot = latest;
                ui.agent_phase = AgentScreenPhase::Empty;
            } else {
                ui.snapshot = previous;
                ui.agent_phase = AgentScreenPhase::Active;
            }
        } else if now.duration_since(*empty_since) >= EMPTY_STATE_DEBOUNCE {
            ui.snapshot = latest;
            ui.agent_phase = AgentScreenPhase::Empty;
        } else {
            ui.snapshot = latest;
            ui.agent_phase = AgentScreenPhase::Loading;
        }
        return;
    }

    ui.empty_since = None;
    ui.agent_phase = AgentScreenPhase::Active;
    ui.last_non_empty_snapshot = Some(latest.clone());
    ui.snapshot = latest;
}

fn canonicalize_snapshot(mut snapshot: RuntimeSnapshot) -> RuntimeSnapshot {
    let mut agents = BTreeMap::new();
    for agent in snapshot.agents {
        agents.insert(agent.id.clone(), agent);
    }
    snapshot.agents = agents.into_values().collect();
    snapshot
}

fn render_chat_manager(
    snapshot: &RuntimeSnapshot,
    focus: &FocusPane,
    selected_agent: usize,
    chat_input: &str,
    api_bind: &str,
) -> String {
    let active_chat = snapshot
        .active_session
        .as_ref()
        .map(|session| format!("{} / {}", session.provider().label(), session.model()))
        .unwrap_or_else(|| "no session".to_string());
    let header = [
        ("Chat", active_chat.as_str()),
        ("Agents", &snapshot.agents.len().to_string()),
        ("API", api_bind),
    ];

    let width = terminal::size()
        .ok()
        .map(|(w, _)| w as usize)
        .unwrap_or(120);
    let mut body = String::new();
    body.push_str(&format_box_table_with_width("ai chat", &header, None));
    body.push_str(&render_chat(snapshot, *focus == FocusPane::Chat, chat_input, width));
    body.push('\n');
    body.push_str(&render_activity(snapshot, selected_agent, width));
    body.push('\n');
    body.push_str("Keys: Enter send · g send to agent · 1/2/3 provider · n new chat\n");
    body.push_str("Press q or Ctrl+C to exit");
    body
}

fn render_agents(
    snapshot: &RuntimeSnapshot,
    focused: bool,
    selected_agent: usize,
    width: usize,
) -> String {
    let mut rows = Vec::new();
    if snapshot.agents.is_empty() {
        rows.push((
            "Status".to_string(),
            "No agent CLIs configured. Add [ai.adapters.*] or [ai.agents.*] to ~/.tinfo/config.toml"
                .to_string(),
        ));
    } else {
        for (idx, agent) in snapshot.agents.iter().enumerate() {
            let pointer = if focused && idx == selected_agent {
                ">"
            } else {
                " "
            };
            let label = format!("{} {}", pointer, agent_instance_label(agent, idx));
            rows.push((
                label,
                format!(
                    "{}{}",
                    agent_status_label(agent),
                    current_activity_suffix(snapshot, agent)
                ),
            ));
        }
    }
    format_box_table_with_width("Agent Manager", &rows, Some(section_width(width)))
}

fn render_approvals(
    snapshot: &RuntimeSnapshot,
    focused: bool,
    selected_approval: usize,
    width: usize,
) -> String {
    let mut rows = Vec::new();
    let pending = pending_requests(snapshot.approvals.as_slice());
    if pending.is_empty() {
        rows.push(("Queue".to_string(), "No approval requests".to_string()));
    } else {
        for (idx, request) in pending.iter().enumerate() {
            let pointer = if focused && idx == selected_approval {
                ">"
            } else {
                " "
            };
            let agent_label = snapshot
                .agents
                .iter()
                .enumerate()
                .find(|(_, agent)| agent.id == request.agent_id)
                .map(|(agent_idx, agent)| agent_instance_label(agent, agent_idx))
                .unwrap_or_else(|| request.agent_id.clone());
            let title = if request.state == ApprovalState::Pending {
                format!("{pointer} {agent_label} wants to:")
            } else {
                format!("{pointer} {agent_label} {}", approval_state_label(request.state))
            };
            rows.push((
                title,
                approval_display(request),
            ));
        }
    }
    format_box_table_with_width("Approvals", &rows, Some(section_width(width)))
}

fn render_status(snapshot: &RuntimeSnapshot, width: usize) -> String {
    let mut rows = Vec::new();
    if snapshot.agents.is_empty() {
        rows.push(("Status".to_string(), "No configured local CLIs".to_string()));
    } else {
        for agent in &snapshot.agents {
            rows.push((
                short_agent_name(agent).to_string(),
                agent_status_label(agent),
            ));
        }
    }
    format_box_table_with_width("Status", &rows, Some(section_width(width)))
}

fn render_activity(snapshot: &RuntimeSnapshot, selected_agent: usize, width: usize) -> String {
    let mut rows = Vec::new();
    if snapshot.agents.is_empty() {
        rows.push(("Activity".to_string(), "No live activity yet".to_string()));
    } else {
        for (idx, agent) in snapshot.agents.iter().enumerate() {
            let summary = current_activity(snapshot, agent).unwrap_or_default();
            let label = if idx == selected_agent {
                format!("> {}", agent_instance_label(agent, idx))
            } else {
                format!("  {}", agent_instance_label(agent, idx))
            };
            rows.push((label, summary));
        }
    }
    format_box_table_with_width("Live Activity", &rows, Some(section_width(width)))
}

fn render_chat(
    snapshot: &RuntimeSnapshot,
    focused: bool,
    chat_input: &str,
    width: usize,
) -> String {
    let mut rows = Vec::new();
    if let Some(session) = snapshot.active_session.as_ref() {
        for message in session.messages().iter().rev().take(8).rev() {
            rows.push((
                message_role_label(message.role).to_string(),
                message.content.clone(),
            ));
        }
        if session.streaming() {
            rows.push(("status".to_string(), "streaming response…".to_string()));
        }
        if let Some(err) = session.last_error() {
            rows.push(("error".to_string(), err.to_string()));
        }
    } else {
        rows.push(("Chat".to_string(), "No active chat session".to_string()));
    }
    rows.push((
        if focused { "input >".to_string() } else { "input".to_string() },
        chat_input.to_string(),
    ));
    format_box_table_with_width("Chat", &rows, Some(width.saturating_sub(4)))
}

fn render_logs(snapshot: &RuntimeSnapshot, width: usize) -> String {
    let mut rows = Vec::new();
    if snapshot.logs.is_empty() {
        rows.push(("Logs".to_string(), "No agent output yet".to_string()));
    } else {
        for entry in snapshot.logs.iter().rev().take(10).rev() {
            rows.push((
                format!("{} {}", short_time(entry.timestamp), entry.agent_id),
                entry.message.clone(),
            ));
        }
    }
    format_box_table_with_width("Stream", &rows, Some(width.saturating_sub(4)))
}

fn render_two_column_sections(left: &str, right: &str, gap: usize) -> String {
    let left_lines = left.lines().collect::<Vec<_>>();
    let right_lines = right.lines().collect::<Vec<_>>();
    let left_width = left_lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let height = left_lines.len().max(right_lines.len());
    let mut lines = Vec::new();

    for idx in 0..height {
        let left = left_lines
            .get(idx)
            .map(|line| line.to_string())
            .unwrap_or_else(|| " ".repeat(left_width));
        let right = right_lines.get(idx).copied().unwrap_or("");
        lines.push(format!("{left:<left_width$}{}{}", " ".repeat(gap), right));
    }

    format!("{}\n", lines.join("\n"))
}

fn pending_requests(requests: &[ApprovalRequest]) -> Vec<ApprovalRequest> {
    requests
        .iter()
        .filter(|request| request.state == ApprovalState::Pending)
        .cloned()
        .collect()
}

fn approval_state_label(state: ApprovalState) -> &'static str {
    match state {
        ApprovalState::Pending => "pending",
        ApprovalState::Approved => "approved",
        ApprovalState::Denied => "denied",
    }
}

fn message_role_label(role: crate::ai::chat::ChatRole) -> &'static str {
    match role {
        crate::ai::chat::ChatRole::System => "system",
        crate::ai::chat::ChatRole::User => "user",
        crate::ai::chat::ChatRole::Assistant => "assistant",
    }
}

fn section_width(width: usize) -> usize {
    if width >= 120 {
        width.saturating_sub(10) / 2
    } else {
        width.saturating_sub(4)
    }
}

fn short_time(value: u64) -> String {
    value.to_string()
}

fn short_agent_name(agent: &crate::ai::agent::AgentSession) -> &str {
    match agent.display_name.as_str() {
        "Codex CLI" => "Codex",
        "Claude Code" => "Claude Code",
        "Gemini CLI" => "Gemini CLI",
        "Generic Agent CLI" => "Agent",
        other => other,
    }
}

fn agent_instance_label(agent: &crate::ai::agent::AgentSession, index: usize) -> String {
    format!("{} ({})", short_agent_name(agent), index + 1)
}

fn agent_state_label(state: AgentState) -> &'static str {
    match state {
        AgentState::Running => "running",
        AgentState::Waiting => "waiting",
        AgentState::Idle => "idle",
        AgentState::Error => "error",
        AgentState::Paused => "paused",
    }
}

fn agent_status_label(agent: &crate::ai::agent::AgentSession) -> String {
    agent_state_label(agent.state).to_string()
}

fn approval_display(request: &ApprovalRequest) -> String {
    if let Some(details) = request.details.as_deref().filter(|value| !value.trim().is_empty()) {
        format!("{}\n{}", request.action, details)
    } else {
        request.action.clone()
    }
}

fn current_activity_suffix(
    snapshot: &RuntimeSnapshot,
    agent: &crate::ai::agent::AgentSession,
) -> String {
    current_activity(snapshot, agent)
        .map(|summary| format!(" · {summary}"))
        .unwrap_or_default()
}

fn current_activity(
    snapshot: &RuntimeSnapshot,
    agent: &crate::ai::agent::AgentSession,
) -> Option<String> {
    if agent_has_pending_approval(snapshot, agent) {
        return None;
    }

    if let Some(task) = agent.current_task.as_ref() {
        return Some(task.description.clone());
    }

    if let Some(output) = agent
        .last_output
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(output.to_string());
    }

    agent.last_error.clone()
}

fn render_agent_summary_line(
    snapshot: &RuntimeSnapshot,
    agent: &crate::ai::agent::AgentSession,
    index: usize,
) -> String {
    let label = agent_instance_label(agent, index);
    let summary = if agent_has_pending_approval(snapshot, agent) {
        "pending approval".to_string()
    } else if let Some(activity) = display_activity_summary(snapshot, agent) {
        activity
    } else {
        agent_dashboard_status(snapshot, agent).to_string()
    };
    format!("{label}\n{summary}")
}

fn agent_dashboard_status(
    snapshot: &RuntimeSnapshot,
    agent: &crate::ai::agent::AgentSession,
) -> &'static str {
    if agent_has_pending_approval(snapshot, agent) {
        "pending approval"
    } else if is_editing_files(snapshot, agent) {
        "editing files"
    } else if matches!(agent.state, AgentState::Idle | AgentState::Paused) {
        "idle"
    } else {
        "reasoning"
    }
}

fn display_activity_summary(
    snapshot: &RuntimeSnapshot,
    agent: &crate::ai::agent::AgentSession,
) -> Option<String> {
    if agent_has_pending_approval(snapshot, agent) {
        return Some("pending approval".to_string());
    }

    if is_editing_files(snapshot, agent) {
        return Some("editing files...".to_string());
    }

    if matches!(agent.state, AgentState::Running) {
        return Some("reasoning...".to_string());
    }

    if matches!(agent.state, AgentState::Idle | AgentState::Paused) {
        return Some("idle".to_string());
    }

    if matches!(agent.state, AgentState::Error) {
        return Some("error".to_string());
    }

    None
}

fn agent_has_pending_approval(
    snapshot: &RuntimeSnapshot,
    agent: &crate::ai::agent::AgentSession,
) -> bool {
    snapshot
        .approvals
        .iter()
        .any(|request| request.agent_id == agent.id && request.state == ApprovalState::Pending)
}

fn is_editing_files(
    snapshot: &RuntimeSnapshot,
    agent: &crate::ai::agent::AgentSession,
) -> bool {
    let task_text = agent
        .current_task
        .as_ref()
        .map(|task| task.description.as_str())
        .or(agent.last_output.as_deref())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let request_text = snapshot
        .approvals
        .iter()
        .rev()
        .find(|request| request.agent_id == agent.id && request.state == ApprovalState::Pending)
        .map(|request| request.action.to_ascii_lowercase())
        .unwrap_or_default();

    let combined = format!("{task_text}\n{request_text}");
    combined.contains("touch ")
        || combined.contains("rm ")
        || combined.contains("mv ")
        || combined.contains("cp ")
        || combined.contains("mkdir ")
        || combined.contains("write ")
        || combined.contains("edit ")
        || combined.contains("delete ")
}

fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { index.min(len - 1) }
}

fn render_scrolling_lines(lines: Vec<String>, scroll_offset: usize) -> String {
    let width = terminal::size()
        .ok()
        .map(|(w, _)| w as usize)
        .unwrap_or(120)
        .max(1);
    let height = terminal::size()
        .ok()
        .map(|(_, h)| h as usize)
        .unwrap_or(30)
        .max(1);

    let wrapped = lines
        .into_iter()
        .flat_map(|line| wrap_line(&line, width))
        .collect::<Vec<_>>();
    let max_start = wrapped.len().saturating_sub(height);
    let start = scroll_offset.min(max_start);
    let end = (start + height).min(wrapped.len());

    wrapped[start..end].join("\n")
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    if width <= 1 {
        return line.chars().map(|ch| ch.to_string()).collect();
    }

    let chars = line.chars().collect::<Vec<_>>();
    let mut wrapped = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + width).min(chars.len());
        wrapped.push(chars[start..end].iter().collect());
        start = end;
    }

    wrapped
}

fn print_non_interactive_snapshot(
    snapshot: &RuntimeSnapshot,
    api_status: &str,
    view_mode: ViewMode,
) {
    match view_mode {
        ViewMode::Agent => {
            println!("ai agent");
            println!("Agents: {}", snapshot.agents.len());
            println!(
                "Pending approvals: {}",
                snapshot
                    .approvals
                    .iter()
                    .filter(|item| item.state == ApprovalState::Pending)
                    .count()
            );
        }
        ViewMode::Chat => {
            println!("ai chat");
            println!("API: {api_status}");
            println!("Chat sessions: {}", snapshot.sessions.len());
            if let Some(session) = snapshot.active_session.as_ref() {
                println!(
                    "Active session: {}/{}",
                    session.provider().label(),
                    session.model()
                );
            }
        }
        ViewMode::Dashboard => {
            println!("ai");
            println!("Agents: {}", snapshot.agents.len());
            println!("API: {api_status}");
            println!(
                "Pending approvals: {}",
                snapshot
                    .approvals
                    .iter()
                    .filter(|item| item.state == ApprovalState::Pending)
                    .count()
            );
            println!("Chat sessions: {}", snapshot.sessions.len());
        }
    }
}
