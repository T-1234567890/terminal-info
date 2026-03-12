use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ApiProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub units: Units,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "location",
        alias = "default_city"
    )]
    pub default_city: Option<String>,
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

        toml::from_str(&contents)
            .map_err(|err| format!("Failed to parse config file {}: {err}", path.display()))
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

    pub fn provider_label(&self) -> &'static str {
        match self.provider {
            Some(ApiProvider::OpenWeather) => "openweather",
            None => "open-meteo",
        }
    }

    pub fn masked_api_key(&self) -> Option<String> {
        self.api_key.as_ref().map(|key| {
            if key.len() <= 4 {
                "*".repeat(key.len())
            } else {
                format!("{}{}", "*".repeat(key.len() - 4), &key[key.len() - 4..])
            }
        })
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
