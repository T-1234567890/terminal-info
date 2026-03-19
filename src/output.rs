use std::sync::OnceLock;

use crate::theme::theme_config;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMode {
    Plain,
    Compact,
    Color,
}

static OUTPUT_MODE: OnceLock<OutputMode> = OnceLock::new();
static JSON_OUTPUT: OnceLock<bool> = OnceLock::new();

pub fn set_output_mode(mode: OutputMode) {
    let _ = OUTPUT_MODE.set(mode);
}

pub fn output_mode() -> OutputMode {
    *OUTPUT_MODE.get().unwrap_or(&OutputMode::Color)
}

pub fn set_json_output(enabled: bool) {
    let _ = JSON_OUTPUT.set(enabled);
}

pub fn json_output() -> bool {
    *JSON_OUTPUT.get().unwrap_or(&false)
}

pub fn success_prefix() -> &'static str {
    match (output_mode(), theme_config().ascii_only) {
        (_, true) => "[OK]",
        (OutputMode::Plain, false) | (OutputMode::Compact, false) => "[OK]",
        (OutputMode::Color, false) => "✔",
    }
}

pub fn error_prefix() -> &'static str {
    match (output_mode(), theme_config().ascii_only) {
        (_, true) => "[ERR]",
        (OutputMode::Plain, false) | (OutputMode::Compact, false) => "[ERR]",
        (OutputMode::Color, false) => "✖",
    }
}

pub fn warn_prefix() -> &'static str {
    match (output_mode(), theme_config().ascii_only) {
        (_, true) => "[WARN]",
        (OutputMode::Plain, false) | (OutputMode::Compact, false) => "[WARN]",
        (OutputMode::Color, false) => "▲",
    }
}
