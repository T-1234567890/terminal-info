use std::collections::HashSet;
use std::path::PathBuf;

use sysinfo::{Pid, System};

use crate::ai::adapters::AgentAdapterKind;
use crate::config::{AiAgentCliConfig, Config};

#[derive(Debug, Clone)]
pub struct DiscoveredAgent {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub adapter: AgentAdapterKind,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub display_name: String,
}

pub fn discover_agents() -> Vec<DiscoveredAgent> {
    let config = Config::load_or_create().ok();
    let configured = configured_commands(config.as_ref());
    let mut system = System::new_all();
    system.refresh_all();

    let mut discovered = system
        .processes()
        .values()
        .filter_map(|process| {
            let process_name = process.name().to_string_lossy().to_ascii_lowercase();
            let command = process
                .exe()
                .map(|path| path.to_string_lossy().to_string())
                .or_else(|| process.cmd().first().map(|value| value.to_string_lossy().to_string()))
                .unwrap_or_else(|| process.name().to_string_lossy().to_string());
            let command_name = command_name(&command);
            let adapter = match_agent(&command_name, &process_name, &configured)?;
            let args = process
                .cmd()
                .iter()
                .skip(1)
                .map(|value| value.to_string_lossy().to_string())
                .collect::<Vec<_>>();
            let cwd = process.cwd().map(|path| path.to_string_lossy().to_string());

            Some(DiscoveredAgent {
                pid: pid_to_u32(process.pid()),
                parent_pid: process.parent().map(pid_to_u32),
                adapter,
                command,
                args,
                cwd,
                display_name: display_name(adapter).to_string(),
            })
        })
        .collect::<Vec<_>>();

    discovered.sort_by_key(|agent| (adapter_sort_key(agent.adapter), agent.pid));
    discovered
}

pub fn discovered_agent_id(agent: &DiscoveredAgent) -> String {
    format!("external-{}-{}", agent.adapter.label(), agent.pid)
}

pub fn attach_discovered_agent(pid: u32) -> Result<String, String> {
    let discovered = discover_agents()
        .into_iter()
        .find(|agent| agent.pid == pid)
        .ok_or_else(|| format!("No supported external agent process with pid {pid} was found."))?;

    let mut config = Config::load_or_create()?;
    let name = unique_agent_name(&config, discovered.adapter, discovered.pid);
    config.ai.agents.insert(
        name.clone(),
        AiAgentCliConfig {
            enabled: true,
            auto_start: false,
            adapter: Some(discovered.adapter.label().to_string()),
            command: discovered.command.clone(),
            args: discovered.args.clone(),
            cwd: discovered.cwd.clone(),
            env: Default::default(),
        },
    );
    config.save()?;
    Ok(name)
}

fn configured_commands(config: Option<&Config>) -> HashSet<String> {
    let mut commands = HashSet::new();
    if let Some(config) = config {
        for command in [
            config.ai.adapters.codex.command.as_str(),
            config.ai.adapters.claude_code.command.as_str(),
            config.ai.adapters.gemini.command.as_str(),
        ] {
            if !command.trim().is_empty() {
                commands.insert(command_name(command));
            }
        }
        for agent in config.ai.agents.values() {
            if !agent.command.trim().is_empty() {
                commands.insert(command_name(&agent.command));
            }
        }
    }
    commands
}

fn match_agent(
    command_name: &str,
    process_name: &str,
    _configured: &HashSet<String>,
) -> Option<AgentAdapterKind> {
    if matches!(command_name, "codex" | "codex.exe")
        || process_name == "codex"
    {
        return Some(AgentAdapterKind::Codex);
    }
    if matches!(
        command_name,
        "claude" | "claude.exe" | "claude-code" | "claude_code"
    ) || matches!(process_name, "claude" | "claude-code" | "claude_code")
    {
        return Some(AgentAdapterKind::ClaudeCode);
    }
    if matches!(command_name, "gemini" | "gemini.exe" | "gemini-cli")
        || matches!(process_name, "gemini" | "gemini-cli")
    {
        return Some(AgentAdapterKind::Gemini);
    }
    None
}

fn command_name(command: &str) -> String {
    PathBuf::from(command)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
        .to_ascii_lowercase()
}

fn unique_agent_name(config: &Config, adapter: AgentAdapterKind, pid: u32) -> String {
    let base = format!("{}-{}", adapter.label(), pid);
    if !config.ai.agents.contains_key(&base) {
        return base;
    }
    let mut index = 2usize;
    loop {
        let candidate = format!("{base}-{index}");
        if !config.ai.agents.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn display_name(adapter: AgentAdapterKind) -> &'static str {
    match adapter {
        AgentAdapterKind::Codex => "Codex CLI",
        AgentAdapterKind::ClaudeCode => "Claude Code",
        AgentAdapterKind::Gemini => "Gemini CLI",
        AgentAdapterKind::Generic => "Generic Agent CLI",
    }
}

fn adapter_sort_key(adapter: AgentAdapterKind) -> u8 {
    match adapter {
        AgentAdapterKind::Codex => 0,
        AgentAdapterKind::ClaudeCode => 1,
        AgentAdapterKind::Gemini => 2,
        AgentAdapterKind::Generic => 3,
    }
}

fn pid_to_u32(pid: Pid) -> u32 {
    pid.as_u32()
}
