use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::config::config_dir;

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionConfig {
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct ConnectionsFile {
    #[serde(default)]
    connections: BTreeMap<String, ConnectionConfig>,
}

pub fn connections_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("connections.toml"))
}

pub fn load_connections() -> Result<BTreeMap<String, ConnectionConfig>, String> {
    let path = connections_path()?;
    let contents = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(format!("Failed to read {}: {err}", path.display())),
    };
    let parsed: ConnectionsFile = toml::from_str(&contents)
        .map_err(|err| format!("Failed to parse {}: {err}", path.display()))?;
    Ok(parsed.connections)
}

pub fn get_connection(name: &str) -> Result<Option<ConnectionConfig>, String> {
    Ok(load_connections()?.remove(name))
}
