use std::io::{self, IsTerminal, Write};
use std::sync::mpsc;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use dialoguer::{Input, Password, Select, theme::ColorfulTheme};

use crate::ai::chat::{ChatRole, ChatSession, HistoryMode, Message, ProviderKind, build_provider};
use crate::ai::connections::{ConnectionConfig, get_connection};
use crate::ai::config::AiConfig;
use crate::ai::input::{build_stdin_analysis_prompt, process_chat_input, read_piped_stdin};
use crate::ai::storage::Storage;

const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_DIM: &str = "\x1b[2m";
const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RESET: &str = "\x1b[0m";

pub struct ChatOptions {
    pub mode: ChatMode,
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
    pub system: Option<String>,
    pub connection: Option<String>,
    pub input: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatMode {
    Chat,
    Ask,
    Fix,
    Summarize,
    Plan,
    Doc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Plain,
    Markdown,
}

impl ChatMode {
    fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Ask => "ask",
            Self::Fix => "fix",
            Self::Summarize => "summarize",
            Self::Plan => "plan",
            Self::Doc => "doc",
        }
    }

    fn system_instruction(self) -> Option<&'static str> {
        match self {
            Self::Chat => None,
            Self::Ask => Some(
                "You are a fast CLI assistant.\n\nAnswer the user's question directly and concisely.\n\nRules:\n- Be short and precise\n- Do not include unnecessary explanation\n- Do not use markdown unless needed\n- Prefer clear, actionable answers",
            ),
            Self::Fix => Some(
                "You are a debugging assistant in a CLI environment.\n\nYour task is to identify problems and suggest fixes.\n\nOutput format:\n1. Problem\n2. Cause\n3. Fix\n\nRules:\n- Be direct and practical\n- Focus on actionable solutions\n- Avoid vague explanations\n- If input is logs or errors, prioritize root cause\n- Keep output structured and easy to scan",
            ),
            Self::Summarize => Some(
                "You are a summarization assistant for terminal users.\n\nYour task is to condense information clearly.\n\nOutput format:\n- Use bullet points\n- Group related ideas\n- Keep it concise\n\nRules:\n- Do not include unnecessary detail\n- Preserve key meaning\n- Prefer structured output over paragraphs",
            ),
            Self::Plan => Some(
                "You are a planning assistant.\n\nYour task is to break down tasks into clear, actionable steps.\n\nOutput format:\n- Use numbered steps\n- Each step should be specific and executable\n\nRules:\n- Avoid vague advice\n- Prefer concrete actions\n- Keep steps logically ordered\n- Use markdown structure (headings and lists)",
            ),
            Self::Doc => Some(
                "You are a documentation generator.\n\nYour task is to produce clean, structured documentation.\n\nOutput format (Markdown):\n- Title\n- Description\n- Usage\n- Examples (if applicable)\n\nRules:\n- Use clear headings\n- Keep explanations concise\n- Avoid marketing language\n- Focus on clarity and usability",
            ),
        }
    }

    fn is_stateless(self) -> bool {
        self != Self::Chat
    }

    fn supports_output_format_prompt(self) -> bool {
        matches!(self, Self::Plan | Self::Doc)
    }
}

pub fn run(options: ChatOptions) -> Result<(), String> {
    let mut config = AiConfig::load_default();
    let provider = resolve_provider(&config, options.provider)?;
    config = ensure_api_key(config, provider)?;
    let model = resolve_model(&config, provider, options.model)?;
    let connection = resolve_connection(options.connection.as_deref())?;
    let output_format = resolve_output_format(options.mode)?;
    let system_prompt = build_system_prompt(
        options.system.or_else(|| config.system_prompt().map(str::to_string)),
        connection.as_ref(),
        options.mode,
        output_format,
    );

    if let Some(stdin_input) = read_piped_stdin()? {
        return run_stdin_analysis(
            &config,
            options.mode,
            output_format,
            provider,
            model,
            system_prompt,
            connection.as_ref(),
            &stdin_input,
        );
    }

    if options.mode.is_stateless() {
        return run_single_shot_mode(
            &config,
            options.mode,
            output_format,
            provider,
            model,
            system_prompt,
            connection.as_ref(),
            options.input.as_deref(),
        );
    }

    let history_enabled = config.chat_history_enabled();
    let context_enabled = config.chat_context_enabled();
    let storage = if history_enabled {
        Some(Storage::new(config.data_dir(), true)?)
    } else {
        None
    };
    let session = load_initial_session(
        storage.as_ref(),
        provider,
        model.clone(),
        system_prompt.clone(),
        history_enabled,
    )?;

    println!("Type '/exit' or '/quit' to leave.");
    println!(
        "Tip: /provider switch provider · /model switch model · /new new chat · /chats open saved chats · /clear clear screen · /copy copy last response · /retry retry last prompt\n"
    );
    let mut state = ChatState {
        config,
        provider,
        model,
        system: system_prompt,
        history_enabled,
        context_enabled,
        connection: connection.clone(),
        storage,
        session,
    };
    sync_state_from_session(&mut state);

    loop {
        let input = read_user_input(
            state.provider,
            &state.model,
            state.connection.as_ref().map(|(name, _)| name.as_str()),
        )?;
        let trimmed = input.trim();
        if trimmed.eq_ignore_ascii_case("/exit")
            || trimmed.eq_ignore_ascii_case("/quit")
        {
            break;
        }
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "/clear" {
            clear_screen()?;
            continue;
        }
        if trimmed == "/copy" {
            copy_last_response(&state)?;
            continue;
        }
        if trimmed == "/retry" {
            retry_last_prompt(&mut state)?;
            continue;
        }
        if trimmed == "/provider" {
            switch_provider(&mut state)?;
            continue;
        }
        if trimmed == "/model" {
            switch_model(&mut state)?;
            continue;
        }
        if trimmed == "/new" {
            create_new_session(&mut state)?;
            continue;
        }
        if trimmed == "/chats" {
            switch_chat_session(&mut state)?;
            continue;
        }

        let processed = match process_chat_input(
            trimmed,
            state.connection.as_ref().map(|(name, _)| name.as_str()),
            state.connection.as_ref().map(|(_, connection)| connection),
        ) {
            Ok(value) => value,
            Err(err) => {
                println!("{err}");
                continue;
            }
        };
        for message in &processed.display_messages {
            println!("{message}");
        }

        let now = now_unix();
        state
            .session
            .push_message(ChatRole::User, processed.prompt, now);
        persist_session(&state)?;
        execute_response(&mut state)?;
    }

    Ok(())
}

struct ChatState {
    config: AiConfig,
    provider: ProviderKind,
    model: String,
    system: Option<String>,
    history_enabled: bool,
    context_enabled: bool,
    connection: Option<(String, ConnectionConfig)>,
    storage: Option<Storage>,
    session: ChatSession,
}

fn resolve_connection(name: Option<&str>) -> Result<Option<(String, ConnectionConfig)>, String> {
    let Some(name) = name else {
        return Ok(None);
    };
    let connection = get_connection(name)?
        .ok_or_else(|| format!("Connection '{}' was not found in ~/.tinfo/connections.toml.", name))?;
    Ok(Some((name.to_string(), connection)))
}

fn retry_last_prompt(state: &mut ChatState) -> Result<(), String> {
    let last_user = state
        .session
        .messages()
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .map(|message| message.content.clone());

    let Some(last_user) = last_user else {
        println!("No previous prompt to retry.");
        return Ok(());
    };

    let latest_is_user = state
        .session
        .messages()
        .last()
        .map(|message| message.role == ChatRole::User)
        .unwrap_or(false);
    if !latest_is_user {
        state
            .session
            .push_message(ChatRole::User, last_user, now_unix());
        persist_session(state)?;
    }

    execute_response(state)
}

fn execute_response(state: &mut ChatState) -> Result<(), String> {
    let request_messages = request_messages_for_session(&state.session, state.context_enabled);

    println!();
    let mut reply = String::new();
    let mut renderer = MarkdownStreamRenderer::default();
    let provider_client = match build_provider(
        &state.config,
        state.provider,
        state.model.clone(),
        state.system.clone(),
    ) {
        Ok(provider) => provider,
        Err(err) => {
            print_chat_error(&err);
            return Ok(());
        }
    };
    let outcome = match stream_response(
        provider_client,
        request_messages,
        true,
        &mut renderer,
        Some(&mut reply),
    ) {
        Ok(outcome) => outcome,
        Err(err) => {
            renderer.finish();
            println!();
            print_chat_error(&err);
            return Ok(());
        }
    };
    renderer.finish();
    println!();
    println!();

    if outcome == StreamOutcome::Completed {
        state.session.push_message(ChatRole::Assistant, reply, now_unix());
        persist_session(state)?;
    }
    Ok(())
}

fn print_chat_error(err: &str) {
    println!("Request failed: {err}");
    if should_suggest_retry(err) {
        println!("Tip: use /retry after connection or provider issues.");
    }
}

fn should_suggest_retry(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("failed to contact")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection")
        || lower.contains("dns")
        || lower.contains("network")
        || lower.contains("tempor")
        || lower.contains("unavailable")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
}

fn build_system_prompt(
    base: Option<String>,
    connection: Option<&(String, ConnectionConfig)>,
    mode: ChatMode,
    _format: OutputFormat,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(base) = base.filter(|value| !value.trim().is_empty()) {
        parts.push(base);
    }
    if let Some(instruction) = mode.system_instruction() {
        parts.push(instruction.to_string());
    }
    if let Some((name, connection)) = connection {
        let mut block = format!(
            "Connection context is attached for awareness only.\nConnection name: {name}\nURL: {}",
            connection.url
        );
        if let Some(description) = connection.description.as_deref() {
            block.push_str(&format!("\nDescription: {description}"));
        }
        if !connection.metadata.is_empty() {
            block.push_str("\nMetadata:");
            for (key, value) in &connection.metadata {
                block.push_str(&format!("\n- {key}: {value}"));
            }
        }
        block.push_str("\nDo not claim to execute tools against this connection.");
        parts.push(block);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn resolve_output_format(mode: ChatMode) -> Result<OutputFormat, String> {
    if !mode.supports_output_format_prompt() {
        return Ok(OutputFormat::Plain);
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(OutputFormat::Markdown);
    }

    let answer = Input::<String>::new()
        .with_prompt("Output as Markdown? (Y/n)")
        .allow_empty(true)
        .interact_text()
        .map_err(|err| format!("Failed to read output format: {err}"))?;
    let normalized = answer.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized == "y" || normalized == "yes" {
        Ok(OutputFormat::Markdown)
    } else if normalized == "n" || normalized == "no" {
        Ok(OutputFormat::Plain)
    } else {
        Err("Invalid output format selection. Use Y, Enter, n, or no.".to_string())
    }
}

fn run_single_shot_mode(
    config: &AiConfig,
    mode: ChatMode,
    output_format: OutputFormat,
    provider: ProviderKind,
    model: String,
    system_prompt: Option<String>,
    connection: Option<&(String, ConnectionConfig)>,
    input: Option<&str>,
) -> Result<(), String> {
    let input = if let Some(input) = input.filter(|value| !value.trim().is_empty()) {
        input.to_string()
    } else if io::stdin().is_terminal() && io::stdout().is_terminal() {
        read_user_input(
            provider,
            &model,
            connection.map(|(name, _)| name.as_str()),
        )?
    } else {
        return Err(format!("Mode '{}' needs text input or piped stdin.", mode.label()));
    };

    println!("Mode: {}", mode.label());
    let processed = process_chat_input(
        input.trim(),
        connection.map(|(name, _)| name.as_str()),
        connection.map(|(_, connection)| connection),
    )?;
    for message in &processed.display_messages {
        println!("{message}");
    }
    println!();

    let provider_client = build_provider(config, provider, model, system_prompt)?;
    let mut renderer = MarkdownStreamRenderer::default();
    let mut reply = String::new();
    let outcome = stream_response(
        provider_client,
        vec![Message {
            role: "user".to_string(),
            content: processed.prompt,
        }],
        true,
        &mut renderer,
        Some(&mut reply),
    )?;
    renderer.finish();
    println!();
    if outcome == StreamOutcome::Completed {
        maybe_save_output(mode, output_format, &reply)?;
    }
    Ok(())
}

fn run_stdin_analysis(
    config: &AiConfig,
    mode: ChatMode,
    output_format: OutputFormat,
    provider: ProviderKind,
    model: String,
    system_prompt: Option<String>,
    connection: Option<&(String, ConnectionConfig)>,
    stdin_input: &str,
) -> Result<(), String> {
    println!("Mode: {}", mode.label());
    let processed = build_stdin_analysis_prompt(stdin_input, connection.map(|(_, config)| config));
    for message in &processed.display_messages {
        println!("{message}");
    }
    println!();
    let provider_client = build_provider(config, provider, model, system_prompt)?;
    let mut renderer = MarkdownStreamRenderer::default();
    let mut reply = String::new();
    let outcome = stream_response(
        provider_client,
        vec![Message {
            role: "user".to_string(),
            content: processed.prompt,
        }],
        false,
        &mut renderer,
        Some(&mut reply),
    )?;
    renderer.finish();
    println!();
    if outcome == StreamOutcome::Completed {
        maybe_save_output(mode, output_format, &reply)?;
    }
    Ok(())
}

fn maybe_save_output(mode: ChatMode, format: OutputFormat, content: &str) -> Result<(), String> {
    if !mode.supports_output_format_prompt() || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(());
    }

    let answer = Input::<String>::new()
        .with_prompt("Save output to file? (y/N)")
        .allow_empty(true)
        .interact_text()
        .map_err(|err| format!("Failed to read save choice: {err}"))?;
    let normalized = answer.trim().to_ascii_lowercase();
    if normalized != "y" && normalized != "yes" {
        return Ok(());
    }

    let default_name = match (mode, format) {
        (ChatMode::Plan, OutputFormat::Markdown) => "./output.md",
        (ChatMode::Plan, OutputFormat::Plain) => "./output.txt",
        (ChatMode::Doc, OutputFormat::Markdown) => "./output.md",
        (ChatMode::Doc, OutputFormat::Plain) => "./output.txt",
        _ => "./output.txt",
    };
    let path = Input::<String>::new()
        .with_prompt(format!("Enter file path (default: {default_name})"))
        .allow_empty(true)
        .interact_text()
        .map_err(|err| format!("Failed to read output path: {err}"))?;
    let final_path = normalize_output_path(path.trim(), default_name, format);

    if final_path.exists() {
        let overwrite = Input::<String>::new()
            .with_prompt("File exists. Overwrite? (y/N)")
            .allow_empty(true)
            .interact_text()
            .map_err(|err| format!("Failed to read overwrite choice: {err}"))?;
        let normalized = overwrite.trim().to_ascii_lowercase();
        if normalized != "y" && normalized != "yes" {
            return Ok(());
        }
    }

    if let Some(parent) = final_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create output directory: {err}"))?;
    }
    std::fs::write(&final_path, content)
        .map_err(|err| format!("Failed to save output: {err}"))?;
    println!("Saved to: {}", final_path.display());
    Ok(())
}

fn normalize_output_path(path: &str, default_name: &str, format: OutputFormat) -> std::path::PathBuf {
    let raw = if path.is_empty() { default_name } else { path };
    let mut path = std::path::PathBuf::from(raw);
    if path.extension().is_none() {
        path.set_extension(match format {
            OutputFormat::Markdown => "md",
            OutputFormat::Plain => "txt",
        });
    }
    path
}

fn clear_screen() -> Result<(), String> {
    print!("\x1b[2J\x1b[H");
    io::stdout()
        .flush()
        .map_err(|err| format!("Failed to clear chat screen: {err}"))
}

fn copy_last_response(state: &ChatState) -> Result<(), String> {
    let Some(text) = state.session.latest_assistant_message() else {
        println!("No assistant response available to copy.");
        return Ok(());
    };
    copy_to_clipboard(text)?;
    println!("Copied last response.");
    Ok(())
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return pipe_to_command("pbcopy", &[], text);
    }

    #[cfg(target_os = "windows")]
    {
        return pipe_to_command("clip", &[], text);
    }

    #[cfg(target_os = "linux")]
    {
        pipe_to_command("wl-copy", &[], text)
            .or_else(|_| pipe_to_command("xclip", &["-selection", "clipboard"], text))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = text;
        Err("Clipboard copy is not supported on this platform.".to_string())
    }
}

fn pipe_to_command(program: &str, args: &[&str], text: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to launch {program}: {err}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|err| format!("Failed to send text to {program}: {err}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("Failed to wait for {program}: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8(output.stderr).unwrap_or_default();
        Err(if stderr.trim().is_empty() {
            format!("{program} exited unsuccessfully.")
        } else {
            format!("{program} failed: {}", stderr.trim())
        })
    }
}

fn ensure_api_key(config: AiConfig, provider: ProviderKind) -> Result<AiConfig, String> {
    if config.provider_has_api_key(provider) {
        return Ok(config);
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(format!(
            "Missing API key for {}. Run `tinfo chat --provider {}` in an interactive terminal to add it.",
            provider.label(),
            provider.label()
        ));
    }

    let api_key = Password::new()
        .with_prompt(format!(
            "Enter your {} API key (input hidden)",
            provider.display_name()
        ))
        .allow_empty_password(false)
        .interact()
        .map_err(|err| format!("Failed to read API key: {err}"))?;
    AiConfig::save_provider_api_key(provider, api_key)
}

fn resolve_provider(
    config: &AiConfig,
    cli_provider: Option<ProviderKind>,
) -> Result<ProviderKind, String> {
    if let Some(provider) = cli_provider {
        let _ = AiConfig::save_default_provider(provider)?;
        return Ok(provider);
    }

    if let Some(provider) = config.configured_default_provider() {
        return Ok(provider);
    }

    select_provider()
}

fn select_provider() -> Result<ProviderKind, String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "No chat provider configured. Run `tinfo chat` in an interactive terminal to select one."
                .to_string(),
        );
    }

    println!("Select a provider:");
    println!("1. OpenRouter (Recommended · multi-model support)");
    println!("2. OpenAI");
    println!("3. Claude");
    println!();
    let choices = [
        ProviderKind::OpenRouter,
        ProviderKind::OpenAi,
        ProviderKind::Anthropic,
    ];
    let selection = Input::<String>::new()
        .with_prompt("Provider")
        .allow_empty(false)
        .interact_text()
        .map_err(|err| format!("Failed to read provider selection: {err}"))?;
    let normalized = selection.trim().to_ascii_lowercase();
    let provider = match normalized.as_str() {
        "1" | "openrouter" => choices[0],
        "2" | "openai" => choices[1],
        "3" | "claude" | "anthropic" => choices[2],
        _ => return Err("Invalid provider selection. Use 1, 2, 3, openrouter, openai, or claude.".to_string()),
    };
    let _ = AiConfig::save_default_provider(provider)?;
    Ok(provider)
}

fn resolve_model(
    config: &AiConfig,
    provider: ProviderKind,
    cli_model: Option<String>,
) -> Result<String, String> {
    if let Some(model) = cli_model {
        validate_model_input(provider, &model)?;
        let _ = AiConfig::save_default_model(provider, model.clone())?;
        return Ok(model);
    }

    let configured = config.default_model(provider).to_string();
    if configured != "default" {
        return Ok(configured);
    }

    select_model(provider)
}

fn switch_provider(state: &mut ChatState) -> Result<(), String> {
    let provider = select_provider()?;
    state.config = ensure_api_key(state.config.clone(), provider)?;
    state.model = resolve_model(&state.config, provider, None)?;
    state.provider = provider;
    state.session.set_provider(provider);
    state.session.set_model(state.model.clone());
    persist_session(state)?;
    println!("Using provider: {}", state.provider.display_name());
    Ok(())
}

fn switch_model(state: &mut ChatState) -> Result<(), String> {
    state.model = select_model(state.provider)?;
    state.session.set_model(state.model.clone());
    persist_session(state)?;
    println!("Using model: {}", state.model);
    Ok(())
}

fn select_model(provider: ProviderKind) -> Result<String, String> {
    let model = if provider == ProviderKind::OpenRouter {
        let items = openrouter_model_items();
        let mut labels = items.iter().map(|(label, _)| *label).collect::<Vec<_>>();
        labels.push("Custom model...");
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a model")
            .items(&labels)
            .default(0)
            .interact()
            .map_err(|err| format!("Failed to read model selection: {err}"))?;
        if selection + 1 == labels.len() {
            prompt_custom_openrouter_model()?
        } else {
            items
                .get(selection)
                .ok_or_else(|| "Invalid model selection.".to_string())?
                .1
                .to_string()
        }
    } else {
        let models = models_for(provider);
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a model")
            .items(models)
            .default(0)
            .interact()
            .map_err(|err| format!("Failed to read model selection: {err}"))?;
        models
            .get(selection)
            .ok_or_else(|| "Invalid model selection.".to_string())?
            .to_string()
    };
    validate_model_input(provider, &model)?;
    let _ = AiConfig::save_default_model(provider, model.clone())?;
    Ok(model)
}

fn models_for(provider: ProviderKind) -> &'static [&'static str] {
    match provider {
        ProviderKind::OpenAi => &[
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.4-nano",
            "gpt-5.1",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5-pro",
            "gpt-5",
            "gpt-4.1",
            "o3-deep-research",
        ],
        ProviderKind::Anthropic => &[
            "claude-opus-4-6",
            "claude-sonnet-4-6",
            "claude-haiku-4-5",
        ],
        ProviderKind::OpenRouter => &[
            "z-ai/glm-5v-turbo",
            "stepfun/step-3.5-flash:free",
            "qwen/qwen3.6-plus-preview",
            "nvidia/nemotron-3-super:free",
            "anthropic/claude-4.6-sonnet",
            "anthropic/claude-4.6-opus",
            "openai/gpt-5.4-pro",
            "openai/gpt-5.3-codex",
            "google/gemini-3.1-pro-preview",
            "google/gemini-3.1-flash",
            "deepseek/deepseek-v3.2",
            "deepseek/deepseek-r1",
            "xiaomi/mimo-v2-pro",
            "minimax/minimax-m2.7",
            "x-ai/grok-4.20-multi-agent",
            "x-ai/grok-4.20",
            "meta/llama-4-400b-instruct",
            "mistralai/mistral-large-2603",
            "mistralai/devstral-2-123b",
            "z-ai/glm-5",
            "z-ai/glm-4.5-air",
            "openai/gpt-5.4-nano",
            "openai/gpt-5.4",
            "openai/gpt-oss-120b",
            "moonshotai/kimi-k2.5",
            "liquid/lfm-2.5-thinking",
            "google/gemma-4-31b-dense",
        ],
    }
}

fn openrouter_model_items() -> &'static [(&'static str, &'static str)] {
    &[
        ("z-ai/glm-5v-turbo", "z-ai/glm-5v-turbo"),
        ("stepfun/step-3.5-flash:free", "stepfun/step-3.5-flash:free"),
        ("qwen/qwen3.6-plus-preview", "qwen/qwen3.6-plus-preview"),
        ("nvidia/nemotron-3-super:free", "nvidia/nemotron-3-super:free"),
        ("anthropic/claude-4.6-sonnet", "anthropic/claude-4.6-sonnet"),
        ("anthropic/claude-4.6-opus", "anthropic/claude-4.6-opus"),
        ("openai/gpt-5.4-pro", "openai/gpt-5.4-pro"),
        ("openai/gpt-5.3-codex", "openai/gpt-5.3-codex"),
        ("google/gemini-3.1-pro-preview", "google/gemini-3.1-pro-preview"),
        ("google/gemini-3.1-flash", "google/gemini-3.1-flash"),
        ("deepseek/deepseek-v3.2", "deepseek/deepseek-v3.2"),
        ("deepseek/deepseek-r1 (Reasoning)", "deepseek/deepseek-r1"),
        ("xiaomi/mimo-v2-pro", "xiaomi/mimo-v2-pro"),
        ("minimax/minimax-m2.7", "minimax/minimax-m2.7"),
        ("x-ai/grok-4.20-multi-agent", "x-ai/grok-4.20-multi-agent"),
        ("x-ai/grok-4.20", "x-ai/grok-4.20"),
        ("meta/llama-4-400b-instruct", "meta/llama-4-400b-instruct"),
        ("mistralai/mistral-large-2603", "mistralai/mistral-large-2603"),
        ("mistralai/devstral-2-123b", "mistralai/devstral-2-123b"),
        ("z-ai/glm-5", "z-ai/glm-5"),
        ("z-ai/glm-4.5-air", "z-ai/glm-4.5-air"),
        ("openai/gpt-5.4-nano", "openai/gpt-5.4-nano"),
        ("openai/gpt-5.4", "openai/gpt-5.4"),
        ("openai/gpt-oss-120b", "openai/gpt-oss-120b"),
        ("moonshotai/kimi-k2.5", "moonshotai/kimi-k2.5"),
        ("liquid/lfm-2.5-thinking", "liquid/lfm-2.5-thinking"),
        ("google/gemma-4-31b-dense", "google/gemma-4-31b-dense"),
    ]
}

fn prompt_custom_openrouter_model() -> Result<String, String> {
    let model = Input::<String>::new()
        .with_prompt("Enter a custom OpenRouter model (provider/model)")
        .allow_empty(false)
        .interact_text()
        .map_err(|err| format!("Failed to read custom model: {err}"))?;
    validate_openrouter_model(&model)?;
    Ok(model)
}

fn validate_model_input(provider: ProviderKind, model: &str) -> Result<(), String> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err("Model cannot be empty.".to_string());
    }
    if provider == ProviderKind::OpenRouter {
        validate_openrouter_model(trimmed)?;
    }
    Ok(())
}

fn validate_openrouter_model(model: &str) -> Result<(), String> {
    let trimmed = model.trim();
    let mut parts = trimmed.split('/');
    let namespace = parts.next().unwrap_or_default();
    let model_name = parts.next().unwrap_or_default();
    if namespace.is_empty() || model_name.is_empty() || parts.next().is_some() {
        return Err(
            "Invalid OpenRouter model format. Use provider/model, for example openai/gpt-5.4-pro."
                .to_string(),
        );
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_' | ':'))
    {
        return Err(
            "Invalid OpenRouter model format. Only letters, numbers, '/', '.', '-', '_', and ':' are allowed."
                .to_string(),
        );
    }
    Ok(())
}

fn read_user_input(
    provider: ProviderKind,
    model: &str,
    connection_name: Option<&str>,
) -> Result<String, String> {
    if let Some(connection_name) = connection_name {
        print!("[{} · {} · {}] > ", provider.display_name(), model, connection_name);
    } else {
        print!("[{} · {}] > ", provider.display_name(), model);
    }
    io::stdout()
        .flush()
        .map_err(|err| format!("Failed to flush prompt: {err}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|err| format!("Failed to read chat input: {err}"))?;
    Ok(strip_terminal_escape_sequences(&input))
}

fn strip_terminal_escape_sequences(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars = input.chars().collect::<Vec<_>>();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '\x1b' {
            i += 1;
            if i < chars.len() && chars[i] == '[' {
                i += 1;
                while i < chars.len() {
                    let ch = chars[i];
                    i += 1;
                    if ('@'..='~').contains(&ch) {
                        break;
                    }
                }
                continue;
            }
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn load_initial_session(
    storage: Option<&Storage>,
    provider: ProviderKind,
    model: String,
    system: Option<String>,
    history_enabled: bool,
) -> Result<ChatSession, String> {
    if let Some(storage) = storage {
        let mut sessions = storage.load_state()?.chat_sessions;
        sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at()));
        if let Some(session) = sessions.into_iter().next() {
            return Ok(session);
        }
    }

    Ok(new_chat_session(provider, model, system, history_enabled))
}

fn create_new_session(state: &mut ChatState) -> Result<(), String> {
    state.session = new_chat_session(
        state.provider,
        state.model.clone(),
        state.system.clone(),
        state.history_enabled,
    );
    persist_session(state)?;
    println!("Started a new chat.");
    Ok(())
}

fn switch_chat_session(state: &mut ChatState) -> Result<(), String> {
    if !state.history_enabled {
        println!("Chat history is disabled in config.");
        return Ok(());
    }

    let Some(storage) = state.storage.as_ref() else {
        println!("Chat history is disabled in config.");
        return Ok(());
    };

    let mut sessions = storage.load_state()?.chat_sessions;
    if sessions.is_empty() {
        println!("No saved chats yet.");
        return Ok(());
    }
    sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at()));
    let labels = sessions.iter().map(chat_session_label).collect::<Vec<_>>();
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a chat")
        .items(&labels)
        .default(0)
        .interact()
        .map_err(|err| format!("Failed to read chat selection: {err}"))?;

    state.session = sessions
        .into_iter()
        .nth(selection)
        .ok_or_else(|| "Invalid chat selection.".to_string())?;
    sync_state_from_session(state);
    println!("Switched to {}", chat_session_label(&state.session));
    Ok(())
}

fn sync_state_from_session(state: &mut ChatState) {
    state.provider = state.session.provider();
    state.model = state.session.model().to_string();
    state.system = state.session.system_prompt().map(str::to_string);
}

fn persist_session(state: &ChatState) -> Result<(), String> {
    if let Some(storage) = state.storage.as_ref() {
        storage.upsert_chat_session(&state.session)?;
    }
    Ok(())
}

fn request_messages_for_session(session: &ChatSession, include_context: bool) -> Vec<Message> {
    if include_context {
        session
            .messages()
            .iter()
            .map(|message| Message {
                role: message_role(message.role).to_string(),
                content: message.content.clone(),
            })
            .collect()
    } else {
        session
            .messages()
            .iter()
            .rev()
            .take(1)
            .map(|message| Message {
                role: message_role(message.role).to_string(),
                content: message.content.clone(),
            })
            .collect()
    }
}

fn message_role(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

fn new_chat_session(
    provider: ProviderKind,
    model: String,
    system: Option<String>,
    history_enabled: bool,
) -> ChatSession {
    ChatSession::new(
        format!("chat-{}", now_unix_millis()),
        provider,
        model,
        if history_enabled {
            HistoryMode::Persisted
        } else {
            HistoryMode::InMemory
        },
        system,
        now_unix(),
    )
}

fn chat_session_label(session: &ChatSession) -> String {
    let first_user = session
        .messages()
        .iter()
        .find(|message| message.role == ChatRole::User)
        .map(|message| summarize_text(&message.content))
        .unwrap_or_else(|| "New chat".to_string());
    format!(
        "{} · {} · {} messages",
        first_user,
        session.provider().display_name(),
        session.messages().len()
    )
}

fn summarize_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "New chat".to_string();
    }
    let mut summary = trimmed.chars().take(48).collect::<String>();
    if trimmed.chars().count() > 48 {
        summary.push_str("...");
    }
    summary
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn now_unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum StreamOutcome {
    Completed,
    Stopped,
}

enum ResponseEvent {
    Chunk(String),
    Done(Result<(), String>),
}

fn stream_response(
    provider_client: Box<dyn crate::ai::chat::ChatProvider>,
    request_messages: Vec<Message>,
    interactive_controls: bool,
    renderer: &mut MarkdownStreamRenderer,
    mut reply: Option<&mut String>,
) -> Result<StreamOutcome, String> {
    let (tx, rx) = mpsc::channel::<ResponseEvent>();
    thread::spawn(move || {
        let mut on_chunk = |chunk: &str| {
            let _ = tx.send(ResponseEvent::Chunk(chunk.to_string()));
        };
        let result = provider_client.send_message(request_messages, &mut on_chunk);
        let _ = tx.send(ResponseEvent::Done(result));
    });

    let controls_enabled = interactive_controls && io::stdin().is_terminal() && io::stdout().is_terminal();
    let mut raw_mode_enabled = false;
    if controls_enabled {
        enable_raw_mode().map_err(|err| format!("Failed to enable chat controls: {err}"))?;
        raw_mode_enabled = true;
        println!("{ANSI_DIM}Press Space to pause/resume · q to stop the current response{ANSI_RESET}");
    }

    print!("AI: ");
    io::stdout()
        .flush()
        .map_err(|err| format!("Failed to flush chat output: {err}"))?;

    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut spinner_index = 0usize;
    let mut saw_output = false;
    let mut paused = false;
    let mut paused_chunks: Vec<String> = Vec::new();

    loop {
        if raw_mode_enabled
            && event::poll(Duration::from_millis(10))
                .map_err(|err| format!("Failed to poll chat controls: {err}"))?
        {
            if let Event::Key(key) =
                event::read().map_err(|err| format!("Failed to read chat controls: {err}"))?
            {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            if raw_mode_enabled {
                                disable_raw_mode()
                                    .map_err(|err| format!("Failed to disable chat controls: {err}"))?;
                            }
                            println!("\rAI: [stopped]");
                            return Ok(StreamOutcome::Stopped);
                        }
                        KeyCode::Char(' ') => {
                            paused = !paused;
                            if paused {
                                print!("\rAI: [paused]");
                            } else {
                                print!("\rAI: ");
                                for chunk in paused_chunks.drain(..) {
                                    renderer.push(&chunk);
                                }
                            }
                            io::stdout()
                                .flush()
                                .map_err(|err| format!("Failed to update chat output: {err}"))?;
                        }
                        _ => {}
                    }
                }
            }
        }

        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(ResponseEvent::Chunk(chunk)) => {
                if let Some(reply) = reply.as_deref_mut() {
                    reply.push_str(&chunk);
                }
                let cleaned = clean_stream_chunk(&chunk);
                if cleaned.is_empty() {
                    continue;
                }
                if !saw_output {
                    print!("\rAI:\n");
                    saw_output = true;
                }
                if paused {
                    paused_chunks.push(cleaned);
                } else {
                    renderer.push(&cleaned);
                    io::stdout()
                        .flush()
                        .map_err(|err| format!("Failed to flush chat output: {err}"))?;
                }
            }
            Ok(ResponseEvent::Done(result)) => {
                if raw_mode_enabled {
                    disable_raw_mode()
                        .map_err(|err| format!("Failed to disable chat controls: {err}"))?;
                }
                if !paused_chunks.is_empty() {
                    if !saw_output {
                        print!("\rAI:\n");
                    }
                    for chunk in paused_chunks.drain(..) {
                        renderer.push(&chunk);
                    }
                } else if !saw_output {
                    print!("\rAI:");
                }
                io::stdout()
                    .flush()
                    .map_err(|err| format!("Failed to flush chat output: {err}"))?;
                result?;
                return Ok(StreamOutcome::Completed);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !saw_output && !paused {
                    print!("\rAI: {}", frames[spinner_index % frames.len()]);
                    io::stdout()
                        .flush()
                        .map_err(|err| format!("Failed to flush chat output: {err}"))?;
                    spinner_index += 1;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if raw_mode_enabled {
                    disable_raw_mode()
                        .map_err(|err| format!("Failed to disable chat controls: {err}"))?;
                }
                return Err("Chat response stream disconnected unexpectedly.".to_string());
            }
        }
    }
}

fn clean_stream_chunk(chunk: &str) -> String {
    let stripped = strip_terminal_escape_sequences(chunk);
    let mut cleaned = String::with_capacity(stripped.len());

    for line in stripped.split_inclusive('\n') {
        let (content, newline) = match line.strip_suffix('\n') {
            Some(content) => (content, "\n"),
            None => (line, ""),
        };
        let content = content.strip_prefix("AI: ").unwrap_or(content);
        let content = content.strip_prefix("AI:").unwrap_or(content);
        let filtered = content
            .chars()
            .filter(|ch| *ch == '\n' || *ch == '\t' || !ch.is_control())
            .collect::<String>();
        cleaned.push_str(&filtered);
        cleaned.push_str(newline);
    }

    cleaned
}

#[derive(Default)]
struct MarkdownStreamRenderer {
    buffer: String,
    in_code_block: bool,
    table_lines: Vec<String>,
}

impl MarkdownStreamRenderer {
    fn push(&mut self, chunk: &str) {
        for ch in chunk.chars() {
            match ch {
                '\r' => {
                    if !self.buffer.is_empty() {
                        self.flush_line(false);
                    }
                }
                '\n' => self.flush_line(true),
                _ => self.buffer.push(ch),
            }
        }
    }

    fn finish(&mut self) {
        self.flush_table_block();
        if !self.buffer.is_empty() {
            self.flush_line(false);
        }
    }

    fn flush_line(&mut self, with_newline: bool) {
        let line = std::mem::take(&mut self.buffer);
        let trimmed = line.trim();

        if !self.in_code_block && (is_table_row(trimmed) || is_table_separator(trimmed)) {
            self.table_lines.push(line);
            return;
        }

        self.flush_table_block();
        let rendered = render_markdown_line(&line, &mut self.in_code_block);
        if with_newline {
            println!("{rendered}");
        } else {
            print!("{rendered}");
        }
    }

    fn flush_table_block(&mut self) {
        if self.table_lines.is_empty() {
            return;
        }
        for line in render_table_block(&self.table_lines) {
            println!("{line}");
        }
        self.table_lines.clear();
    }
}

fn render_markdown_line(line: &str, in_code_block: &mut bool) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        *in_code_block = !*in_code_block;
        return format!("{ANSI_DIM}```{ANSI_RESET}");
    }

    if *in_code_block {
        return format!("{ANSI_DIM}    {line}{ANSI_RESET}");
    }

    if is_horizontal_rule(trimmed) {
        return format!("{ANSI_DIM}────────────────────{ANSI_RESET}");
    }

    if is_table_separator(trimmed) {
        return format!("{ANSI_DIM}{trimmed}{ANSI_RESET}");
    }

    if is_table_row(trimmed) {
        return render_table_row(trimmed);
    }

    if let Some(content) = trimmed.strip_prefix("### ") {
        return format!("{ANSI_BOLD}{content}{ANSI_RESET}");
    }
    if let Some(content) = trimmed.strip_prefix("## ") {
        return format!("{ANSI_BOLD}{ANSI_CYAN}{content}{ANSI_RESET}");
    }
    if let Some(content) = trimmed.strip_prefix("# ") {
        return format!("{ANSI_BOLD}{ANSI_CYAN}{content}{ANSI_RESET}");
    }
    if let Some(content) = trimmed.strip_prefix("> ") {
        return format!("{ANSI_DIM}> {}{ANSI_RESET}", render_inline_markdown(content));
    }
    if let Some(content) = trimmed.strip_prefix("- ") {
        if let Some(task) = content.strip_prefix("[ ]") {
            return format!("☐ {}", render_inline_markdown(task.trim_start()));
        }
        if let Some(task) = content.strip_prefix("[x]") {
            return format!("☑ {}", render_inline_markdown(task.trim_start()));
        }
        if let Some(task) = content.strip_prefix("[X]") {
            return format!("☑ {}", render_inline_markdown(task.trim_start()));
        }
        return format!("• {}", render_inline_markdown(content));
    }
    if let Some(content) = trimmed.strip_prefix("* ") {
        if let Some(task) = content.strip_prefix("[ ]") {
            return format!("☐ {}", render_inline_markdown(task.trim_start()));
        }
        if let Some(task) = content.strip_prefix("[x]") {
            return format!("☑ {}", render_inline_markdown(task.trim_start()));
        }
        if let Some(task) = content.strip_prefix("[X]") {
            return format!("☑ {}", render_inline_markdown(task.trim_start()));
        }
        return format!("• {}", render_inline_markdown(content));
    }
    if let Some((marker, content)) = split_numbered_marker(trimmed) {
        return format!("{marker} {}", render_inline_markdown(content));
    }

    render_inline_markdown(line)
}

fn split_numbered_marker(value: &str) -> Option<(&str, &str)> {
    let mut chars = value.char_indices();
    let mut end = 0;
    for (idx, ch) in &mut chars {
        if ch.is_ascii_digit() {
            end = idx + ch.len_utf8();
            continue;
        }
        if ch == '.' && end > 0 {
            let rest = &value[idx + ch.len_utf8()..];
            if let Some(content) = rest.strip_prefix(' ') {
                return Some((&value[..=idx], content));
            }
        }
        break;
    }
    None
}

fn is_horizontal_rule(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let chars = trimmed.chars().collect::<Vec<_>>();
    let first = chars[0];
    if first != '-' && first != '*' && first != '_' {
        return false;
    }
    chars.iter().all(|ch| *ch == first)
}

fn is_table_separator(value: &str) -> bool {
    value.contains('|')
        && value
            .chars()
            .all(|ch| ch == '|' || ch == '-' || ch == ':' || ch.is_whitespace())
}

fn is_table_row(value: &str) -> bool {
    value.contains('|') && !is_table_separator(value)
}

fn render_table_row(value: &str) -> String {
    let cells = value
        .trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| render_inline_markdown(cell.trim()))
        .collect::<Vec<_>>();
    format!("| {} |", cells.join(" | "))
}

fn render_table_block(lines: &[String]) -> Vec<String> {
    let mut rows = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if is_table_separator(trimmed) {
            continue;
        }
        if !is_table_row(trimmed) {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>();
        rows.push(cells);
    }

    if rows.is_empty() {
        return lines
            .iter()
            .map(|line| render_table_row(line.trim()))
            .collect::<Vec<_>>();
    }

    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; column_count];
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    let mut rendered = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        rendered.push(render_table_row_aligned(row, &widths));
        if index == 0 && rows.len() > 1 {
            rendered.push(render_table_rule(&widths));
        }
    }
    rendered
}

fn render_table_row_aligned(row: &[String], widths: &[usize]) -> String {
    let mut parts = Vec::with_capacity(widths.len());
    for (index, width) in widths.iter().enumerate() {
        let value = row.get(index).map(String::as_str).unwrap_or("");
        let rendered = render_inline_markdown(value);
        let pad = width.saturating_sub(value.chars().count());
        parts.push(format!("{rendered}{}", " ".repeat(pad)));
    }
    parts.join("   ")
}

fn render_table_rule(widths: &[usize]) -> String {
    let total_width = widths
        .iter()
        .map(|width| (*width).max(1))
        .sum::<usize>()
        + widths.len().saturating_sub(1) * 3;
    format!("{ANSI_DIM}{}{ANSI_RESET}", "─".repeat(total_width.max(1)))
}

fn render_inline_markdown(value: &str) -> String {
    let mut out = String::new();
    let chars = value.chars().collect::<Vec<_>>();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_double_star_end(&chars, i + 2) {
                let content = chars[i + 2..end].iter().collect::<String>();
                out.push_str(ANSI_BOLD);
                out.push_str(&content);
                out.push_str(ANSI_RESET);
                i = end + 2;
                continue;
            }
        }

        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|ch| *ch == '`') {
                let end_index = i + 1 + end;
                let content = chars[i + 1..end_index].iter().collect::<String>();
                out.push_str(ANSI_YELLOW);
                out.push_str(&content);
                out.push_str(ANSI_RESET);
                i = end_index + 1;
                continue;
            }
        }

        if chars[i] == '[' {
            if let Some((text_end, url_start, url_end)) = find_link_bounds(&chars, i) {
                let label = chars[i + 1..text_end].iter().collect::<String>();
                let url = chars[url_start..url_end].iter().collect::<String>();
                out.push_str(&label);
                out.push_str(" (");
                out.push_str(&url);
                out.push(')');
                i = url_end + 1;
                continue;
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn find_double_star_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '*' && chars[i + 1] == '*' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_link_bounds(chars: &[char], start: usize) -> Option<(usize, usize, usize)> {
    let text_end = chars[start + 1..]
        .iter()
        .position(|ch| *ch == ']')
        .map(|idx| start + 1 + idx)?;
    if chars.get(text_end + 1) != Some(&'(') {
        return None;
    }
    let url_end = chars[text_end + 2..]
        .iter()
        .position(|ch| *ch == ')')
        .map(|idx| text_end + 2 + idx)?;
    Some((text_end, text_end + 2, url_end))
}
