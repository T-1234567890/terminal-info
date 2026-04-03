use std::env;
use std::fs;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{PluginError, PluginResult};

/// Typed plugin configuration accessor.
#[derive(Clone, Debug)]
pub struct ConfigContext {
    values: Value,
}

impl ConfigContext {
    pub(crate) fn load_from_env() -> Self {
        if let Ok(raw) = env::var("TINFO_PLUGIN_CONFIG_JSON") {
            if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                return Self { values: value };
            }
        }

        let path = env::var("TINFO_CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_tinfo_config_path());
        let values = fs::read_to_string(path)
            .ok()
            .and_then(|contents| toml::from_str::<toml::Value>(&contents).ok())
            .and_then(|value| serde_json::to_value(value).ok())
            .unwrap_or(Value::Null);
        Self { values }
    }

    pub(crate) fn from_value(values: Value) -> Self {
        Self { values }
    }

    /// Return the raw JSON value for the provided dotted path.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.lookup(key).cloned()
    }

    /// Read a string value.
    pub fn string(&self, key: &str) -> PluginResult<Option<String>> {
        match self.lookup(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(value)) => Ok(Some(value.clone())),
            Some(other) => Err(type_error(key, "string", other)),
        }
    }

    /// Read a bool value.
    pub fn bool(&self, key: &str) -> PluginResult<Option<bool>> {
        match self.lookup(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(value)) => Ok(Some(*value)),
            Some(other) => Err(type_error(key, "bool", other)),
        }
    }

    /// Read an unsigned integer value.
    pub fn u64(&self, key: &str) -> PluginResult<Option<u64>> {
        match self.lookup(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(value)) => value
                .as_u64()
                .ok_or_else(|| type_error(key, "u64", &Value::Number(value.clone())))
                .map(Some),
            Some(other) => Err(type_error(key, "u64", other)),
        }
    }

    /// Read a signed integer value.
    pub fn i64(&self, key: &str) -> PluginResult<Option<i64>> {
        match self.lookup(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(value)) => value
                .as_i64()
                .ok_or_else(|| type_error(key, "i64", &Value::Number(value.clone())))
                .map(Some),
            Some(other) => Err(type_error(key, "i64", other)),
        }
    }

    /// Read a floating-point value.
    pub fn f64(&self, key: &str) -> PluginResult<Option<f64>> {
        match self.lookup(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(value)) => value
                .as_f64()
                .ok_or_else(|| type_error(key, "f64", &Value::Number(value.clone())))
                .map(Some),
            Some(other) => Err(type_error(key, "f64", other)),
        }
    }

    /// Deserialize a subtree into a Rust type.
    pub fn deserialize<T>(&self, key: &str) -> PluginResult<Option<T>>
    where
        T: DeserializeOwned,
    {
        match self.lookup(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(PluginError::from),
        }
    }

    /// Borrow the full raw config tree.
    pub fn raw(&self) -> &Value {
        &self.values
    }

    fn lookup(&self, key: &str) -> Option<&Value> {
        let mut current = &self.values;
        for segment in key.split('.') {
            current = current.get(segment)?;
        }
        Some(current)
    }
}

fn default_tinfo_config_path() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".tinfo")
        .join("config.toml")
}

fn type_error(key: &str, expected: &str, actual: &Value) -> PluginError {
    PluginError::new(format!(
        "config key '{key}' expected {expected}, got {}",
        value_kind(actual)
    ))
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
