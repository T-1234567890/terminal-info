use std::io::{BufRead, BufReader};

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ai::config::AiConfig;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    OpenRouter,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "claude",
            Self::OpenRouter => "openrouter",
        }
    }

    pub fn config_key(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "claude",
            Self::OpenRouter => "openrouter",
        }
    }

    pub fn secret_key_name(self) -> &'static str {
        match self {
            Self::OpenAi => "openai_api_key",
            Self::Anthropic => "claude_api_key",
            Self::OpenRouter => "openrouter_api_key",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Claude",
            Self::OpenRouter => "OpenRouter",
        }
    }

    pub fn from_label(value: &str) -> Self {
        match value {
            "claude" => Self::Anthropic,
            "anthropic" => Self::Anthropic,
            "openrouter" => Self::OpenRouter,
            _ => Self::OpenAi,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub trait ChatProvider: Send {
    fn send_message(
        &self,
        messages: Vec<Message>,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryMode {
    InMemory,
    Persisted,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatSession {
    id: String,
    provider: ProviderKind,
    model: String,
    history_mode: HistoryMode,
    system_prompt: Option<String>,
    messages: Vec<ChatMessage>,
    created_at: u64,
    updated_at: u64,
    streaming: bool,
    last_error: Option<String>,
}

impl ChatSession {
    pub fn new(
        id: impl Into<String>,
        provider: ProviderKind,
        model: impl Into<String>,
        history_mode: HistoryMode,
        system_prompt: Option<String>,
        now: u64,
    ) -> Self {
        Self {
            id: id.into(),
            provider,
            model: model.into(),
            history_mode,
            system_prompt,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            streaming: false,
            last_error: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn history_mode(&self) -> HistoryMode {
        self.history_mode
    }

    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn updated_at(&self) -> u64 {
        self.updated_at
    }

    pub fn streaming(&self) -> bool {
        self.streaming
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn set_provider(&mut self, provider: ProviderKind) {
        self.provider = provider;
    }

    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    pub fn set_system_prompt(&mut self, prompt: Option<String>) {
        self.system_prompt = prompt;
    }

    pub fn push_message(&mut self, role: ChatRole, content: impl Into<String>, now: u64) {
        self.messages.push(ChatMessage {
            role,
            content: content.into(),
            timestamp: now,
        });
        self.updated_at = now;
    }

    pub fn start_stream(&mut self, now: u64) {
        self.streaming = true;
        self.last_error = None;
        self.updated_at = now;
        self.messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            timestamp: now,
        });
    }

    pub fn append_stream_chunk(&mut self, chunk: &str, now: u64) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == ChatRole::Assistant {
                last.content.push_str(chunk);
                self.updated_at = now;
            }
        }
    }

    pub fn finish_stream(&mut self, now: u64) {
        self.streaming = false;
        self.updated_at = now;
    }

    pub fn fail_stream(&mut self, err: impl Into<String>, now: u64) {
        self.streaming = false;
        self.last_error = Some(err.into());
        self.updated_at = now;
    }

    pub fn latest_assistant_message(&self) -> Option<&str> {
        self.messages
            .iter()
            .rev()
            .find(|message| message.role == ChatRole::Assistant)
            .map(|message| message.content.as_str())
    }
}

pub fn build_provider(
    config: &AiConfig,
    provider: ProviderKind,
    model: impl Into<String>,
    system_prompt: Option<String>,
) -> Result<Box<dyn ChatProvider>, String> {
    let api_key = config
        .load_provider_api_key(provider)?
        .ok_or_else(|| format!("Missing API key for {}.", provider.label()))?;
    let endpoint = config
        .provider_config(provider)
        .endpoint
        .clone()
        .unwrap_or_else(|| default_endpoint(provider).to_string());
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Failed to create AI HTTP client: {err}"))?;

    let model = model.into();
    let provider: Box<dyn ChatProvider> = match provider {
        ProviderKind::OpenAi => Box::new(OpenAiProvider {
            client,
            endpoint,
            api_key,
            model,
            system_prompt,
        }),
        ProviderKind::Anthropic => Box::new(AnthropicProvider {
            client,
            endpoint,
            api_key,
            model,
            system_prompt,
        }),
        ProviderKind::OpenRouter => Box::new(OpenRouterProvider {
            client,
            endpoint,
            api_key,
            model,
            system_prompt,
        }),
    };
    Ok(provider)
}

pub fn complete_message(config: &AiConfig, session: &ChatSession) -> Result<String, String> {
    let provider_cfg = config.provider_config(session.provider());
    let api_key = config
        .load_provider_api_key(session.provider())?
        .ok_or_else(|| format!("Missing API key for {}.", session.provider().label()))?;
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Failed to create AI HTTP client: {err}"))?;

    match session.provider() {
        ProviderKind::OpenAi | ProviderKind::OpenRouter => complete_openai_like(
            &client,
            provider_cfg
                .endpoint
                .as_deref()
                .unwrap_or(default_endpoint(session.provider())),
            &api_key,
            session,
        ),
        ProviderKind::Anthropic => complete_anthropic(
            &client,
            provider_cfg
                .endpoint
                .as_deref()
                .unwrap_or(default_endpoint(session.provider())),
            &api_key,
            session,
        ),
    }
}

pub fn stream_message<F>(
    config: &AiConfig,
    session: &ChatSession,
    mut on_chunk: F,
) -> Result<(), String>
where
    F: FnMut(&str),
{
    let provider_cfg = config.provider_config(session.provider());
    let api_key = config
        .load_provider_api_key(session.provider())?
        .ok_or_else(|| format!("Missing API key for {}.", session.provider().label()))?;
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Failed to create AI HTTP client: {err}"))?;

    match session.provider() {
        ProviderKind::OpenAi | ProviderKind::OpenRouter => stream_openai_like(
            &client,
            provider_cfg
                .endpoint
                .as_deref()
                .unwrap_or(default_endpoint(session.provider())),
            &api_key,
            session,
            &mut on_chunk,
        ),
        ProviderKind::Anthropic => stream_anthropic(
            &client,
            provider_cfg
                .endpoint
                .as_deref()
                .unwrap_or(default_endpoint(session.provider())),
            &api_key,
            session,
            &mut on_chunk,
        ),
    }
}

fn complete_openai_like(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    session: &ChatSession,
) -> Result<String, String> {
    let mut messages = Vec::new();
    if let Some(system_prompt) = session.system_prompt() {
        messages.push(json!({
            "role": "system",
            "content": system_prompt,
        }));
    }
    for message in session.messages() {
        messages.push(json!({
            "role": role_label(message.role),
            "content": message.content,
        }));
    }

    let mut request = client
        .post(endpoint)
        .bearer_auth(api_key)
        .header("User-Agent", "tinfo");
    if session.provider() == ProviderKind::OpenRouter {
        request = request.headers(openrouter_attribution_headers());
    }
    let response = request
        .json(&json!({
            "model": session.model(),
            "messages": messages,
            "stream": false,
        }))
        .send()
        .map_err(|err| format!("Failed to contact {}: {err}", session.provider().label()))?
        .error_for_status()
        .map_err(|err| format!("{} request failed: {err}", session.provider().label()))?;
    let body: Value = response
        .json()
        .map_err(|err| format!("Failed to parse {} response: {err}", session.provider().label()))?;
    body.get("choices")
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("message"))
        .and_then(|value| value.get("content"))
        .and_then(Value::as_str)
        .map(|text| text.to_string())
        .ok_or_else(|| format!("{} response did not include assistant content.", session.provider().label()))
}

fn stream_openai_like<F>(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    session: &ChatSession,
    on_chunk: &mut F,
) -> Result<(), String>
where
    F: FnMut(&str) + ?Sized,
{
    let mut messages = Vec::new();
    if let Some(system_prompt) = session.system_prompt() {
        messages.push(json!({
            "role": "system",
            "content": system_prompt,
        }));
    }
    for message in session.messages() {
        messages.push(json!({
            "role": role_label(message.role),
            "content": message.content,
        }));
    }

    let mut request = client
        .post(endpoint)
        .bearer_auth(api_key)
        .header("User-Agent", "tinfo");
    if session.provider() == ProviderKind::OpenRouter {
        request = request.headers(openrouter_attribution_headers());
    }
    let response = request
        .json(&json!({
            "model": session.model(),
            "messages": messages,
            "stream": true,
        }))
        .send()
        .map_err(|err| format!("Failed to contact {}: {err}", session.provider().label()))?
        .error_for_status()
        .map_err(|err| format!("{} request failed: {err}", session.provider().label()))?;

    let mut saw_chunk = false;
    let reader = BufReader::new(response);
    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim();
        if !line.starts_with("data: ") {
            continue;
        }
        let payload = line.trim_start_matches("data: ").trim();
        if payload == "[DONE]" {
            break;
        }
        let Ok(body) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if let Some(content) = body
            .get("choices")
            .and_then(|value| value.get(0))
            .and_then(|value| value.get("delta"))
            .and_then(|value| value.get("content"))
            .and_then(Value::as_str)
        {
            if !content.is_empty() {
                saw_chunk = true;
                on_chunk(content);
            }
        }
    }

    if saw_chunk {
        Ok(())
    } else {
        let full = complete_openai_like(client, endpoint, api_key, session)?;
        if !full.is_empty() {
            on_chunk(&full);
        }
        Ok(())
    }
}

fn complete_anthropic(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    session: &ChatSession,
) -> Result<String, String> {
    let messages = session
        .messages()
        .iter()
        .filter(|message| message.role != ChatRole::System)
        .map(|message| {
            json!({
                "role": role_label(message.role),
                "content": message.content,
            })
        })
        .collect::<Vec<_>>();

    let response = client
        .post(endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("User-Agent", "tinfo")
        .json(&json!({
            "model": session.model(),
            "max_tokens": 1024,
            "system": session.system_prompt(),
            "messages": messages,
        }))
        .send()
        .map_err(|err| format!("Failed to contact anthropic: {err}"))?
        .error_for_status()
        .map_err(|err| format!("anthropic request failed: {err}"))?;
    let body: Value = response
        .json()
        .map_err(|err| format!("Failed to parse anthropic response: {err}"))?;
    body.get("content")
        .and_then(Value::as_array)
        .and_then(|content| {
            content.iter().find_map(|item| {
                if item.get("type").and_then(Value::as_str) == Some("text") {
                    item.get("text").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .map(|text| text.to_string())
        .ok_or_else(|| "anthropic response did not include assistant content.".to_string())
}

fn stream_anthropic<F>(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    session: &ChatSession,
    on_chunk: &mut F,
) -> Result<(), String>
where
    F: FnMut(&str) + ?Sized,
{
    let messages = session
        .messages()
        .iter()
        .filter(|message| message.role != ChatRole::System)
        .map(|message| {
            json!({
                "role": role_label(message.role),
                "content": message.content,
            })
        })
        .collect::<Vec<_>>();

    let response = client
        .post(endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("User-Agent", "tinfo")
        .json(&json!({
            "model": session.model(),
            "max_tokens": 1024,
            "system": session.system_prompt(),
            "messages": messages,
            "stream": true,
        }))
        .send()
        .map_err(|err| format!("Failed to contact anthropic: {err}"))?
        .error_for_status()
        .map_err(|err| format!("anthropic request failed: {err}"))?;

    let mut saw_chunk = false;
    let reader = BufReader::new(response);
    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim();
        if !line.starts_with("data: ") {
            continue;
        }
        let payload = line.trim_start_matches("data: ").trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }
        let Ok(body) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if let Some(content) = body
            .get("delta")
            .and_then(|value| value.get("text"))
            .and_then(Value::as_str)
            .or_else(|| {
                body.get("content_block")
                    .and_then(|value| value.get("text"))
                    .and_then(Value::as_str)
            })
        {
            if !content.is_empty() {
                saw_chunk = true;
                on_chunk(content);
            }
        }
    }

    if saw_chunk {
        Ok(())
    } else {
        let full = complete_anthropic(client, endpoint, api_key, session)?;
        if !full.is_empty() {
            on_chunk(&full);
        }
        Ok(())
    }
}

fn role_label(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

fn default_endpoint(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::OpenAi => "https://api.openai.com/v1/chat/completions",
        ProviderKind::Anthropic => "https://api.anthropic.com/v1/messages",
        ProviderKind::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
    }
}

fn openrouter_attribution_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "HTTP-Referer",
        HeaderValue::from_static("https://tinfo.1234567890.dev"),
    );
    headers.insert(
        "X-OpenRouter-Title",
        HeaderValue::from_static("Terminal Info (tinfo)"),
    );
    headers.insert(
        "X-OpenRouter-Categories",
        HeaderValue::from_static("cli-agent,programming-app"),
    );
    headers
}

struct OpenAiProvider {
    client: Client,
    endpoint: String,
    api_key: String,
    model: String,
    system_prompt: Option<String>,
}

struct AnthropicProvider {
    client: Client,
    endpoint: String,
    api_key: String,
    model: String,
    system_prompt: Option<String>,
}

struct OpenRouterProvider {
    client: Client,
    endpoint: String,
    api_key: String,
    model: String,
    system_prompt: Option<String>,
}

impl ChatProvider for OpenAiProvider {
    fn send_message(
        &self,
        messages: Vec<Message>,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<(), String> {
        let session = build_request_session(
            ProviderKind::OpenAi,
            self.model.clone(),
            self.system_prompt.clone(),
            messages,
        );
        stream_openai_like(
            &self.client,
            &self.endpoint,
            &self.api_key,
            &session,
            on_chunk,
        )
    }
}

impl ChatProvider for AnthropicProvider {
    fn send_message(
        &self,
        messages: Vec<Message>,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<(), String> {
        let session = build_request_session(
            ProviderKind::Anthropic,
            self.model.clone(),
            self.system_prompt.clone(),
            messages,
        );
        stream_anthropic(
            &self.client,
            &self.endpoint,
            &self.api_key,
            &session,
            on_chunk,
        )
    }
}

impl ChatProvider for OpenRouterProvider {
    fn send_message(
        &self,
        messages: Vec<Message>,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<(), String> {
        let session = build_request_session(
            ProviderKind::OpenRouter,
            self.model.clone(),
            self.system_prompt.clone(),
            messages,
        );
        stream_openai_like(
            &self.client,
            &self.endpoint,
            &self.api_key,
            &session,
            on_chunk,
        )
    }
}

fn build_request_session(
    provider: ProviderKind,
    model: String,
    system_prompt: Option<String>,
    messages: Vec<Message>,
) -> ChatSession {
    let now = 0;
    let mut session = ChatSession::new(
        "cli",
        provider,
        model,
        HistoryMode::InMemory,
        system_prompt,
        now,
    );
    for message in messages {
        session.push_message(role_from_message(&message.role), message.content, now);
    }
    session
}

fn role_from_message(role: &str) -> ChatRole {
    match role {
        "system" => ChatRole::System,
        "assistant" => ChatRole::Assistant,
        _ => ChatRole::User,
    }
}
