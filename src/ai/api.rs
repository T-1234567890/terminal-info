use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::ai::agent::ApprovalKind;
use crate::ai::chat::ProviderKind;
use crate::ai::hook::HookEventPayload;
use crate::ai::runtime::{Runtime, RuntimeEvent};
use crate::ai::web;

#[derive(Debug, Clone)]
pub struct LocalApi {
    bind_address: String,
    web_enabled: bool,
    refresh_ms: u64,
}

pub struct LocalApiServer {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl LocalApi {
    pub fn new(bind_address: impl Into<String>, web_enabled: bool, refresh_ms: u64) -> Self {
        Self {
            bind_address: bind_address.into(),
            web_enabled,
            refresh_ms,
        }
    }

    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }

    pub fn start(&self, runtime: Runtime) -> Result<LocalApiServer, String> {
        let listener = TcpListener::bind(&self.bind_address)
            .map_err(|err| format!("Failed to bind local API {}: {err}", self.bind_address))?;
        listener
            .set_nonblocking(true)
            .map_err(|err| format!("Failed to configure local API socket: {err}"))?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let web_enabled = self.web_enabled;
        let refresh_ms = self.refresh_ms;
        let handle = thread::spawn(move || loop {
            if stop_flag.load(Ordering::SeqCst) {
                return;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = handle_connection(&mut stream, &runtime, web_enabled, refresh_ms);
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(_) => return,
            }
        });

        Ok(LocalApiServer {
            stop,
            handle: Some(handle),
        })
    }
}

impl Drop for LocalApiServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Deserialize)]
struct ApprovalDecisionBody {
    id: String,
}

#[derive(Deserialize)]
struct ApprovalRequestBody {
    agent_id: String,
    action: String,
    details: Option<String>,
    kind: Option<String>,
}

#[derive(Deserialize)]
struct ExternalLogBody {
    agent_id: String,
    level: Option<String>,
    message: String,
}

#[derive(Deserialize)]
struct ExternalEventBody {
    agent_id: String,
    event_type: String,
    message: Option<String>,
}

#[derive(Deserialize)]
struct ChatSessionBody {
    session_id: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    system_prompt: Option<String>,
}

#[derive(Deserialize)]
struct ChatMessageBody {
    session_id: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatAgentBody {
    session_id: String,
    agent_id: Option<String>,
}

fn handle_connection(
    stream: &mut TcpStream,
    runtime: &Runtime,
    web_enabled: bool,
    refresh_ms: u64,
) -> Result<(), String> {
    let mut buffer = [0_u8; 64 * 1024];
    let read = stream
        .read(&mut buffer)
        .map_err(|err| format!("Failed to read local API request: {err}"))?;
    if read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "Missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    let path_only = path.split('?').next().unwrap_or(path);

    if web_enabled {
        match (method, path_only) {
            ("GET", "/") => return write_html(stream, &web::index_html(refresh_ms)),
            ("GET", "/index.html") => return write_html(stream, &web::index_html(refresh_ms)),
            ("GET", "/stream") => return write_snapshot_stream(stream, runtime, refresh_ms),
            _ => {}
        }
    }

    if let Some(agent_id) = path_only
        .strip_prefix("/agents/")
        .and_then(|tail| tail.split('/').next())
    {
        let suffix = path_only.trim_start_matches("/agents/").trim_start_matches(agent_id);
        return match (method, suffix) {
            ("GET", "") => {
                if let Some(agent) = runtime.agent(agent_id) {
                    write_json(stream, &agent)
                } else {
                    write_text(stream, "404 Not Found", "agent not found")
                }
            }
            ("POST", "/start") => {
                runtime.start_agent(agent_id)?;
                write_text(stream, "200 OK", "started")
            }
            ("POST", "/pause") => {
                runtime.pause_agent(agent_id)?;
                write_text(stream, "200 OK", "paused")
            }
            ("POST", "/resume") => {
                runtime.resume_agent(agent_id)?;
                write_text(stream, "200 OK", "resumed")
            }
            ("POST", "/stop") => {
                runtime.stop_agent(agent_id)?;
                write_text(stream, "200 OK", "stopped")
            }
            _ => write_text(stream, "404 Not Found", "not found"),
        };
    }

    match (method, path_only) {
        ("GET", "/agents") => write_json(stream, &runtime.snapshot().agents),
        ("GET", "/logs") => write_json(stream, &runtime.snapshot().logs),
        ("GET", "/events") => write_json(stream, &runtime.snapshot().events),
        ("GET", "/approvals") => write_json(stream, &runtime.snapshot().approvals),
        ("GET", "/chat/session") => write_json(stream, &runtime.chat_sessions_payload()),
        ("POST", "/approve") => {
            let payload: ApprovalDecisionBody = serde_json::from_str(body)
                .map_err(|err| format!("Invalid /approve payload: {err}"))?;
            runtime.approve_request(&payload.id)?;
            write_text(stream, "200 OK", "approved")
        }
        ("POST", "/deny") => {
            let payload: ApprovalDecisionBody = serde_json::from_str(body)
                .map_err(|err| format!("Invalid /deny payload: {err}"))?;
            runtime.deny_request(&payload.id)?;
            write_text(stream, "200 OK", "denied")
        }
        ("POST", "/approvals/request") => {
            let payload: ApprovalRequestBody = serde_json::from_str(body)
                .map_err(|err| format!("Invalid approval request payload: {err}"))?;
            let request = runtime.add_approval_request(
                payload.agent_id,
                parse_approval_kind(payload.kind.as_deref()),
                payload.action,
                payload.details,
            );
            write_json(stream, &request)
        }
        ("POST", "/logs") => {
            let payload: ExternalLogBody =
                serde_json::from_str(body).map_err(|err| format!("Invalid log payload: {err}"))?;
            runtime.append_external_log(
                payload.agent_id,
                payload.level.unwrap_or_else(|| "info".to_string()),
                payload.message,
            );
            write_text(stream, "200 OK", "logged")
        }
        ("POST", "/events") => {
            let payload: ExternalEventBody = serde_json::from_str(body)
                .map_err(|err| format!("Invalid event payload: {err}"))?;
            runtime.append_external_event(event_from_body(payload));
            write_text(stream, "200 OK", "event recorded")
        }
        ("POST", "/hook/event") => {
            let payload: HookEventPayload = serde_json::from_str(body)
                .map_err(|err| format!("Invalid hook event payload: {err}"))?;
            runtime.ingest_hook_event(payload)?;
            write_text(stream, "200 OK", "hook event recorded")
        }
        ("POST", "/chat/session") => {
            let payload: ChatSessionBody = serde_json::from_str(body)
                .map_err(|err| format!("Invalid chat session payload: {err}"))?;
            let session_id = runtime.ensure_chat_session(
                payload.session_id,
                payload.provider.as_deref().map(ProviderKind::from_label),
                payload.model,
                payload.system_prompt,
            )?;
            let session = runtime
                .chat_sessions_payload()
                .sessions
                .into_iter()
                .find(|session| session.id() == session_id)
                .ok_or_else(|| "Chat session was not created.".to_string())?;
            write_json(stream, &session)
        }
        ("POST", "/chat/message") => {
            let payload: ChatMessageBody = serde_json::from_str(body)
                .map_err(|err| format!("Invalid chat message payload: {err}"))?;
            runtime.send_chat_message(&payload.session_id, payload.content)?;
            write_text(stream, "200 OK", "queued")
        }
        ("POST", "/chat/send-to-agent") => {
            let payload: ChatAgentBody = serde_json::from_str(body)
                .map_err(|err| format!("Invalid send-to-agent payload: {err}"))?;
            let task = runtime.send_chat_to_agent(&payload.session_id, payload.agent_id)?;
            write_json(stream, &task)
        }
        _ => write_text(stream, "404 Not Found", "not found"),
    }
}

fn write_snapshot_stream(
    stream: &mut TcpStream,
    runtime: &Runtime,
    refresh_ms: u64,
) -> Result<(), String> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\n\r\n"
    )
    .map_err(|err| format!("Failed to start live stream response: {err}"))?;
    stream
        .flush()
        .map_err(|err| format!("Failed to flush live stream headers: {err}"))?;

    let interval = Duration::from_millis(refresh_ms.max(100));
    loop {
        let payload = serde_json::to_string(&snapshot_payload(runtime))
            .map_err(|err| format!("Failed to encode live stream payload: {err}"))?;
        if write!(stream, "event: snapshot\r\ndata: {payload}\r\n\r\n")
            .and_then(|_| stream.flush())
            .is_err()
        {
            return Ok(());
        }
        thread::sleep(interval);
    }
}

fn snapshot_payload(runtime: &Runtime) -> serde_json::Value {
    let snapshot = runtime.snapshot();
    let chat = runtime.chat_sessions_payload();
    json!({
        "agents": snapshot.agents,
        "approvals": snapshot.approvals,
        "logs": snapshot.logs,
        "events": snapshot.events,
        "chat": chat,
    })
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

fn event_from_body(payload: ExternalEventBody) -> RuntimeEvent {
    match payload.event_type.as_str() {
        "step_started" => RuntimeEvent::StepStarted {
            agent_id: payload.agent_id,
            step: payload.message.unwrap_or_else(|| "step".to_string()),
        },
        "tool_called" => RuntimeEvent::ToolCalled {
            agent_id: payload.agent_id,
            tool: payload.message.unwrap_or_else(|| "tool".to_string()),
        },
        "waiting_approval" => RuntimeEvent::WaitingApproval {
            agent_id: payload.agent_id,
            request_id: payload.message.unwrap_or_else(|| "request".to_string()),
        },
        "finished" => RuntimeEvent::Finished {
            agent_id: payload.agent_id,
        },
        "error" => RuntimeEvent::Error {
            agent_id: payload.agent_id,
            message: payload.message.unwrap_or_else(|| "error".to_string()),
        },
        _ => RuntimeEvent::OutputStream {
            agent_id: payload.agent_id,
            chunk: payload.message.unwrap_or_else(|| "event".to_string()),
        },
    }
}

fn write_json<T: serde::Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), String> {
    let body =
        serde_json::to_string_pretty(value).map_err(|err| format!("JSON encode failed: {err}"))?;
    write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
}

fn write_text(stream: &mut TcpStream, status: &str, body: &str) -> Result<(), String> {
    write_response(stream, status, "text/plain; charset=utf-8", body)
}

fn write_html(stream: &mut TcpStream, body: &str) -> Result<(), String> {
    write_response(stream, "200 OK", "text/html; charset=utf-8", body)
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("Failed to write local API response: {err}"))
}
