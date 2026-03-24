//! High-level SDK for writing Terminal Info plugins.
//!
//! The SDK is intentionally opinionated:
//! - plugin authors target Rust APIs, not the raw host protocol
//! - configuration access is typed
//! - commands are routed declaratively
//! - plugin output uses shared presentation primitives
//! - tests can run without spawning the real host
//!
//! ```no_run
//! use serde::Serialize;
//! use tinfo_plugin::{Plugin, PluginCommand, PluginResult, StatusLevel};
//!
//! #[derive(Serialize)]
//! struct Snapshot {
//!     host: String,
//! }
//!
//! fn status(ctx: tinfo_plugin::Context, _args: tinfo_plugin::CommandInput) -> PluginResult<()> {
//!     let city = ctx.config.string("location")?.unwrap_or_else(|| "unknown".to_string());
//!     ctx.output().section("Status");
//!     ctx.output().status(StatusLevel::Ok, format!("configured city: {city}"));
//!     ctx.output().json(&Snapshot {
//!         host: ctx.host.version(),
//!     })?;
//!     Ok(())
//! }
//!
//! fn main() {
//!     Plugin::new("example")
//!         .description("Example plugin built with the Terminal Info SDK")
//!         .author("Plugin Author")
//!         .command(
//!             PluginCommand::new("status")
//!                 .description("Show a typed status view")
//!                 .handler(status),
//!         )
//!         .dispatch();
//! }
//! ```

mod command;
mod config;
mod context;
mod error;
mod manifest;
mod output;
mod widget;
pub mod testing;

use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;

pub use command::{CommandHandler, CommandInput, PluginCommand};
pub use config::ConfigContext;
pub use context::{CacheContext, Context, FsContext, HostContext, NetworkContext, NetworkRequest, SystemContext};
pub use error::{PluginError, PluginResult, ResultExt};
pub use manifest::{
    Capability, CompatibilityPolicy, ManifestValidation, PluginCompatibility, PluginManifest,
    PluginMetadata, SUPPORTED_PLUGIN_API,
};
pub use output::{Log, Output, StatusLevel, Table};
pub use widget::{Widget, WidgetBody, WidgetMode};

use context::{RuntimeHost, RuntimeState};

pub type WidgetHandler = Arc<dyn Fn(Context, WidgetMode) -> PluginResult<Widget> + Send + Sync>;

/// Declarative plugin application.
///
/// `Plugin` is the main builder entry point for plugin authors. It owns plugin
/// metadata, command routing, and host compatibility behavior.
pub struct Plugin {
    metadata: PluginMetadata,
    commands: Vec<PluginCommand>,
    default_handler: Option<CommandHandler>,
    widget_handler: Option<WidgetHandler>,
}

impl Plugin {
    /// Create a plugin with sane defaults and a stable compatibility target.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            metadata: PluginMetadata::new(
                name.into(),
                env!("CARGO_PKG_VERSION"),
                CompatibilityPolicy::current(),
            ),
            commands: Vec::new(),
            default_handler: None,
            widget_handler: None,
        }
    }

    /// Set the user-facing plugin description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.metadata.description = description.into();
        self
    }

    /// Set the plugin author.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.metadata.author = Some(author.into());
        self
    }

    /// Override the host compatibility requirement string.
    pub fn compatibility(mut self, requirement: impl Into<String>) -> Self {
        self.metadata.compatibility.tinfo = requirement.into();
        self
    }

    /// Declare a capability consumed by the plugin.
    pub fn capability(mut self, capability: Capability) -> Self {
        self.metadata.push_capability(capability);
        self
    }

    /// Declare multiple capabilities consumed by the plugin.
    pub fn capabilities<I>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = Capability>,
    {
        for capability in capabilities {
            self.metadata.push_capability(capability);
        }
        self
    }

    /// Register a routed subcommand.
    ///
    /// This controls plugin-local routing only. It does not change the
    /// top-level command names exposed through plugin metadata.
    pub fn command(mut self, command: PluginCommand) -> Self {
        self.commands.push(command);
        self
    }

    /// Expose an additional top-level command alias in metadata.
    ///
    /// Most plugins should not need this. It exists for compatibility with the
    /// host metadata schema, not for normal subcommand routing.
    pub fn command_alias(mut self, command: impl Into<String>) -> Self {
        self.metadata.push_command(command);
        self
    }

    /// Provide a default handler for plugins that do not use subcommands.
    pub fn default_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(Context, CommandInput) -> PluginResult<()> + Send + Sync + 'static,
    {
        self.default_handler = Some(Arc::new(handler));
        self
    }

    /// Backwards-compatible single-handler entry point.
    pub fn run<F>(self, handler: F)
    where
        F: Fn(Context) -> PluginResult<()> + Send + Sync + 'static,
    {
        self.default_handler(move |ctx, _| handler(ctx)).dispatch()
    }

    /// Register a structured dashboard widget handler.
    ///
    /// The host owns rendering. Plugins return a stable widget payload, and
    /// the host chooses how to display it in compact or full mode.
    pub fn widget<F>(mut self, handler: F) -> Self
    where
        F: Fn(Context, WidgetMode) -> PluginResult<Widget> + Send + Sync + 'static,
    {
        self.widget_handler = Some(Arc::new(handler));
        self
    }

    /// Execute the plugin using environment-provided host state.
    pub fn dispatch(self) {
        if let Err(err) = self.execute_from_env() {
            eprintln!("Plugin error: {err}");
            std::process::exit(1);
        }
    }

    /// Generate the canonical manifest model for the current plugin metadata.
    pub fn manifest(&self) -> PluginManifest {
        PluginManifest::from_metadata(&self.metadata)
    }

    /// Borrow the plugin metadata that will be emitted by `--metadata`.
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn execute_from_env(self) -> PluginResult<()> {
        let state = Arc::new(RuntimeState::terminal(RuntimeHost::from_env(&self.metadata.name)));
        let args = env::args().skip(1).collect::<Vec<_>>();
        self.execute_with_state(state, ConfigContext::load_from_env(), args)
    }

    pub(crate) fn execute_with_state(
        self,
        state: Arc<RuntimeState>,
        config: ConfigContext,
        args: Vec<String>,
    ) -> PluginResult<()> {
        if args.first().map(String::as_str) == Some("--widget") {
            let mode = if args.iter().any(|arg| arg == "--compact") {
                WidgetMode::Compact
            } else {
                WidgetMode::Full
            };
            let handler = self.widget_handler.ok_or_else(|| {
                PluginError::new("plugin does not provide a dashboard widget")
            })?;
            let widget = handler(Context::new(state.clone(), config), mode)?;
            state.write_stdout(&serde_json::to_string_pretty(&widget)?);
            return Ok(());
        }

        if args.first().map(String::as_str) == Some("--metadata") {
            state.write_stdout(&serde_json::to_string_pretty(&self.metadata)?);
            return Ok(());
        }

        if args.first().map(String::as_str) == Some("--manifest") {
            state.write_stdout(&self.manifest().to_toml_string()?);
            return Ok(());
        }

        if matches!(args.first().map(String::as_str), Some("--help") | Some("help")) {
            state.write_stdout(&self.help_text());
            return Ok(());
        }

        let ctx = Context::new(state.clone(), config);
        if let Some(first) = args.first() {
            if let Some(command) = self.commands.iter().find(|candidate| candidate.name() == first) {
                return command.execute(ctx, CommandInput::new(args[1..].to_vec()));
            }
        }

        if let Some(handler) = self.default_handler {
            return handler(ctx, CommandInput::new(args));
        }

        if self.commands.is_empty() {
            return Err(PluginError::new(
                "plugin has no handlers; register a command or a default handler",
            ));
        }

        Err(PluginError::new(self.help_text()))
    }

    fn help_text(&self) -> String {
        let mut lines = vec![format!("{} - {}", self.metadata.name, self.metadata.description)];
        if !self.commands.is_empty() {
            lines.push(String::new());
            lines.push("Commands:".to_string());
            for command in &self.commands {
                let description = command.description_text();
                if description.is_empty() {
                    lines.push(format!("  {}", command.name()));
                } else {
                    lines.push(format!("  {:<12} {}", command.name(), description));
                }
            }
        }
        lines.push(String::new());
        lines.push("Flags:".to_string());
        if self.widget_handler.is_some() {
            lines.push("  --widget      Print dashboard widget JSON".to_string());
            lines.push("  --widget --compact  Print compact dashboard widget JSON".to_string());
        }
        lines.push("  --metadata    Print plugin metadata as JSON".to_string());
        lines.push("  --manifest    Print the generated plugin.toml".to_string());
        lines.push("  --help        Print this help output".to_string());
        lines.join("\n")
    }
}

/// Return the simulated host environment used by the SDK and plugin tests.
pub fn host_environment() -> BTreeMap<String, String> {
    RuntimeHost::from_env("plugin").into_env()
}
