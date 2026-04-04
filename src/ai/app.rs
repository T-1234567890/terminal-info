use std::io::{self, IsTerminal};

use dialoguer::{Confirm, Password};

use crate::ai::chat::ProviderKind;
use crate::ai::config::AiConfig;
use crate::ai::hook::{hooks_enabled, hooks_supported, install_hooks};
use crate::ai::runtime::Runtime;
use crate::ai::ui::{FocusPane, ViewMode, run_dashboard};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum EntryMode {
    Dashboard,
    Agent,
    Chat,
}

pub fn main(program_name: &str) {
    if let Err(err) = run(parse_mode_from_env()) {
        eprintln!("{program_name} error: {err}");
        std::process::exit(1);
    }
}

pub fn run_entry(mode: EntryMode) -> Result<(), String> {
    run(mode)
}

fn run(mode: EntryMode) -> Result<(), String> {
    if cfg!(target_os = "windows") && mode == EntryMode::Agent {
        println!("Agent hooks are not supported on Windows yet.");
        println!("Use `tinfo chat` instead.");
        return Ok(());
    }

    let config = maybe_prompt_for_api_key(AiConfig::load_default(), mode)?;
    maybe_prompt_for_agent_hooks(mode, &config)?;
    maybe_warn_gemini_experimental(mode, &config);
    let runtime = Runtime::new(config);
    run_dashboard(runtime, focus_for_mode(mode), view_mode(mode))
}

fn maybe_prompt_for_api_key(config: AiConfig, mode: EntryMode) -> Result<AiConfig, String> {
    if mode != EntryMode::Chat && mode != EntryMode::Dashboard {
        return Ok(config);
    }

    if config.any_provider_configured() || !io::stdin().is_terminal() || !io::stdout().is_terminal()
    {
        return Ok(config);
    }

    let install = Confirm::new()
        .with_prompt(
            "No AI provider API key found. Add an OpenRouter API key now? (Recommended · multi-model support)",
        )
        .default(true)
        .interact()
        .map_err(|err| format!("Failed to read confirmation: {err}"))?;
    if !install {
        return Ok(config);
    }

    let api_key = Password::new()
        .with_prompt("OpenRouter API key (Recommended · multi-model support)")
        .allow_empty_password(false)
        .interact()
        .map_err(|err| format!("Failed to read API key: {err}"))?;
    AiConfig::save_provider_api_key(ProviderKind::OpenRouter, api_key)
}

fn parse_mode_from_env() -> EntryMode {
    match std::env::args().nth(1).as_deref() {
        Some("agent") => EntryMode::Agent,
        Some("chat") => EntryMode::Chat,
        _ => EntryMode::Dashboard,
    }
}

fn maybe_prompt_for_agent_hooks(mode: EntryMode, config: &AiConfig) -> Result<(), String> {
    if mode != EntryMode::Agent || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(());
    }

    if !hooks_supported() {
        println!("Agent hooks are not supported on Windows yet.");
        return Ok(());
    }

    if hooks_enabled()? {
        return Ok(());
    }

    let enable = Confirm::new()
        .with_prompt(
            "Enable Codex and Claude Code hooks for full agent functionality?\nThis will modify your Codex and Claude Code configuration.",
        )
        .default(true)
        .interact()
        .map_err(|err| format!("Failed to read setup choice: {err}"))?;

    if !enable {
        println!("Hooks not enabled. Agent features will be limited.");
        return Ok(());
    }

    let current_exe = std::env::current_exe()
        .map_err(|err| format!("Failed to locate current executable: {err}"))?;
    let paths = install_hooks(config.api_bind(), &current_exe)?;
    println!("Enabled Codex and Claude Code hooks.");
    for path in paths {
        println!("Updated {}", path.display());
    }
    Ok(())
}

fn maybe_warn_gemini_experimental(mode: EntryMode, config: &AiConfig) {
    if mode != EntryMode::Agent {
        return;
    }

    if config
        .agents()
        .iter()
        .any(|agent| agent.adapter == crate::ai::adapters::AgentAdapterKind::Gemini)
    {
        println!("[Experimental] Gemini does not support hooks. Behavior may be unstable.");
    }
}

fn focus_for_mode(mode: EntryMode) -> FocusPane {
    match mode {
        EntryMode::Dashboard => FocusPane::Agents,
        EntryMode::Agent => FocusPane::Agents,
        EntryMode::Chat => FocusPane::Chat,
    }
}

fn view_mode(mode: EntryMode) -> ViewMode {
    match mode {
        EntryMode::Dashboard => ViewMode::Dashboard,
        EntryMode::Agent => ViewMode::Agent,
        EntryMode::Chat => ViewMode::Chat,
    }
}
