use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use reqwest::blocking::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{ConfigContext, Log, Output, PluginError, PluginResult, ResultExt};

#[derive(Clone)]
pub(crate) enum IoMode {
    Terminal,
    Buffered {
        stdout: Arc<Mutex<String>>,
        stderr: Arc<Mutex<String>>,
    },
}

#[derive(Clone)]
pub(crate) struct RuntimeState {
    host: RuntimeHost,
    io: IoMode,
}

impl RuntimeState {
    pub(crate) fn terminal(host: RuntimeHost) -> Self {
        Self {
            host,
            io: IoMode::Terminal,
        }
    }

    pub(crate) fn buffered(host: RuntimeHost) -> Self {
        Self {
            host,
            io: IoMode::Buffered {
                stdout: Arc::new(Mutex::new(String::new())),
                stderr: Arc::new(Mutex::new(String::new())),
            },
        }
    }

    pub(crate) fn host(&self) -> &RuntimeHost {
        &self.host
    }

    pub(crate) fn write_stdout(&self, message: &str) {
        match &self.io {
            IoMode::Terminal => println!("{message}"),
            IoMode::Buffered { stdout, .. } => {
                let mut buffer = stdout.lock().expect("stdout buffer poisoned");
                buffer.push_str(message);
                buffer.push('\n');
            }
        }
    }

    pub(crate) fn write_stderr(&self, message: &str) {
        match &self.io {
            IoMode::Terminal => eprintln!("{message}"),
            IoMode::Buffered { stderr, .. } => {
                let mut buffer = stderr.lock().expect("stderr buffer poisoned");
                buffer.push_str(message);
                buffer.push('\n');
            }
        }
    }

    pub(crate) fn stdout_contents(&self) -> Option<String> {
        match &self.io {
            IoMode::Terminal => None,
            IoMode::Buffered { stdout, .. } => {
                Some(stdout.lock().expect("stdout buffer poisoned").clone())
            }
        }
    }

    pub(crate) fn stderr_contents(&self) -> Option<String> {
        match &self.io {
            IoMode::Terminal => None,
            IoMode::Buffered { stderr, .. } => {
                Some(stderr.lock().expect("stderr buffer poisoned").clone())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeHost {
    pub plugin_name: String,
    pub host_version: String,
    pub plugin_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub config_path: PathBuf,
}

impl RuntimeHost {
    pub(crate) fn from_env(plugin_name: impl Into<String>) -> Self {
        let plugin_name = plugin_name.into();
        let terminal_home = env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".terminal-info");
        let plugin_dir = env::var("TINFO_PLUGIN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| terminal_home.join("plugins"));
        let cache_dir = env::var("TINFO_PLUGIN_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| terminal_home.join("cache"));
        let config_path = env::var("TINFO_CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".tinfo")
                    .join("config.toml")
            });

        Self {
            plugin_name,
            host_version: env::var("TINFO_HOST_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
            plugin_dir,
            cache_dir,
            config_path,
        }
    }

    pub(crate) fn into_env(self) -> std::collections::BTreeMap<String, String> {
        let mut envs = std::collections::BTreeMap::new();
        envs.insert("TINFO_HOST_VERSION".to_string(), self.host_version);
        envs.insert(
            "TINFO_PLUGIN_DIR".to_string(),
            self.plugin_dir.display().to_string(),
        );
        envs.insert(
            "TINFO_PLUGIN_CACHE_DIR".to_string(),
            self.cache_dir.display().to_string(),
        );
        envs.insert(
            "TINFO_CONFIG_PATH".to_string(),
            self.config_path.display().to_string(),
        );
        if let Ok(contents) = fs::read_to_string(&self.config_path) {
            if let Ok(value) = toml::from_str::<toml::Value>(&contents) {
                if let Ok(json) = serde_json::to_string(&value) {
                    envs.insert("TINFO_PLUGIN_CONFIG_JSON".to_string(), json);
                }
            }
        }
        envs
    }
}

/// Full host context passed into plugin handlers.
#[derive(Clone)]
pub struct Context {
    state: Arc<RuntimeState>,
    pub system: SystemContext,
    pub host: HostContext,
    pub config: ConfigContext,
    pub cache: CacheContext,
    pub fs: FsContext,
    pub network: NetworkContext,
    output: Output,
    log: Log,
}

impl Context {
    pub(crate) fn new(state: Arc<RuntimeState>, config: ConfigContext) -> Self {
        Self {
            state: state.clone(),
            system: SystemContext,
            host: HostContext {
                state: state.clone(),
            },
            config,
            cache: CacheContext {
                state: state.clone(),
            },
            fs: FsContext {
                state: state.clone(),
            },
            network: NetworkContext {
                client: Client::new(),
            },
            output: Output::new(state.clone()),
            log: Log::new(state),
        }
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.state.host().cache_dir.clone()
    }

    pub fn plugin_dir(&self) -> PathBuf {
        self.state.host().plugin_dir.clone()
    }

    pub fn output(&self) -> &Output {
        &self.output
    }

    pub fn log(&self) -> &Log {
        &self.log
    }
}

/// System facts exposed to plugins.
#[derive(Clone, Debug)]
pub struct SystemContext;

impl SystemContext {
    pub fn os(&self) -> &'static str {
        env::consts::OS
    }

    pub fn arch(&self) -> &'static str {
        env::consts::ARCH
    }
}

/// Host facts exposed to plugins.
#[derive(Clone)]
pub struct HostContext {
    state: Arc<RuntimeState>,
}

impl HostContext {
    pub fn version(&self) -> String {
        self.state.host().host_version.clone()
    }

    pub fn plugin_name(&self) -> &str {
        &self.state.host().plugin_name
    }
}

/// Capability-aware cache helper bound to plugin-owned storage.
#[derive(Clone)]
pub struct CacheContext {
    state: Arc<RuntimeState>,
}

impl CacheContext {
    pub fn root_dir(&self) -> PathBuf {
        self.state.host().cache_dir.clone()
    }

    pub fn plugin_dir(&self) -> PathBuf {
        self.root_dir().join(self.state.host().plugin_name.as_str())
    }

    pub fn path(&self, key: &str) -> PathBuf {
        self.plugin_dir().join(sanitize_key(key))
    }

    pub fn read_string(&self, key: &str) -> PluginResult<Option<String>> {
        let path = self.path(key);
        if !path.exists() {
            return Ok(None);
        }
        fs::read_to_string(path)
            .map(Some)
            .map_err(PluginError::from)
    }

    pub fn write_string(&self, key: &str, value: impl AsRef<str>) -> PluginResult<()> {
        let path = self.path(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, value.as_ref()).map_err(PluginError::from)
    }

    pub fn read_json<T>(&self, key: &str) -> PluginResult<Option<T>>
    where
        T: DeserializeOwned,
    {
        match self.read_string(key)? {
            Some(value) => serde_json::from_str::<T>(&value)
                .map(Some)
                .map_err(PluginError::from),
            None => Ok(None),
        }
    }

    pub fn write_json<T>(&self, key: &str, value: &T) -> PluginResult<()>
    where
        T: Serialize,
    {
        self.write_string(key, serde_json::to_string_pretty(value)?)
    }
}

/// Filesystem helper bound to plugin-owned directories.
#[derive(Clone)]
pub struct FsContext {
    state: Arc<RuntimeState>,
}

impl FsContext {
    pub fn plugin_dir(&self) -> PathBuf {
        self.state.host().plugin_dir.clone()
    }

    pub fn config_path(&self) -> &Path {
        &self.state.host().config_path
    }

    pub fn plugin_home(&self) -> PathBuf {
        self.plugin_dir()
            .join(self.state.host().plugin_name.as_str())
    }

    pub fn plugin_data_dir(&self) -> PluginResult<PathBuf> {
        let path = self.plugin_home().join("data");
        fs::create_dir_all(&path)?;
        Ok(path)
    }
}

/// Simple blocking network helper.
#[derive(Clone)]
pub struct NetworkContext {
    client: Client,
}

impl NetworkContext {
    pub fn get(&self, url: impl Into<String>) -> NetworkRequest {
        NetworkRequest {
            client: self.client.clone(),
            url: url.into(),
            query: Vec::new(),
            headers: Vec::new(),
        }
    }
}

/// Fluent blocking request builder used by plugins.
#[derive(Clone)]
pub struct NetworkRequest {
    client: Client,
    url: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
}

impl NetworkRequest {
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push((key.into(), value.into()));
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    pub fn send_text(self) -> PluginResult<String> {
        let mut request = self.client.get(&self.url);
        for (key, value) in self.query {
            request = request.query(&[(key, value)]);
        }
        for (key, value) in self.headers {
            request = request.header(key, value);
        }
        request
            .send()
            .context(format!("GET {}", self.url))?
            .error_for_status()
            .context(format!("GET {}", self.url))?
            .text()
            .map_err(PluginError::from)
    }

    pub fn send_json<T>(self) -> PluginResult<T>
    where
        T: DeserializeOwned,
    {
        let mut request = self.client.get(&self.url);
        for (key, value) in self.query {
            request = request.query(&[(key, value)]);
        }
        for (key, value) in self.headers {
            request = request.header(key, value);
        }
        request
            .send()
            .context(format!("GET {}", self.url))?
            .error_for_status()
            .context(format!("GET {}", self.url))?
            .json()
            .map_err(PluginError::from)
    }
}

fn sanitize_key(key: &str) -> String {
    let mut value = String::with_capacity(key.len() + 5);
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            value.push(ch);
        } else {
            value.push('_');
        }
    }
    if value.ends_with(".json") || value.ends_with(".txt") || value.ends_with(".cache") {
        value
    } else {
        format!("{value}.cache")
    }
}
