use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMode {
    Plain,
    Compact,
    Color,
}

static OUTPUT_MODE: OnceLock<OutputMode> = OnceLock::new();

pub fn set_output_mode(mode: OutputMode) {
    let _ = OUTPUT_MODE.set(mode);
}

pub fn output_mode() -> OutputMode {
    *OUTPUT_MODE.get().unwrap_or(&OutputMode::Color)
}

pub fn success_prefix() -> &'static str {
    match output_mode() {
        OutputMode::Plain | OutputMode::Compact => "[OK]",
        OutputMode::Color => "✔",
    }
}

pub fn error_prefix() -> &'static str {
    match output_mode() {
        OutputMode::Plain | OutputMode::Compact => "[ERR]",
        OutputMode::Color => "✖",
    }
}
