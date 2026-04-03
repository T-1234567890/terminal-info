//! Test harness for plugin authors.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::context::{RuntimeHost, RuntimeState};
use crate::{ConfigContext, Plugin, PluginResult};

/// Configurable fake host state for unit and integration tests.
#[derive(Clone, Debug)]
pub struct MockHost {
    version: String,
    plugin_dir: PathBuf,
    cache_dir: PathBuf,
    config_path: PathBuf,
    config: Value,
}

impl Default for MockHost {
    fn default() -> Self {
        Self {
            version: "0.9.0".to_string(),
            plugin_dir: PathBuf::from("/tmp/tinfo-test/plugins"),
            cache_dir: PathBuf::from("/tmp/tinfo-test/cache"),
            config_path: PathBuf::from("/tmp/tinfo-test/config.toml"),
            config: Value::Null,
        }
    }
}

impl MockHost {
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn plugin_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.plugin_dir = path.into();
        self
    }

    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = path.into();
        self
    }

    pub fn config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = path.into();
        self
    }

    pub fn config_json(mut self, config: Value) -> Self {
        self.config = config;
        self
    }
}

/// Captured test execution result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestRun {
    pub stdout: String,
    pub stderr: String,
}

/// Plugin test runner that executes handlers in-process.
pub struct TestRunner {
    plugin: Plugin,
    host: MockHost,
    args: Vec<String>,
}

impl TestRunner {
    pub fn new(plugin: Plugin) -> Self {
        Self {
            plugin,
            host: MockHost::default(),
            args: Vec::new(),
        }
    }

    pub fn host(mut self, host: MockHost) -> Self {
        self.host = host;
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn run(self) -> PluginResult<TestRun> {
        let host = RuntimeHost {
            plugin_name: "plugin".to_string(),
            host_version: self.host.version,
            plugin_dir: self.host.plugin_dir,
            cache_dir: self.host.cache_dir,
            config_path: self.host.config_path,
        };
        let state = Arc::new(RuntimeState::buffered(host));
        let config = ConfigContext::from_value(self.host.config);
        self.plugin
            .execute_with_state(state.clone(), config, self.args)?;
        Ok(TestRun {
            stdout: state.stdout_contents().unwrap_or_default(),
            stderr: state.stderr_contents().unwrap_or_default(),
        })
    }
}
