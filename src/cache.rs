use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize)]
struct CacheEntry<T> {
    timestamp: u64,
    data: T,
}

pub fn read_cache<T: DeserializeOwned>(key: &str, ttl_secs: u64) -> Option<T> {
    let path = cache_file_path(key).ok()?;
    let contents = fs::read_to_string(path).ok()?;
    let entry: CacheEntry<T> = serde_json::from_str(&contents).ok()?;
    if now_unix().saturating_sub(entry.timestamp) <= ttl_secs {
        Some(entry.data)
    } else {
        None
    }
}

pub fn write_cache<T: Serialize>(key: &str, data: &T) -> Result<(), String> {
    let path = cache_file_path(key)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create cache directory: {err}"))?;
    }

    let entry = CacheEntry {
        timestamp: now_unix(),
        data,
    };
    let json = serde_json::to_string_pretty(&entry)
        .map_err(|err| format!("Failed to serialize cache entry: {err}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|err| format!("Failed to write cache entry: {err}"))
}

pub fn cache_file_path(key: &str) -> Result<PathBuf, String> {
    let base = cache_dir_path()?;
    Ok(base.join(format!("{key}.json")))
}

pub fn cache_dir_path() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }

    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".tinfo").join("cache"))
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
