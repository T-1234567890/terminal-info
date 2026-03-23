use std::env::VarError;
use std::fmt::{Display, Formatter};
use std::io;

/// Plugin SDK error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginError {
    message: String,
}

/// Standard plugin result type.
pub type PluginResult<T = ()> = Result<T, PluginError>;

impl PluginError {
    /// Construct a new plugin error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Prefix an existing error with higher-level context.
    pub fn context(self, context: impl Display) -> Self {
        Self::new(format!("{context}: {}", self.message))
    }

    /// Borrow the error message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for PluginError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for PluginError {}

impl From<&str> for PluginError {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PluginError {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<io::Error> for PluginError {
    fn from(value: io::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<serde_json::Error> for PluginError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<toml::de::Error> for PluginError {
    fn from(value: toml::de::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<toml::ser::Error> for PluginError {
    fn from(value: toml::ser::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<reqwest::Error> for PluginError {
    fn from(value: reqwest::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<VarError> for PluginError {
    fn from(value: VarError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<std::num::ParseIntError> for PluginError {
    fn from(value: std::num::ParseIntError) -> Self {
        Self::new(value.to_string())
    }
}

impl From<std::num::ParseFloatError> for PluginError {
    fn from(value: std::num::ParseFloatError) -> Self {
        Self::new(value.to_string())
    }
}

/// Convenience trait for attaching context to results.
pub trait ResultExt<T> {
    fn context(self, context: impl Display) -> PluginResult<T>;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: Into<PluginError>,
{
    fn context(self, context: impl Display) -> PluginResult<T> {
        self.map_err(|error| error.into().context(context))
    }
}
