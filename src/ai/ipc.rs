use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ai::hook::HookEventPayload;
use crate::ai::runtime::Runtime;
use crate::config::config_dir;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentDecision {
    pub agent_id: String,
    pub request_id: String,
    pub decision: String,
}

pub fn events_log_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("events.log"))
}

fn decisions_dir_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("decisions"))
}

fn decision_path(agent_id: &str) -> Result<PathBuf, String> {
    Ok(decisions_dir_path()?.join(format!("{agent_id}.json")))
}

pub fn append_hook_event(event: &HookEventPayload) -> Result<(), String> {
    let path = events_log_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create {}: {err}", parent.display()))?;
    }
    let encoded = serde_json::to_string(event)
        .map_err(|err| format!("Failed to encode hook event: {err}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("Failed to open {}: {err}", path.display()))?;
    file.write_all(encoded.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|err| format!("Failed to append {}: {err}", path.display()))?;
    eprintln!("HOOK EVENT WRITTEN: {}", event.event_type);
    Ok(())
}

pub fn write_agent_decision(decision: &AgentDecision) -> Result<(), String> {
    let dir = decisions_dir_path()?;
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to create {}: {err}", dir.display()))?;
    let path = decision_path(&decision.agent_id)?;
    let encoded = serde_json::to_string(decision)
        .map_err(|err| format!("Failed to encode agent decision: {err}"))?;
    fs::write(&path, encoded)
        .map_err(|err| format!("Failed to write {}: {err}", path.display()))?;
    Ok(())
}

pub fn take_agent_decision(agent_id: &str) -> Result<Option<AgentDecision>, String> {
    let path = decision_path(agent_id)?;
    let encoded = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("Failed to read {}: {err}", path.display())),
    };
    fs::remove_file(&path)
        .map_err(|err| format!("Failed to remove {}: {err}", path.display()))?;
    let decision = serde_json::from_str(&encoded)
        .map_err(|err| format!("Failed to decode {}: {err}", path.display()))?;
    Ok(Some(decision))
}

pub fn start_event_log_consumer(runtime: Runtime) {
    thread::spawn(move || {
        let path = match events_log_path() {
            Ok(path) => path,
            Err(err) => {
                eprintln!("HOOK EVENT READ FAILED: {err}");
                return;
            }
        };
        let mut offset = 0_u64;
        loop {
            match File::open(&path) {
                Ok(file) => {
                    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
                    if offset == 0 {
                        // Start at EOF so a new agent dashboard only follows live events.
                        offset = len;
                    } else if offset > len {
                        offset = 0;
                    }
                    let mut reader = BufReader::new(file);
                    if reader.seek(SeekFrom::Start(offset)).is_err() {
                        offset = 0;
                        thread::sleep(Duration::from_millis(250));
                        continue;
                    }

                    let mut line = String::new();
                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) => {
                                offset = reader.stream_position().unwrap_or(offset);
                                break;
                            }
                            Ok(_) => {
                                offset = reader.stream_position().unwrap_or(offset);
                                let trimmed = line.trim();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                match serde_json::from_str::<HookEventPayload>(trimmed) {
                                    Ok(event) => {
                                        eprintln!("HOOK EVENT READ: {}", event.event_type);
                                        let _ = runtime.ingest_hook_event(event);
                                    }
                                    Err(err) => {
                                        eprintln!("HOOK EVENT READ FAILED: {err}");
                                    }
                                }
                            }
                            Err(err) => {
                                eprintln!("HOOK EVENT READ FAILED: {err}");
                                break;
                            }
                        }
                    }
                }
                Err(_) => {}
            }
            thread::sleep(Duration::from_millis(250));
        }
    });
}
