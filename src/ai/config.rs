use std::path::{Path, PathBuf};

use crate::ai::adapters::{AgentAdapterKind, ConfiguredAgent};
use crate::ai::chat::ProviderKind;
use crate::ai::secret::{SecretStore, SystemSecretStore};
use crate::config::{
    AiAgentCliConfig, AiApprovalMode, AiProviderConfig, AiSettings, Config, config_dir,
    config_path as shared_config_path,
};

/// Shared AI configuration view backed by the main `~/.tinfo/config.toml`.
///
/// The AI control layer is part of core `tinfo`, so it reads and writes
/// through the same config file as the main CLI.
#[derive(Debug, Clone)]
pub struct AiConfig {
    shared: AiSettings,
    config_path: PathBuf,
}

impl AiConfig {
    pub fn load_default() -> Self {
        let config_path =
            shared_config_path().unwrap_or_else(|_| PathBuf::from("~/.tinfo/config.toml"));
        let shared = Config::load_or_create()
            .map(|config| config.ai)
            .unwrap_or_else(|_| AiSettings::default());

        Self {
            shared,
            config_path,
        }
    }

    pub fn save_provider_api_key(
        provider: ProviderKind,
        api_key: String,
    ) -> Result<Self, String> {
        let mut config = Config::load_or_create()?;
        SystemSecretStore.save_provider_key(provider, &api_key)?;
        config.ai.default_provider = Some(provider.config_key().to_string());
        clear_provider_api_key(&mut config, provider);
        config.save()?;
        let loaded = Self::load_default();
        if loaded.load_provider_api_key(provider)?.is_none() {
            return Err(format!(
                "Saved {} API key, but secure storage could not read it back. Check your OS keychain permissions and try again.",
                provider.display_name()
            ));
        }
        Ok(loaded)
    }

    pub fn save_default_provider(provider: ProviderKind) -> Result<Self, String> {
        let mut config = Config::load_or_create()?;
        config.ai.default_provider = Some(provider.config_key().to_string());
        config.save()?;
        Ok(Self::load_default())
    }

    pub fn save_default_model(provider: ProviderKind, model: String) -> Result<Self, String> {
        let mut config = Config::load_or_create()?;
        match provider {
            ProviderKind::OpenAi => config.ai.providers.openai.default_model = model,
            ProviderKind::Anthropic => config.ai.providers.anthropic.default_model = model,
            ProviderKind::OpenRouter => config.ai.providers.openrouter.default_model = model,
        }
        config.save()?;
        Ok(Self::load_default())
    }

    pub fn system_prompt(&self) -> Option<&str> {
        self.shared.system_prompt.as_deref()
    }

    pub fn provider_label(&self) -> String {
        let provider = self
            .configured_default_provider()
            .unwrap_or_else(|| self.default_chat_provider());
        format!("{}/{}", provider.label(), self.default_model(provider))
    }

    pub fn default_chat_provider(&self) -> ProviderKind {
        if let Some(provider) = self.configured_default_provider() {
            provider
        } else if self.provider_has_api_key(ProviderKind::OpenAi) {
            ProviderKind::OpenAi
        } else if self.provider_has_api_key(ProviderKind::Anthropic) {
            ProviderKind::Anthropic
        } else if self.provider_has_api_key(ProviderKind::OpenRouter) {
            ProviderKind::OpenRouter
        } else {
            ProviderKind::OpenAi
        }
    }

    pub fn default_model(&self, provider: ProviderKind) -> &str {
        self.provider_config(provider).default_model.as_str()
    }

    pub fn provider_config(&self, provider: ProviderKind) -> &AiProviderConfig {
        match provider {
            ProviderKind::OpenAi => &self.shared.providers.openai,
            ProviderKind::Anthropic => &self.shared.providers.anthropic,
            ProviderKind::OpenRouter => &self.shared.providers.openrouter,
        }
    }

    pub fn any_provider_configured(&self) -> bool {
        self.provider_has_api_key(ProviderKind::OpenAi)
            || self.provider_has_api_key(ProviderKind::Anthropic)
            || self.provider_has_api_key(ProviderKind::OpenRouter)
    }

    pub fn configured_default_provider(&self) -> Option<ProviderKind> {
        self.shared
            .default_provider
            .as_deref()
            .map(ProviderKind::from_label)
    }

    pub fn load_provider_api_key(&self, provider: ProviderKind) -> Result<Option<String>, String> {
        if let Some(value) = SystemSecretStore.load_provider_key(provider)? {
            return Ok(Some(value));
        }
        Ok(self.provider_config(provider).api_key.clone())
    }

    pub fn provider_has_api_key(&self, provider: ProviderKind) -> bool {
        self.load_provider_api_key(provider)
            .map(|value| value.is_some())
            .unwrap_or_else(|_| self.provider_config(provider).api_key.is_some())
    }

    pub fn approval_mode(&self) -> AiApprovalMode {
        self.shared.agent.approval_mode
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn api_bind(&self) -> &str {
        &self.shared.api.bind
    }

    pub fn web_enabled(&self) -> bool {
        self.shared.ui.web_enabled
    }

    pub fn ui_refresh_ms(&self) -> u64 {
        self.shared.ui.refresh_ms
    }

    pub fn event_buffer_size(&self) -> usize {
        self.shared.runtime.event_buffer_size
    }

    pub fn log_buffer_size(&self) -> usize {
        self.shared.runtime.log_buffer_size
    }

    pub fn auto_reject_timeout_secs(&self) -> Option<u64> {
        self.shared.runtime.auto_reject_timeout_secs
    }

    pub fn chat_history_enabled(&self) -> bool {
        self.shared.runtime.chat_history
    }

    pub fn chat_context_enabled(&self) -> bool {
        self.shared.runtime.chat_context
    }

    pub fn persist_chat_transcripts(&self) -> bool {
        self.shared.runtime.persist_chat_transcripts
    }

    pub fn data_dir(&self) -> PathBuf {
        config_dir()
            .unwrap_or_else(|_| PathBuf::from(".tinfo"))
            .join("ai")
    }

    pub fn agents(&self) -> Vec<ConfiguredAgent> {
        let mut agents = Vec::new();
        let adapters = [
            ("codex", AgentAdapterKind::Codex, &self.shared.adapters.codex),
            (
                "claude_code",
                AgentAdapterKind::ClaudeCode,
                &self.shared.adapters.claude_code,
            ),
            ("gemini", AgentAdapterKind::Gemini, &self.shared.adapters.gemini),
        ];

        for (id, kind, config) in adapters {
            if config.enabled && !effective_command(kind, config).trim().is_empty() {
                let mut config = config.clone();
                if config.command.trim().is_empty() {
                    config.command = default_command(kind).to_string();
                }
                agents.push(ConfiguredAgent {
                    id: id.to_string(),
                    adapter: kind,
                    config,
                });
            }
        }

        for (name, config) in &self.shared.agents {
            if !config.enabled {
                continue;
            }
            let mut config = config.clone();
            let adapter = AgentAdapterKind::from_config(config.adapter.as_deref());
            if config.command.trim().is_empty() {
                config.command = default_command(adapter).to_string();
            }
            agents.push(ConfiguredAgent {
                id: name.clone(),
                adapter,
                config,
            });
        }

        agents
    }

    pub fn shared(&self) -> &AiSettings {
        &self.shared
    }
}

fn effective_command(kind: AgentAdapterKind, config: &AiAgentCliConfig) -> &str {
    if config.command.trim().is_empty() {
        default_command(kind)
    } else {
        config.command.as_str()
    }
}

fn default_command(kind: AgentAdapterKind) -> &'static str {
    match kind {
        AgentAdapterKind::Codex => "codex",
        AgentAdapterKind::ClaudeCode => "claude",
        AgentAdapterKind::Gemini => "gemini",
        AgentAdapterKind::Generic => "",
    }
}

fn clear_provider_api_key(config: &mut Config, provider: ProviderKind) {
    match provider {
        ProviderKind::OpenAi => config.ai.providers.openai.api_key = None,
        ProviderKind::Anthropic => config.ai.providers.anthropic.api_key = None,
        ProviderKind::OpenRouter => config.ai.providers.openrouter.api_key = None,
    }
}
