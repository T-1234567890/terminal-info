use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::output::OutputMode;
use crate::theme::ThemeConfig;

pub const CURRENT_CONFIG_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiProvider {
    OpenWeather,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    #[default]
    Metric,
    Imperial,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DefaultOutput {
    Plain,
    Compact,
    #[default]
    Color,
}

impl DefaultOutput {
    pub fn label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Compact => "compact",
            Self::Color => "color",
        }
    }

    pub fn as_output_mode(self) -> OutputMode {
        match self {
            Self::Plain => OutputMode::Plain,
            Self::Compact => OutputMode::Compact,
            Self::Color => OutputMode::Color,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProfileConfig {
    #[serde(skip_serializing_if = "Option::is_none", rename = "location")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<Units>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ApiProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<DashboardConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DashboardConfig {
    #[serde(default = "default_dashboard_widgets")]
    pub widgets: Vec<String>,
    #[serde(default = "default_dashboard_refresh_interval")]
    pub refresh_interval: u64,
    #[serde(default)]
    pub layout: DashboardLayout,
    #[serde(default)]
    pub columns: Option<usize>,
    #[serde(default)]
    pub compact_mode: bool,
    #[serde(default)]
    pub freeze: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DashboardLayout {
    Vertical,
    Horizontal,
    #[default]
    Auto,
}

impl DashboardLayout {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
            Self::Auto => "auto",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskSortOrder {
    #[default]
    Created,
    Status,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TasksConfig {
    #[serde(default = "default_tasks_show_completed")]
    pub show_completed: bool,
    #[serde(default)]
    pub sort_order: TaskSortOrder,
    #[serde(default = "default_tasks_max_display")]
    pub max_display: usize,
    #[serde(default)]
    pub auto_remove_completed: bool,
}

impl Default for TasksConfig {
    fn default() -> Self {
        Self {
            show_completed: default_tasks_show_completed(),
            sort_order: TaskSortOrder::default(),
            max_display: default_tasks_max_display(),
            auto_remove_completed: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NotesConfig {
    #[serde(default = "default_notes_max_stored")]
    pub max_stored: usize,
    #[serde(default = "default_true")]
    pub show_in_widget: bool,
}

impl Default for NotesConfig {
    fn default() -> Self {
        Self {
            max_stored: default_notes_max_stored(),
            show_in_widget: default_true(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TimerConfig {
    #[serde(default = "default_timer_duration")]
    pub default_duration: String,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub show_in_widget: bool,
    #[serde(default = "default_true")]
    pub hide_when_complete: bool,
    #[serde(default)]
    pub mode: TimerWidgetMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TimerWidgetMode {
    #[default]
    Full,
    Compact,
}

impl TimerWidgetMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
        }
    }
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            default_duration: default_timer_duration(),
            auto_start: false,
            show_in_widget: default_true(),
            hide_when_complete: default_true(),
            mode: TimerWidgetMode::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemindersConfig {
    #[serde(default = "default_reminder_duration")]
    pub default_duration: String,
    #[serde(default = "default_true")]
    pub enable_notifications: bool,
    #[serde(default = "default_true")]
    pub sound_alert: bool,
    #[serde(default = "default_true")]
    pub visual_alert: bool,
}

impl Default for RemindersConfig {
    fn default() -> Self {
        Self {
            default_duration: default_reminder_duration(),
            enable_notifications: default_true(),
            sound_alert: default_true(),
            visual_alert: default_true(),
        }
    }
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            widgets: default_dashboard_widgets(),
            refresh_interval: default_dashboard_refresh_interval(),
            layout: DashboardLayout::default(),
            columns: None,
            compact_mode: false,
            freeze: false,
        }
    }
}

fn default_dashboard_widgets() -> Vec<String> {
    vec![
        "weather".to_string(),
        "time".to_string(),
        "network".to_string(),
        "system".to_string(),
        "timer".to_string(),
        "tasks".to_string(),
        "notes".to_string(),
        "history".to_string(),
        "reminders".to_string(),
        "plugins".to_string(),
    ]
}

fn default_dashboard_refresh_interval() -> u64 {
    1
}

fn default_true() -> bool {
    true
}

fn default_tasks_show_completed() -> bool {
    true
}

fn default_tasks_max_display() -> usize {
    5
}

fn default_notes_max_stored() -> usize {
    50
}

fn default_timer_duration() -> String {
    "25m".to_string()
}

fn default_reminder_duration() -> String {
    "15m".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CacheConfig {
    #[serde(default = "default_weather_cache_ttl")]
    pub weather_ttl_secs: u64,
    #[serde(default = "default_network_cache_ttl")]
    pub network_ttl_secs: u64,
    #[serde(default = "default_time_cache_ttl")]
    pub time_ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            weather_ttl_secs: default_weather_cache_ttl(),
            network_ttl_secs: default_network_cache_ttl(),
            time_ttl_secs: default_time_cache_ttl(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default = "default_ai_model")]
    pub default_model: String,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            endpoint: None,
            default_model: default_ai_model(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiProvidersConfig {
    #[serde(default)]
    pub openai: AiProviderConfig,
    #[serde(default)]
    pub anthropic: AiProviderConfig,
    #[serde(default)]
    pub openrouter: AiProviderConfig,
}

impl Default for AiProvidersConfig {
    fn default() -> Self {
        Self {
            openai: AiProviderConfig::default(),
            anthropic: AiProviderConfig::default(),
            openrouter: AiProviderConfig::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AiApprovalMode {
    #[default]
    Manual,
    Auto,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiAgentSettings {
    #[serde(default)]
    pub approval_mode: AiApprovalMode,
    #[serde(default = "default_true")]
    pub audit_log: bool,
    #[serde(default = "default_true")]
    pub compact_activity: bool,
}

impl Default for AiAgentSettings {
    fn default() -> Self {
        Self {
            approval_mode: AiApprovalMode::default(),
            audit_log: true,
            compact_activity: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiRuntimeSettings {
    #[serde(default = "default_ai_event_buffer_size")]
    pub event_buffer_size: usize,
    #[serde(default = "default_ai_log_buffer_size")]
    pub log_buffer_size: usize,
    #[serde(default)]
    pub auto_reject_timeout_secs: Option<u64>,
    #[serde(default = "default_true")]
    pub chat_history: bool,
    #[serde(default = "default_true")]
    pub chat_context: bool,
    #[serde(default)]
    pub persist_chat_transcripts: bool,
}

impl Default for AiRuntimeSettings {
    fn default() -> Self {
        Self {
            event_buffer_size: default_ai_event_buffer_size(),
            log_buffer_size: default_ai_log_buffer_size(),
            auto_reject_timeout_secs: None,
            chat_history: true,
            chat_context: true,
            persist_chat_transcripts: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiApiSettings {
    #[serde(default = "default_ai_api_bind")]
    pub bind: String,
}

impl Default for AiApiSettings {
    fn default() -> Self {
        Self {
            bind: default_ai_api_bind(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiUiSettings {
    #[serde(default = "default_true")]
    pub web_enabled: bool,
    #[serde(default = "default_ai_ui_refresh_ms")]
    pub refresh_ms: u64,
    #[serde(default = "default_ai_default_view")]
    pub default_view: String,
    #[serde(default = "default_true")]
    pub remember_last_view: bool,
    #[serde(default = "default_true")]
    pub show_tips: bool,
}

impl Default for AiUiSettings {
    fn default() -> Self {
        Self {
            web_enabled: true,
            refresh_ms: default_ai_ui_refresh_ms(),
            default_view: default_ai_default_view(),
            remember_last_view: true,
            show_tips: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiAgentCliConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Default for AiAgentCliConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_start: false,
            adapter: None,
            command: String::new(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiAdaptersConfig {
    #[serde(default = "default_codex_adapter")]
    pub codex: AiAgentCliConfig,
    #[serde(default = "default_claude_adapter")]
    pub claude_code: AiAgentCliConfig,
    #[serde(default = "default_gemini_adapter")]
    pub gemini: AiAgentCliConfig,
}

impl Default for AiAdaptersConfig {
    fn default() -> Self {
        Self {
            codex: default_codex_adapter(),
            claude_code: default_claude_adapter(),
            gemini: default_gemini_adapter(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub providers: AiProvidersConfig,
    #[serde(default)]
    pub agent: AiAgentSettings,
    #[serde(default)]
    pub runtime: AiRuntimeSettings,
    #[serde(default)]
    pub api: AiApiSettings,
    #[serde(default)]
    pub ui: AiUiSettings,
    #[serde(default)]
    pub adapters: AiAdaptersConfig,
    #[serde(default)]
    pub agents: BTreeMap<String, AiAgentCliConfig>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            default_provider: None,
            system_prompt: None,
            providers: AiProvidersConfig::default(),
            agent: AiAgentSettings::default(),
            runtime: AiRuntimeSettings::default(),
            api: AiApiSettings::default(),
            ui: AiUiSettings::default(),
            adapters: AiAdaptersConfig::default(),
            agents: BTreeMap::new(),
        }
    }
}

fn default_weather_cache_ttl() -> u64 {
    60
}

fn default_network_cache_ttl() -> u64 {
    30
}

fn default_time_cache_ttl() -> u64 {
    10
}

fn default_ai_model() -> String {
    "default".to_string()
}

fn default_ai_event_buffer_size() -> usize {
    256
}

fn default_ai_log_buffer_size() -> usize {
    200
}

fn default_ai_api_bind() -> String {
    "127.0.0.1:7878".to_string()
}

fn default_ai_ui_refresh_ms() -> u64 {
    1000
}

fn default_ai_default_view() -> String {
    "agent".to_string()
}

fn default_disabled_ai_adapter(command: &str, adapter: &str) -> AiAgentCliConfig {
    AiAgentCliConfig {
        enabled: false,
        auto_start: false,
        adapter: Some(adapter.to_string()),
        command: command.to_string(),
        args: Vec::new(),
        cwd: None,
        env: BTreeMap::new(),
    }
}

fn default_codex_adapter() -> AiAgentCliConfig {
    default_disabled_ai_adapter("codex", "codex")
}

fn default_claude_adapter() -> AiAgentCliConfig {
    default_disabled_ai_adapter("claude", "claude_code")
}

fn default_gemini_adapter() -> AiAgentCliConfig {
    default_disabled_ai_adapter("gemini", "gemini")
}

impl Units {
    pub fn label(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }

    pub fn temperature_symbol(self) -> &'static str {
        match self {
            Self::Metric => "°C",
            Self::Imperial => "°F",
        }
    }

    pub fn wind_speed_unit(self) -> &'static str {
        match self {
            Self::Metric => "m/s",
            Self::Imperial => "mph",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_config_version")]
    pub config_version: u32,
    #[serde(default = "default_setup_complete")]
    pub setup_complete: bool,
    #[serde(default)]
    pub server_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ApiProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub units: Units,
    #[serde(default)]
    pub default_output: DefaultOutput,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profile: BTreeMap<String, ProfileConfig>,
    #[serde(default)]
    pub dashboard: DashboardConfig,
    #[serde(default)]
    pub tasks: TasksConfig,
    #[serde(default)]
    pub notes: NotesConfig,
    #[serde(default, alias = "timers")]
    pub timer: TimerConfig,
    #[serde(default)]
    pub reminders: RemindersConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub ai: AiSettings,
    #[serde(default)]
    pub locations: BTreeMap<String, String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "location",
        alias = "default_city"
    )]
    pub default_city: Option<String>,
}

fn default_config_version() -> u32 {
    CURRENT_CONFIG_VERSION
}

fn default_setup_complete() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            setup_complete: true,
            server_mode: false,
            provider: None,
            api_key: None,
            units: Units::default(),
            default_output: DefaultOutput::default(),
            theme: ThemeConfig::default(),
            active_profile: None,
            profile: BTreeMap::new(),
            dashboard: DashboardConfig::default(),
            tasks: TasksConfig::default(),
            notes: NotesConfig::default(),
            timer: TimerConfig::default(),
            reminders: RemindersConfig::default(),
            cache: CacheConfig::default(),
            ai: AiSettings::default(),
            locations: BTreeMap::new(),
            default_city: None,
        }
    }
}

impl Config {
    pub fn load_or_create() -> Result<Self, String> {
        let path = config_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Failed to create config directory: {err}"))?;
        }

        if !path.exists() {
            let mut config = Self::default();
            config.setup_complete = false;
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read config file {}: {err}", path.display()))?;

        if contents.trim().is_empty() {
            let mut config = Self::default();
            config.setup_complete = false;
            config.save()?;
            return Ok(config);
        }

        let mut config: Self = toml::from_str(&contents)
            .map_err(|err| format!("Failed to parse config file {}: {err}", path.display()))?;
        config.ensure_current_version();
        Ok(config)
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("Failed to create config directory: {err}"))?;
        }

        let toml = toml::to_string_pretty(self)
            .map_err(|err| format!("Failed to serialize config: {err}"))?;

        fs::write(&path, format!("{toml}\n"))
            .map_err(|err| format!("Failed to write config file {}: {err}", path.display()))
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn ensure_current_version(&mut self) {
        if self.config_version == 0 {
            self.config_version = CURRENT_CONFIG_VERSION;
        }
        if self.dashboard.refresh_interval == 0 {
            self.dashboard.refresh_interval = default_dashboard_refresh_interval();
        }
        if matches!(self.dashboard.columns, Some(0)) {
            self.dashboard.columns = None;
        }
        if self.cache.weather_ttl_secs == 0 {
            self.cache.weather_ttl_secs = default_weather_cache_ttl();
        }
        if self.cache.network_ttl_secs == 0 {
            self.cache.network_ttl_secs = default_network_cache_ttl();
        }
        if self.cache.time_ttl_secs == 0 {
            self.cache.time_ttl_secs = default_time_cache_ttl();
        }
        if self.ai.runtime.event_buffer_size == 0 {
            self.ai.runtime.event_buffer_size = default_ai_event_buffer_size();
        }
        if self.ai.runtime.log_buffer_size == 0 {
            self.ai.runtime.log_buffer_size = default_ai_log_buffer_size();
        }
        if self.ai.api.bind.trim().is_empty() {
            self.ai.api.bind = default_ai_api_bind();
        }
        if self.ai.ui.refresh_ms == 0 {
            self.ai.ui.refresh_ms = default_ai_ui_refresh_ms();
        }
        if self.ai.adapters.codex.command.trim().is_empty() {
            self.ai.adapters.codex = default_codex_adapter();
        }
        if self.ai.adapters.claude_code.command.trim().is_empty() {
            self.ai.adapters.claude_code = default_claude_adapter();
        }
        if self.ai.adapters.gemini.command.trim().is_empty() {
            self.ai.adapters.gemini = default_gemini_adapter();
        }
        if self.tasks.max_display == 0 {
            self.tasks.max_display = default_tasks_max_display();
        }
        if self.notes.max_stored == 0 {
            self.notes.max_stored = default_notes_max_stored();
        }
        if self.timer.default_duration.trim().is_empty() {
            self.timer.default_duration = default_timer_duration();
        }
        if self.reminders.default_duration.trim().is_empty() {
            self.reminders.default_duration = default_reminder_duration();
        }
    }

    pub fn apply_profile(&mut self, name: &str) -> Result<(), String> {
        if !self.profile.contains_key(name) {
            return Err(format!("Profile '{}' not found.", name));
        }
        self.active_profile = Some(name.to_string());
        Ok(())
    }

    pub fn active_profile_config(&self) -> Option<&ProfileConfig> {
        self.active_profile
            .as_deref()
            .and_then(|name| self.profile.get(name))
    }

    pub fn add_profile_from_current(&mut self, name: &str) -> Result<(), String> {
        if self.profile.contains_key(name) {
            return Err(format!("Profile '{}' already exists.", name));
        }
        let profile = ProfileConfig {
            location: self.effective_location().map(str::to_string),
            units: Some(self.effective_units()),
            provider: self.effective_provider(),
            api_key: self.effective_api_key().map(str::to_string),
            dashboard: Some(self.effective_dashboard()),
        };
        self.profile.insert(name.to_string(), profile);
        Ok(())
    }

    pub fn remove_profile(&mut self, name: &str) -> Result<(), String> {
        if self.profile.remove(name).is_none() {
            return Err(format!("Profile '{}' not found.", name));
        }
        if self.active_profile.as_deref() == Some(name) {
            self.active_profile = None;
        }
        Ok(())
    }

    pub fn profile_named(&self, name: &str) -> Option<&ProfileConfig> {
        self.profile.get(name)
    }

    pub fn effective_dashboard(&self) -> DashboardConfig {
        self.active_profile_config()
            .and_then(|profile| profile.dashboard.clone())
            .unwrap_or_else(|| self.dashboard.clone())
    }

    pub fn effective_units(&self) -> Units {
        self.active_profile_config()
            .and_then(|profile| profile.units)
            .unwrap_or(self.units)
    }

    pub fn effective_provider(&self) -> Option<ApiProvider> {
        self.active_profile_config()
            .and_then(|profile| profile.provider)
            .or(self.provider)
    }

    pub fn effective_api_key(&self) -> Option<&str> {
        self.active_profile_config()
            .and_then(|profile| profile.api_key.as_deref())
            .or(self.api_key.as_deref())
    }

    pub fn effective_location(&self) -> Option<&str> {
        self.active_profile_config()
            .and_then(|profile| profile.location.as_deref())
            .or(self.default_city.as_deref())
    }

    pub fn provider_label(&self) -> &'static str {
        match self.effective_provider() {
            Some(ApiProvider::OpenWeather) => "openweather",
            None => "open-meteo",
        }
    }

    pub fn masked_api_key(&self) -> Option<String> {
        self.effective_api_key().map(|key| {
            if key.len() <= 4 {
                "*".repeat(key.len())
            } else {
                format!("{}{}", "*".repeat(key.len() - 4), &key[key.len() - 4..])
            }
        })
    }

    pub fn configured_location(&self) -> Option<&str> {
        self.effective_location()
            .filter(|city| !city.eq_ignore_ascii_case("auto"))
    }

    pub fn resolve_location_alias<'a>(&'a self, value: &'a str) -> &'a str {
        self.locations
            .get(value)
            .map(String::as_str)
            .unwrap_or(value)
    }

    pub fn uses_auto_location(&self) -> bool {
        self.effective_location()
            .map(|city| city.eq_ignore_ascii_case("auto"))
            .unwrap_or(false)
    }
}

pub fn config_path() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_CONFIG_DIR") {
        return Ok(PathBuf::from(dir).join("config.toml"));
    }

    if let Ok(dir) = env::var("TW_CONFIG_DIR") {
        return Ok(PathBuf::from(dir).join("config.toml"));
    }

    Ok(config_dir()?.join("config.toml"))
}

pub fn home_dir_path() -> PathBuf {
    if let Some(path) = dirs::home_dir() {
        return path;
    }

    if let Ok(path) = env::var("USERPROFILE") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(path) = env::var("HOME") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    env::current_dir().unwrap_or_else(|_| env::temp_dir())
}

pub fn config_dir() -> Result<PathBuf, String> {
    Ok(home_dir_path().join(".tinfo"))
}

pub fn data_dir_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("data"))
}

pub fn legacy_json_config_path() -> Result<PathBuf, String> {
    Ok(home_dir_path().join(".tw").join("config.json"))
}

pub fn plugin_dir_path() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_PLUGIN_DIR") {
        return Ok(PathBuf::from(dir));
    }

    Ok(home_dir_path().join(".terminal-info").join("plugins"))
}

pub fn legacy_plugin_dir_path() -> Result<PathBuf, String> {
    Ok(home_dir_path().join(".tinfo").join("plugins"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_current_version() {
        let config = Config::default();
        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert!(!config.server_mode);
        assert_eq!(config.dashboard.refresh_interval, 1);
        assert_eq!(config.dashboard.layout, DashboardLayout::Auto);
        assert_eq!(config.cache.weather_ttl_secs, 60);
        assert_eq!(config.cache.network_ttl_secs, 30);
        assert_eq!(config.cache.time_ttl_secs, 10);
        assert_eq!(config.theme, ThemeConfig::default());
    }

    #[test]
    fn ensure_current_version_populates_missing_defaults() {
        let mut config = Config {
            config_version: 0,
            ..Config::default()
        };
        config.dashboard.refresh_interval = 0;
        config.cache.weather_ttl_secs = 0;
        config.cache.network_ttl_secs = 0;
        config.cache.time_ttl_secs = 0;
        config.ensure_current_version();
        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
        assert_eq!(config.dashboard.refresh_interval, 1);
        assert_eq!(config.cache.weather_ttl_secs, 60);
        assert_eq!(config.cache.network_ttl_secs, 30);
        assert_eq!(config.cache.time_ttl_secs, 10);
    }

    #[test]
    fn profile_add_and_use_preserves_base_fallback() {
        let mut config = Config::default();
        config.default_city = Some("Shenzhen".to_string());
        config.dashboard.widgets = vec!["weather".to_string(), "time".to_string()];
        config.add_profile_from_current("home").unwrap();

        config.default_city = Some("Tokyo".to_string());
        config.dashboard.widgets = vec!["network".to_string()];

        config.apply_profile("home").unwrap();
        assert_eq!(config.configured_location(), Some("Shenzhen"));
        assert_eq!(
            config.effective_dashboard().widgets,
            vec!["weather".to_string(), "time".to_string()]
        );
        assert_eq!(config.default_city.as_deref(), Some("Tokyo"));
    }

    #[test]
    fn removing_active_profile_falls_back_to_base_config() {
        let mut config = Config::default();
        config.default_city = Some("Tokyo".to_string());
        config.add_profile_from_current("office").unwrap();
        config.apply_profile("office").unwrap();
        config.remove_profile("office").unwrap();
        assert_eq!(config.active_profile, None);
        assert_eq!(config.configured_location(), Some("Tokyo"));
    }
}
