use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
    pub compact_mode: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            widgets: default_dashboard_widgets(),
            refresh_interval: default_dashboard_refresh_interval(),
            compact_mode: false,
        }
    }
}

fn default_dashboard_widgets() -> Vec<String> {
    vec![
        "weather".to_string(),
        "time".to_string(),
        "network".to_string(),
        "system".to_string(),
        "plugins".to_string(),
    ]
}

fn default_dashboard_refresh_interval() -> u64 {
    1
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

fn default_weather_cache_ttl() -> u64 {
    60
}

fn default_network_cache_ttl() -> u64 {
    30
}

fn default_time_cache_ttl() -> u64 {
    10
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
    #[serde(default)]
    pub server_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ApiProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub units: Units,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profile: BTreeMap<String, ProfileConfig>,
    #[serde(default)]
    pub dashboard: DashboardConfig,
    #[serde(default)]
    pub cache: CacheConfig,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            server_mode: false,
            provider: None,
            api_key: None,
            units: Units::default(),
            active_profile: None,
            profile: BTreeMap::new(),
            dashboard: DashboardConfig::default(),
            cache: CacheConfig::default(),
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
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read config file {}: {err}", path.display()))?;

        if contents.trim().is_empty() {
            let config = Self::default();
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
        if self.cache.weather_ttl_secs == 0 {
            self.cache.weather_ttl_secs = default_weather_cache_ttl();
        }
        if self.cache.network_ttl_secs == 0 {
            self.cache.network_ttl_secs = default_network_cache_ttl();
        }
        if self.cache.time_ttl_secs == 0 {
            self.cache.time_ttl_secs = default_time_cache_ttl();
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

    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".tinfo").join("config.toml"))
}

pub fn legacy_json_config_path() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".tw").join("config.json"))
}

pub fn plugin_dir_path() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("TINFO_PLUGIN_DIR") {
        return Ok(PathBuf::from(dir));
    }

    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".terminal-info").join("plugins"))
}

pub fn legacy_plugin_dir_path() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".tinfo").join("plugins"))
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
        assert_eq!(config.cache.weather_ttl_secs, 60);
        assert_eq!(config.cache.network_ttl_secs, 30);
        assert_eq!(config.cache.time_ttl_secs, 10);
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
