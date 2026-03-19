use std::sync::Arc;

use crate::{Context, PluginError, PluginResult};

/// Executable command handler function.
pub type CommandHandler =
    Arc<dyn Fn(Context, CommandInput) -> PluginResult<()> + Send + Sync + 'static>;

/// Parsed command arguments exposed to plugin handlers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandInput {
    raw: Vec<String>,
}

impl CommandInput {
    pub(crate) fn new(raw: Vec<String>) -> Self {
        Self { raw }
    }

    pub fn raw(&self) -> &[String] {
        &self.raw
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn positional(&self, index: usize) -> Option<&str> {
        self.raw.get(index).map(String::as_str)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.raw.iter().any(|item| item == name)
    }

    pub fn option(&self, name: &str) -> Option<&str> {
        self.raw
            .iter()
            .position(|item| item == name)
            .and_then(|index| self.raw.get(index + 1))
            .map(String::as_str)
    }
}

/// Declarative command model used by plugin authors.
#[derive(Clone)]
pub struct PluginCommand {
    name: String,
    description: String,
    handler: Option<CommandHandler>,
}

impl PluginCommand {
    /// Create a routed subcommand definition.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            handler: None,
        }
    }

    /// Set the user-facing command description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Register the command handler.
    pub fn handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(Context, CommandInput) -> PluginResult<()> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        self
    }

    /// Borrow the routed subcommand name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Borrow the command description.
    ///
    /// This is the preferred accessor for v1.0 and later.
    pub fn summary(&self) -> &str {
        &self.description
    }

    /// Borrow the command description.
    ///
    /// Kept for pre-v1 compatibility. Prefer [`PluginCommand::summary`].
    pub fn description_text(&self) -> &str {
        &self.description
    }

    pub(crate) fn execute(&self, ctx: Context, args: CommandInput) -> PluginResult<()> {
        match &self.handler {
            Some(handler) => handler(ctx, args),
            None => Err(PluginError::new(format!(
                "command '{}' is missing a handler",
                self.name
            ))),
        }
    }
}
