use std::fmt::Display;
use std::sync::Arc;

use serde::Serialize;

use crate::context::RuntimeState;
use crate::PluginResult;

/// Structured output status levels shared by the SDK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusLevel {
    Ok,
    Info,
    Warn,
    Error,
    Running,
}

impl Display for StatusLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => f.write_str("ok"),
            Self::Info => f.write_str("info"),
            Self::Warn => f.write_str("warn"),
            Self::Error => f.write_str("error"),
            Self::Running => f.write_str("running"),
        }
    }
}

/// Shared plugin output surface.
#[derive(Clone)]
pub struct Output {
    state: Arc<RuntimeState>,
}

impl Output {
    pub(crate) fn new(state: Arc<RuntimeState>) -> Self {
        Self { state }
    }

    pub fn section(&self, title: impl Display) {
        let title = title.to_string();
        self.state.write_stdout(&title);
        self.state.write_stdout(&"-".repeat(title.len().max(3)));
    }

    pub fn message(&self, message: impl Display) {
        self.state.write_stdout(&message.to_string());
    }

    pub fn kv(&self, key: impl Display, value: impl Display) {
        self.state.write_stdout(&format!("{key}: {value}"));
    }

    pub fn list<I, T>(&self, items: I)
    where
        I: IntoIterator<Item = T>,
        T: Display,
    {
        for item in items {
            self.state.write_stdout(&format!("- {item}"));
        }
    }

    pub fn warning(&self, message: impl Display) {
        self.state.write_stdout(&format!("WARNING: {message}"));
    }

    pub fn error(&self, message: impl Display) {
        self.state.write_stdout(&format!("ERROR: {message}"));
    }

    pub fn status(&self, level: StatusLevel, message: impl Display) {
        self.state.write_stdout(&format!("[{level}] {message}"));
    }

    pub fn progress(&self, message: impl Display) {
        self.state.write_stdout(&format!("... {message}"));
    }

    pub fn table(&self, table: Table) {
        if table.headers.is_empty() {
            return;
        }
        self.state.write_stdout(&table.headers.join(" | "));
        self.state.write_stdout(
            &table
                .headers
                .iter()
                .map(|header| "-".repeat(header.len().max(3)))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        for row in table.rows {
            self.state.write_stdout(&row.join(" | "));
        }
    }

    pub fn json<T: Serialize>(&self, value: &T) -> PluginResult<()> {
        self.state
            .write_stdout(&serde_json::to_string_pretty(value)?);
        Ok(())
    }
}

/// Structured logger bound to plugin stderr.
#[derive(Clone)]
pub struct Log {
    state: Arc<RuntimeState>,
}

impl Log {
    pub(crate) fn new(state: Arc<RuntimeState>) -> Self {
        Self { state }
    }

    pub fn info(&self, message: impl Display) {
        self.state.write_stderr(&format!("[INFO] {message}"));
    }

    pub fn warn(&self, message: impl Display) {
        self.state.write_stderr(&format!("[WARN] {message}"));
    }

    pub fn error(&self, message: impl Display) {
        self.state.write_stderr(&format!("[ERROR] {message}"));
    }
}

/// Lightweight table builder for terminal-friendly output.
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
        }
    }

    pub fn row(mut self, row: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.rows.push(row.into_iter().map(Into::into).collect());
        self
    }
}
