use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::config::{config_dir, home_dir_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEventPayload {
    pub adapter: String,
    pub event_type: String,
    pub agent_id: String,
    pub command: Option<String>,
    pub output: Option<String>,
    pub details: Option<String>,
}

pub fn hook_bin_dir() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("ai").join("bin"))
}

pub fn codex_home_dir() -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("CODEX_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    Ok(home_dir_path().join(".codex"))
}

pub fn codex_config_path() -> Result<PathBuf, String> {
    Ok(codex_home_dir()?.join("config.toml"))
}

pub fn codex_hooks_path() -> Result<PathBuf, String> {
    Ok(codex_home_dir()?.join("hooks.json"))
}

pub fn claude_home_dir() -> Result<PathBuf, String> {
    if let Ok(value) = std::env::var("CLAUDE_CONFIG_DIR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    Ok(home_dir_path().join(".claude"))
}

pub fn claude_settings_path() -> Result<PathBuf, String> {
    Ok(claude_home_dir()?.join("settings.json"))
}

pub fn hooks_supported() -> bool {
    !cfg!(target_os = "windows")
}

pub fn hooks_enabled() -> Result<bool, String> {
    if !hooks_supported() {
        return Ok(false);
    }

    let config_enabled = read_codex_hooks_enabled()?;
    let hooks_installed = read_hooks_json()?
        .map(|value| hooks_json_contains_tinfo(&value))
        .unwrap_or(false);
    let claude_enabled = read_claude_hooks_enabled()?;
    Ok((config_enabled && hooks_installed) || claude_enabled)
}

pub fn install_hooks(api_bind: &str, tinfo_exe: &Path) -> Result<Vec<PathBuf>, String> {
    if !hooks_supported() {
        return Err("Agent hooks are not supported on Windows yet.".to_string());
    }

    let codex_home = codex_home_dir()?;
    fs::create_dir_all(&codex_home)
        .map_err(|err| format!("Failed to create {}: {err}", codex_home.display()))?;

    let config_path = codex_config_path()?;
    let hooks_path = codex_hooks_path()?;
    backup_if_exists(&config_path)?;
    backup_if_exists(&hooks_path)?;

    write_codex_config(&config_path)?;
    write_hooks_json(&hooks_path, api_bind, tinfo_exe)?;

    let claude_home = claude_home_dir()?;
    fs::create_dir_all(&claude_home)
        .map_err(|err| format!("Failed to create {}: {err}", claude_home.display()))?;
    let claude_settings = claude_settings_path()?;
    backup_if_exists(&claude_settings)?;
    write_claude_settings(&claude_settings, tinfo_exe)?;

    Ok(vec![codex_home, claude_home])
}

pub fn uninstall_hooks() -> Result<Vec<PathBuf>, String> {
    let hooks_path = codex_hooks_path()?;
    if let Some(mut value) = read_hooks_json()? {
        remove_tinfo_hooks(&mut value);
        write_json_pretty(&hooks_path, &value)?;
    }

    let claude_settings = claude_settings_path()?;
    if let Some(mut value) = read_claude_settings()? {
        remove_tinfo_claude_hooks(&mut value);
        write_json_pretty(&claude_settings, &value)?;
    }
    Ok(vec![hooks_path, claude_settings])
}

pub fn read_hook_event_from_stdin(
    adapter: Option<&str>,
    event_type: Option<&str>,
) -> Result<HookEventPayload, String> {
    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .map_err(|err| format!("Failed to read hook event payload: {err}"))?;

    let parsed = if stdin.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str::<Value>(&stdin)
            .map_err(|err| format!("Invalid hook event payload: {err}"))?
    };

    let raw_event_type = event_type
        .map(|value| value.to_string())
        .or_else(|| {
            first_string(&parsed, &[
                &["event_type"],
                &["hook_event_name"],
                &["event"],
                &["hook_event", "name"],
                &["eventName"],
            ])
        })
        .unwrap_or_else(|| "output".to_string());
    let adapter = adapter
        .map(|value| value.to_string())
        .or_else(|| infer_adapter(&parsed, &raw_event_type))
        .unwrap_or_else(|| "codex".to_string());
    let normalized_event_type = normalize_event_type(&adapter, &raw_event_type, &parsed);
    let command = extract_hook_command(&parsed);
    let output = extract_hook_output(&parsed);
    let details = extract_hook_details(&parsed);
    let agent_id = first_string(&parsed, &[
        &["agent_id"],
        &["session_id"],
        &["thread_id"],
    ])
    .unwrap_or_else(|| adapter.to_string());

    Ok(HookEventPayload {
        adapter: adapter.to_string(),
        event_type: normalized_event_type,
        agent_id,
        command,
        output,
        details,
    })
}

fn read_codex_hooks_enabled() -> Result<bool, String> {
    let path = codex_config_path()?;
    if !path.exists() {
        return Ok(false);
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
    let value = text
        .parse::<toml::Value>()
        .map_err(|err| format!("Failed to parse {}: {err}", path.display()))?;
    Ok(value
        .get("codex_hooks")
        .and_then(|value| value.as_bool())
        .unwrap_or(false))
}

fn read_claude_hooks_enabled() -> Result<bool, String> {
    Ok(read_claude_settings()?
        .map(|value| claude_settings_contains_tinfo(&value))
        .unwrap_or(false))
}

fn write_codex_config(path: &Path) -> Result<(), String> {
    let mut value = if path.exists() {
        fs::read_to_string(path)
            .map_err(|err| format!("Failed to read {}: {err}", path.display()))?
            .parse::<toml::Value>()
            .map_err(|err| format!("Failed to parse {}: {err}", path.display()))?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    let Some(table) = value.as_table_mut() else {
        return Err(format!("{} must contain a TOML table.", path.display()));
    };
    table.insert("codex_hooks".to_string(), toml::Value::Boolean(true));
    let encoded = toml::to_string_pretty(&value)
        .map_err(|err| format!("Failed to encode {}: {err}", path.display()))?;
    fs::write(path, encoded).map_err(|err| format!("Failed to write {}: {err}", path.display()))?;
    Ok(())
}

fn write_hooks_json(path: &Path, _api_bind: &str, tinfo_exe: &Path) -> Result<(), String> {
    let mut value = read_hooks_json()?.unwrap_or_else(default_hooks_json);
    remove_tinfo_hooks(&mut value);
    merge_tinfo_hooks(&mut value, tinfo_exe)?;
    write_json_pretty(path, &value)
}

fn write_claude_settings(path: &Path, tinfo_exe: &Path) -> Result<(), String> {
    let mut value = read_claude_settings()?.unwrap_or_else(|| json!({}));
    remove_tinfo_claude_hooks(&mut value);
    merge_tinfo_claude_hooks(&mut value, tinfo_exe)?;
    write_json_pretty(path, &value)
}

fn read_hooks_json() -> Result<Option<Value>, String> {
    let path = codex_hooks_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
    let value = serde_json::from_str::<Value>(&text)
        .map_err(|err| format!("Failed to parse {}: {err}", path.display()))?;
    Ok(Some(value))
}

fn read_claude_settings() -> Result<Option<Value>, String> {
    let path = claude_settings_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
    let value = serde_json::from_str::<Value>(&text)
        .map_err(|err| format!("Failed to parse {}: {err}", path.display()))?;
    Ok(Some(value))
}

fn default_hooks_json() -> Value {
    json!({ "hooks": {} })
}

fn merge_tinfo_hooks(value: &mut Value, tinfo_exe: &Path) -> Result<(), String> {
    let Some(root) = value.as_object_mut() else {
        return Err("Codex hooks file must contain a JSON object.".to_string());
    };
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(hooks_map) = hooks.as_object_mut() else {
        return Err("Codex hooks file must contain a top-level 'hooks' object.".to_string());
    };

    for (hook_name, event_type) in [
        ("SessionStart", "session_start"),
        ("Stop", "stop"),
        ("UserPromptSubmit", "user_prompt_submit"),
        ("CommandRequest", "command_request"),
        ("CommandStart", "command_start"),
        ("CommandFinish", "command_finish"),
        ("Output", "output"),
    ] {
        let entries = hooks_map
            .entry(hook_name.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(array) = entries.as_array_mut() else {
            return Err(format!("Codex hook entry '{hook_name}' must be an array."));
        };
        let command = build_hook_command(tinfo_exe, "codex", event_type);
        let already_present = array.iter().any(|entry| {
            entry.get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.get("command").and_then(Value::as_str))
                .any(|existing| existing == command)
        });
        if !already_present {
            array.push(json!({
                "hooks": [{
                    "type": "command",
                    "timeout": 5,
                    "command": command,
                }]
            }));
        }
    }

    Ok(())
}

fn merge_tinfo_claude_hooks(value: &mut Value, tinfo_exe: &Path) -> Result<(), String> {
    let Some(root) = value.as_object_mut() else {
        return Err("Claude settings file must contain a JSON object.".to_string());
    };
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(hooks_map) = hooks.as_object_mut() else {
        return Err("Claude settings file must contain a top-level 'hooks' object.".to_string());
    };

    for (hook_name, event_type, matcher) in [
        ("SessionStart", "session_start", None),
        ("SessionEnd", "stop", None),
        ("Stop", "stop", None),
        ("SubagentStop", "stop", None),
        ("UserPromptSubmit", "user_prompt_submit", None),
        ("Notification", "output", None),
        ("PermissionRequest", "command_request", None),
        ("PreToolUse", "command_request", Some("*")),
        ("PostToolUse", "command_finish", Some("*")),
    ] {
        let entries = hooks_map
            .entry(hook_name.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(array) = entries.as_array_mut() else {
            return Err(format!("Claude hook entry '{hook_name}' must be an array."));
        };
        let command = build_hook_command(tinfo_exe, "claude_code", event_type);
        let already_present = array.iter().any(|entry| {
            entry.get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.get("command").and_then(Value::as_str))
                .any(|existing| existing == command)
        });
        if !already_present {
            let mut entry = Map::new();
            let mut hook = Map::new();
            hook.insert("type".to_string(), Value::String("command".to_string()));
            hook.insert("command".to_string(), Value::String(command));
            entry.insert("hooks".to_string(), Value::Array(vec![Value::Object(hook)]));
            if let Some(matcher) = matcher {
                entry.insert("matcher".to_string(), Value::String(matcher.to_string()));
            }
            array.push(Value::Object(entry));
        }
    }

    Ok(())
}

fn remove_tinfo_hooks(value: &mut Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    for entries in hooks.values_mut() {
        let Some(array) = entries.as_array_mut() else {
            continue;
        };
        array.retain(|entry| {
            let command = entry
                .get("hooks")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("command"))
                .and_then(Value::as_str);
            !command
                .map(is_tinfo_codex_hook_command)
                .unwrap_or(false)
        });
    }
}

fn remove_tinfo_claude_hooks(value: &mut Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    for entries in hooks.values_mut() {
        let Some(array) = entries.as_array_mut() else {
            continue;
        };
        array.retain(|entry| {
            let command = entry
                .get("hooks")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("command"))
                .and_then(Value::as_str);
            !command.map(is_tinfo_claude_hook_command).unwrap_or(false)
        });
    }
}

fn hooks_json_contains_tinfo(value: &Value) -> bool {
    value.get("hooks")
        .and_then(Value::as_object)
        .map(|hooks| {
            hooks.values().any(|entries| {
                entries
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|entry| {
                        entry
                            .get("hooks")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|item| item.get("command").and_then(Value::as_str))
                            .any(is_current_tinfo_codex_hook_command)
                    })
            })
        })
        .unwrap_or(false)
}

fn claude_settings_contains_tinfo(value: &Value) -> bool {
    value.get("hooks")
        .and_then(Value::as_object)
        .map(|hooks| {
            hooks.values().any(|entries| {
                entries
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|entry| {
                        entry
                            .get("hooks")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(|item| item.get("command").and_then(Value::as_str))
                            .any(is_current_tinfo_claude_hook_command)
                    })
            })
        })
        .unwrap_or(false)
}

fn build_hook_command(tinfo_exe: &Path, adapter: &str, event_type: &str) -> String {
    let _ = (adapter, event_type);
    format!("'{}' hook-handler", tinfo_exe.display())
}

fn normalize_event_type(adapter: &str, event_type: &str, parsed: &Value) -> String {
    let raw = first_string(parsed, &[&["hook_event_name"]]).unwrap_or_else(|| event_type.to_string());
    let adapter = adapter.trim().to_ascii_lowercase();
    if adapter == "claude" || adapter == "claude_code" || adapter == "claude code" {
        return match raw.as_str() {
            "SessionStart" => "session_start",
            "UserPromptSubmit" => "user_prompt_submit",
            "Notification" => "output",
            "PreToolUse" | "PermissionRequest" => "command_request",
            "PostToolUse" => "command_finish",
            "Stop" | "SessionEnd" | "SubagentStop" => "stop",
            _ => event_type,
        }
        .to_string();
    }
    raw.to_ascii_lowercase()
}

fn extract_hook_command(parsed: &Value) -> Option<String> {
    if let Some(command) = first_string(parsed, &[
        &["command"],
        &["payload", "command"],
        &["tool", "command"],
        &["details", "command"],
        &["tool_input", "command"],
        &["tool_input", "cmd"],
    ]) {
        return Some(command);
    }

    let tool_name = first_string(parsed, &[&["tool_name"]])?;
    let target = first_string(parsed, &[
        &["tool_input", "file_path"],
        &["tool_input", "path"],
        &["tool_input", "url"],
        &["tool_input", "query"],
        &["tool_input", "description"],
        &["tool_input", "prompt"],
    ]);
    Some(match target {
        Some(target) if !target.trim().is_empty() => format!("{tool_name}: {target}"),
        _ => tool_name,
    })
}

fn extract_hook_output(parsed: &Value) -> Option<String> {
    first_string(parsed, &[
        &["output"],
        &["message"],
        &["payload", "output"],
        &["payload", "message"],
        &["text"],
        &["tool_response", "stdout"],
        &["tool_response", "stderr"],
        &["tool_response", "message"],
    ])
}

fn extract_hook_details(parsed: &Value) -> Option<String> {
    first_string(parsed, &[
        &["details"],
        &["payload", "details"],
        &["tool_name"],
        &["reason"],
    ])
}

fn infer_adapter(parsed: &Value, raw_event_type: &str) -> Option<String> {
    if let Some(adapter) = first_string(parsed, &[
        &["adapter"],
        &["source"],
        &["client"],
        &["provider"],
    ]) {
        let normalized = adapter.trim().to_ascii_lowercase();
        if normalized.contains("claude") {
            return Some("claude_code".to_string());
        }
        if normalized.contains("gemini") {
            return Some("gemini".to_string());
        }
        if normalized.contains("codex") {
            return Some("codex".to_string());
        }
    }

    match raw_event_type {
        "SessionEnd" | "PreToolUse" | "PostToolUse" | "PermissionRequest" | "Notification"
        | "SubagentStop" | "SubagentStart" => {
            Some("claude_code".to_string())
        }
        _ => None,
    }
}

fn is_tinfo_codex_hook_command(command: &str) -> bool {
    command.contains("hook-handler") || command.contains("agent hook event codex")
}

fn is_tinfo_claude_hook_command(command: &str) -> bool {
    command.contains("hook-handler") || command.contains("agent hook event claude_code")
}

fn is_current_tinfo_codex_hook_command(command: &str) -> bool {
    command.contains("hook-handler")
}

fn is_current_tinfo_claude_hook_command(command: &str) -> bool {
    command.contains("hook-handler")
}

fn backup_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System time error: {err}"))?
        .as_secs();
    let backup_path = path.with_extension(format!(
        "{}.bak.{}",
        path.extension().and_then(|ext| ext.to_str()).unwrap_or("backup"),
        timestamp
    ));
    fs::copy(path, &backup_path).map_err(|err| {
        format!(
            "Failed to back up {} to {}: {err}",
            path.display(),
            backup_path.display()
        )
    })?;
    Ok(())
}

fn write_json_pretty(path: &Path, value: &Value) -> Result<(), String> {
    let encoded = serde_json::to_string_pretty(value)
        .map_err(|err| format!("Failed to encode {}: {err}", path.display()))?;
    fs::write(path, encoded).map_err(|err| format!("Failed to write {}: {err}", path.display()))
}

fn first_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| lookup_string(value, path))
}

fn lookup_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(|value| value.to_string())
}
