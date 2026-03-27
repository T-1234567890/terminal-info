mod builtins;
mod cache;
mod config;
mod config_menu;
mod dashboard;
mod disk;
mod hardware;
mod migration;
mod output;
mod plugin;
mod productivity;
mod process_inspect;
mod search;
mod speedtest;
mod storage;
mod theme;
mod weather;

use std::fs;
use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::{env, ffi::OsString};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use dialoguer::{Confirm, theme::ColorfulTheme};
use flate2::read::GzDecoder;
use minisign_verify::{PublicKey, Signature};
use reqwest::blocking::Client;
use reqwest::header::ACCEPT_ENCODING;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tar::Archive;
#[cfg(target_os = "windows")]
use zip::ZipArchive;

use crate::builtins::{
    run_config_doctor, run_diagnostic_all, run_diagnostic_full, run_diagnostic_leaks,
    run_diagnostic_network, run_diagnostic_performance, run_diagnostic_security,
    run_diagnostic_system, run_ping, show_network_info, show_system_info,
};
use crate::cache::{read_cache, write_cache};
use crate::config::{ApiProvider, Config, DefaultOutput, Units, config_path};
use crate::config_menu::show_config_menu;
use crate::output::{OutputMode, set_json_output, set_output_mode};
use crate::plugin::{
    info_plugin, init_plugin_template, install_plugin, list_plugins, list_trusted_plugins,
    plugin_browse,
    plugin_doctor, plugin_inspect, plugin_keygen, plugin_lint, plugin_pack, plugin_publish_check,
    plugin_sign, plugin_test, remove_plugin, run_diagnostic_plugins, run_plugin, search_plugins,
    set_plugin_trust, update_plugin, upgrade_all_plugins, verify_plugins,
};
use crate::productivity::{
    add_note, add_reminder, add_task, clear_notes, complete_task, delete_task,
    has_active_timer_state, interactive_task_menu, list_notes, list_tasks,
    replace_notes_with_single_entry, show_history, start_stopwatch, start_timer,
    stop_stopwatch, stop_timer, timer_dashboard_output,
};
use crate::theme::{AccentColor, BorderStyle, format_box_table, set_theme};
use crate::weather::{AlertsReport, ForecastReport, HourlyReport, WeatherClient, WeatherReport};

#[derive(Parser, Debug)]
#[command(name = "tinfo", version, about = "Terminal Info CLI")]
struct Cli {
    /// Use minimal output for scripts
    #[arg(long, conflicts_with_all = ["compact", "color"])]
    plain: bool,
    /// Use short one-line output when available
    #[arg(long, conflicts_with_all = ["plain", "color"])]
    compact: bool,
    /// Use interactive terminal formatting
    #[arg(long, conflicts_with_all = ["plain", "compact"])]
    color: bool,
    /// Output machine-readable JSON where supported
    #[arg(long, global = true)]
    json: bool,
    /// Render once and exit instead of refreshing live views
    #[arg(long, global = true, conflicts_with = "live")]
    freeze: bool,
    /// Force live updates even when dashboard freeze is enabled in config
    #[arg(long, global = true, conflicts_with = "freeze")]
    live: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Weather related commands
    Weather {
        #[command(subcommand)]
        command: Option<WeatherCommand>,
    },
    /// Test network latency to a host
    Ping {
        /// Hostname to test
        host: Option<String>,
    },
    /// Test network latency using the same probes as ping
    Latency {
        /// Hostname to test, or `full` for expanded probes
        host: Option<String>,
    },
    /// Show network information
    Network {
        #[command(subcommand)]
        command: Option<NetworkCommand>,
    },
    /// Inspect physical disk health and reliability
    Disk {
        #[command(subcommand)]
        command: Option<DiskCommand>,
    },
    /// Analyze filesystem usage and storage optimization opportunities
    Storage {
        #[command(subcommand)]
        command: Option<StorageCommand>,
    },
    /// Show system information
    System {
        #[command(subcommand)]
        command: Option<SystemCommand>,
    },
    /// Inspect running processes
    #[command(visible_alias = "top")]
    Ps {
        /// Maximum number of processes to display
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Sort process list by cpu or memory
        #[arg(long, value_enum, default_value_t = ProcessSortArg::Cpu)]
        sort: ProcessSortArg,
    },
    /// Show local or global times
    Time {
        /// Optional city name
        city: Option<String>,
    },
    /// Show or manage countdown timers
    Timer {
        #[command(subcommand)]
        command: Option<TimerCommand>,
    },
    /// Manage a stopwatch separately from countdown timers
    Stopwatch {
        #[command(subcommand)]
        command: StopwatchCommand,
    },
    /// Manage local tasks
    Task {
        #[command(subcommand)]
        command: Option<TaskCommand>,
    },
    /// Capture quick notes
    Note {
        #[command(subcommand)]
        command: NoteCommand,
    },
    /// Show recent shell commands
    History {
        /// Maximum number of history lines to show
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Schedule a reminder after a delay
    Remind {
        /// Delay such as `10m` or `1h30m`
        time: Option<String>,
        /// Optional reminder message
        message: Vec<String>,
    },
    /// Search built-ins and plugins
    Search {
        /// Search term
        #[arg(required = true)]
        query: Vec<String>,
    },
    /// Run diagnostics
    Diagnostic {
        /// Export the diagnostic result as Markdown to the given path
        #[arg(long, value_name = "PATH")]
        markdown_out: Option<PathBuf>,
        #[command(subcommand)]
        command: Option<DiagnosticCommand>,
    },
    /// Manage configuration
    #[command(visible_alias = "configure")]
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
    /// Manage configuration profiles
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        shell: CompletionCommand,
    },
    /// Configure dashboard behavior
    Dashboard {
        #[command(subcommand)]
        command: Option<DashboardCommand>,
    },
    /// Manage plugins and scaffold new plugin projects
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    /// Download and install the latest released version of tinfo
    Update,
    /// Repair the current installation using the latest release
    SelfRepair,
    /// Reinstall the latest release
    Reinstall,
    /// Remove the Terminal Info binary and optionally its local data
    Uninstall {
        /// Remove the binary only and keep ~/.terminal-info
        #[arg(long)]
        keep_data: bool,
    },
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
enum WeatherCommand {
    /// Show current weather for the configured location or a city
    Now {
        /// City name, such as Tokyo or London
        city: Option<String>,
    },
    /// Show a short forecast for the configured location or a city
    Forecast {
        /// City name, such as Tokyo or London
        city: Option<String>,
    },
    /// Show hourly weather for the configured location or a city
    Hourly {
        /// City name, such as Tokyo or London
        city: Option<String>,
    },
    /// Show weather alerts for the configured location or a city
    Alerts {
        /// City name, such as Tokyo or London
        city: Option<String>,
    },
    /// Show or set the default location
    Location {
        /// City name to store as the default location
        city: Option<String>,
    },
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Run the guided first-run setup again
    Setup,
    /// Show or set the default location
    Location {
        /// City name to store
        city: Option<String>,
    },
    /// Show or set units
    Units { units: Option<UnitArg> },
    /// Show or set the default output mode
    Output { mode: Option<OutputArg> },
    /// Show or change theme preferences
    Theme {
        #[command(subcommand)]
        command: Option<ThemeCommand>,
    },
    /// Show or set API provider configuration
    Api {
        #[command(subcommand)]
        command: Option<ApiCommand>,
    },
    /// Enable, disable, or inspect server mode
    Server {
        #[command(subcommand)]
        command: Option<ServerCommand>,
    },
    /// Show or change dashboard widgets
    Widgets {
        #[command(subcommand)]
        command: Option<WidgetsCommand>,
    },
    /// Open the TOML config file with the system default app
    Open,
    /// Edit the TOML config file in $EDITOR, nano, or vim
    Edit,
    /// Reset configuration to defaults
    Reset,
    /// Run configuration diagnostics
    Doctor,
}

#[derive(Subcommand, Debug)]
enum ProfileCommand {
    /// Use a profile
    Use { name: String },
    /// List profiles
    List,
    /// Show a profile definition
    Show { name: String },
    /// Add a profile from the current effective settings
    Add { name: String },
    /// Remove a profile
    Remove { name: String },
}

#[derive(Subcommand, Debug)]
enum TaskCommand {
    Add { text: Vec<String> },
    List,
    Done { id: u64 },
    Delete { id: u64 },
}

#[derive(Subcommand, Debug)]
enum NoteCommand {
    Add { text: Vec<String> },
    List,
}

#[derive(Subcommand, Debug)]
enum TimerCommand {
    Start { duration: Option<String> },
    Stop,
}

#[derive(Subcommand, Debug)]
enum StopwatchCommand {
    Start,
    Stop,
}

#[derive(Subcommand, Debug)]
enum ApiCommand {
    /// Save an API provider and key
    Set { provider: ProviderArg, key: String },
    /// Show the current API configuration
    Show,
}

#[derive(Subcommand, Debug)]
enum ServerCommand {
    /// Enable server mode
    Enable,
    /// Disable server mode
    Disable,
    /// Show server mode status
    Status,
}

#[derive(Subcommand, Debug)]
enum WidgetsCommand {
    /// Show the active widget order
    Show,
    /// Add a widget to the end of the dashboard
    Add { name: String },
    /// Remove a widget from the dashboard
    Remove { name: String },
    /// Replace the full widget order
    Set { names: Vec<String> },
    /// Reset widget order to defaults
    Reset,
}

#[derive(Subcommand, Debug)]
enum PluginCommand {
    /// List installed plugins
    List,
    /// Search for plugins
    Search {
        /// Optional search term
        query: Vec<String>,
    },
    /// Browse plugins in a local browser UI
    Browse {
        /// Do not try to open the browser automatically
        #[arg(long)]
        no_open: bool,
    },
    /// Interactively scaffold a new plugin template
    Init {
        /// Optional plugin name used as the default prompt value
        name: Option<String>,
    },
    /// Generate a Minisign keypair for plugin releases
    Keygen {
        /// Directory to write minisign.key and minisign.pub into
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Sign a plugin release artifact with Minisign
    Sign {
        /// File to sign, such as a plugin archive or binary
        file: PathBuf,
        /// Path to the Minisign secret key
        #[arg(long)]
        key: Option<PathBuf>,
    },
    /// Inspect local plugin metadata and compatibility
    Inspect,
    /// Run a local plugin test with host simulation
    Test,
    /// Build a signed plugin release bundle and registry JSON
    Pack {
        /// Generate registry JSON from existing dist artifacts instead of building locally
        #[arg(long)]
        from_dist: bool,
    },
    /// Install a plugin
    Install { name: String },
    /// Trust a plugin so it can execute
    Trust { name: String },
    /// Remove trust from a plugin
    Untrust { name: String },
    /// List trusted plugins
    Trusted,
    /// Show plugin details
    Info { name: String },
    /// Verify installed plugins
    Verify,
    /// Run detailed plugin checks against the current environment
    Doctor,
    /// Validate the current plugin project files
    Lint,
    /// Validate plugin release artifacts before publishing
    PublishCheck,
    /// Update a plugin
    Update { name: String },
    /// Update all installed plugins
    UpgradeAll,
    /// Remove a plugin
    Remove { name: String },
}

#[derive(Subcommand, Debug)]
enum DiagnosticCommand {
    /// Run network diagnostics
    Network,
    /// Run system diagnostics
    System,
    /// Run plugin diagnostics
    Plugins,
    /// Run server performance diagnostics
    Performance,
    /// Run server security diagnostics
    Security,
    /// Run local leak detection checks
    Leaks,
    /// Run a comprehensive diagnostic pass
    Full,
}

#[derive(Subcommand, Debug)]
enum DiskCommand {
    /// Show a quick disk health overview
    Health,
    /// Show detailed SMART attributes
    Smart,
    /// Show disk temperature and thermal status
    Temperature,
    /// Analyze reliability indicators
    Reliability,
}

#[derive(Subcommand, Debug)]
enum StorageCommand {
    /// Show filesystem usage overview
    Usage,
    /// Identify the largest directories or files
    Largest,
    /// Analyze storage consumption
    Analyze,
    /// Suggest storage cleanup opportunities
    Optimize,
}

#[derive(Subcommand, Debug)]
enum NetworkCommand {
    /// Measure network download speed
    Speed,
}

#[derive(Subcommand, Debug)]
enum SystemCommand {
    /// Show detailed hardware inventory
    Hardware,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CompletionCommand {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Install,
    Uninstall,
    Status,
}

#[derive(Subcommand, Debug)]
enum DashboardCommand {
    /// Show dashboard settings
    Config,
    /// Reset dashboard settings
    Reset,
    /// Manage dashboard notes
    Notes {
        #[command(subcommand)]
        command: DashboardNotesCommand,
    },
}

#[derive(Subcommand, Debug)]
enum DashboardNotesCommand {
    /// Show the current notes file
    Show,
    /// Replace notes content
    Set { text: Vec<String> },
    /// Clear saved notes
    Clear,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ProviderArg {
    Openweather,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum UnitArg {
    Metric,
    Imperial,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputArg {
    Plain,
    Compact,
    Color,
}

#[derive(Subcommand, Debug)]
enum ThemeCommand {
    /// Show the active theme settings
    Show,
    /// Show or set the border style
    Border { style: Option<BorderStyleArg> },
    /// Show or set the accent color
    Accent { color: Option<AccentColorArg> },
    /// Enable or disable Unicode box drawing
    Unicode { enabled: Option<ToggleArg> },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum BorderStyleArg {
    Sharp,
    Rounded,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum AccentColorArg {
    Auto,
    Blue,
    Cyan,
    Green,
    Magenta,
    Red,
    Yellow,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ToggleArg {
    On,
    Off,
}

impl From<BorderStyleArg> for BorderStyle {
    fn from(value: BorderStyleArg) -> Self {
        match value {
            BorderStyleArg::Sharp => BorderStyle::Sharp,
            BorderStyleArg::Rounded => BorderStyle::Rounded,
        }
    }
}

impl From<AccentColorArg> for AccentColor {
    fn from(value: AccentColorArg) -> Self {
        match value {
            AccentColorArg::Auto => AccentColor::Auto,
            AccentColorArg::Blue => AccentColor::Blue,
            AccentColorArg::Cyan => AccentColor::Cyan,
            AccentColorArg::Green => AccentColor::Green,
            AccentColorArg::Magenta => AccentColor::Magenta,
            AccentColorArg::Red => AccentColor::Red,
            AccentColorArg::Yellow => AccentColor::Yellow,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ProcessSortArg {
    Cpu,
    Memory,
}

impl From<ProcessSortArg> for process_inspect::ProcessSort {
    fn from(value: ProcessSortArg) -> Self {
        match value {
            ProcessSortArg::Cpu => process_inspect::ProcessSort::Cpu,
            ProcessSortArg::Memory => process_inspect::ProcessSort::Memory,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let _migration_status = match migration::run_startup_migration() {
        Ok(status) => status,
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    };
    let mut config = match Config::load_or_create() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    };

    if should_run_first_run_setup(&cli, &config) {
        if let Err(err) = config_menu::run_first_run_setup(&mut config) {
            eprintln!("{err}");
            process::exit(1);
        }
    }

    set_output_mode(resolve_output_mode(&cli, &config));
    set_theme(config.theme);
    set_json_output(cli.json);
    let live_view_freeze = resolve_live_view_freeze(&cli);
    let dashboard_freeze = resolve_dashboard_freeze(&cli, &config);

    let result = match cli.command {
        Some(Command::Weather { command }) => handle_weather(&mut config, command, live_view_freeze),
        Some(Command::Ping { host }) => handle_ping(&config, host),
        Some(Command::Latency { host }) => handle_latency(&config, host),
        Some(Command::Network { command }) => handle_network(command),
        Some(Command::Disk { command }) => handle_disk(command),
        Some(Command::Storage { command }) => handle_storage(command),
        Some(Command::System { command }) => handle_system(command),
        Some(Command::Ps { limit, sort }) => {
            process_inspect::show_processes(limit, sort.into())
        }
        Some(Command::Time { city }) => live_time(city, live_view_freeze),
        Some(Command::Timer { command }) => handle_timer(command, live_view_freeze, &config),
        Some(Command::Stopwatch { command }) => handle_stopwatch(command),
        Some(Command::Task { command }) => handle_task(&config, command),
        Some(Command::Note { command }) => handle_note(command),
        Some(Command::History { limit }) => show_history(limit),
        Some(Command::Remind { time, message }) => {
            let joined = if message.is_empty() {
                None
            } else {
                Some(message.join(" "))
            };
            handle_remind(&config, time.as_deref(), joined.as_deref())
        }
        Some(Command::Search { query }) => search::run_search(&query),
        Some(Command::Diagnostic {
            command,
            markdown_out,
        }) => handle_diagnostic(&config, command, markdown_out),
        Some(Command::Config { command }) => handle_config(&mut config, command),
        Some(Command::Profile { command }) => handle_profile(&mut config, command),
        Some(Command::Completion { shell }) => {
            handle_completion(shell);
            Ok(())
        }
        Some(Command::Dashboard { command }) => handle_dashboard(&mut config, command, dashboard_freeze),
        Some(Command::Plugin { command }) => handle_plugin(command),
        Some(Command::Update) => handle_update(),
        Some(Command::SelfRepair) => handle_self_repair(),
        Some(Command::Reinstall) => handle_reinstall(),
        Some(Command::Uninstall { keep_data }) => handle_uninstall(keep_data),
        Some(Command::External(args)) => handle_external(args),
        None => live_dashboard(&config, dashboard_freeze),
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn resolve_live_view_freeze(cli: &Cli) -> bool {
    if cli.freeze {
        true
    } else if cli.live {
        false
    } else {
        cli.json || !io::stdout().is_terminal()
    }
}

fn resolve_dashboard_freeze(cli: &Cli, config: &Config) -> bool {
    if cli.freeze {
        true
    } else if cli.live {
        false
    } else if config.effective_dashboard().freeze {
        true
    } else {
        cli.json || !io::stdout().is_terminal()
    }
}

fn should_run_first_run_setup(cli: &Cli, config: &Config) -> bool {
    if config.setup_complete || cli.json || !io::stdin().is_terminal() || !io::stdout().is_terminal()
    {
        return false;
    }

    matches!(cli.command, None | Some(Command::Config { command: None }))
}

fn resolve_output_mode(cli: &Cli, config: &Config) -> OutputMode {
    if cli.plain {
        OutputMode::Plain
    } else if cli.compact {
        OutputMode::Compact
    } else if cli.color {
        OutputMode::Color
    } else {
        config.default_output.as_output_mode()
    }
}

fn handle_diagnostic(
    config: &Config,
    command: Option<DiagnosticCommand>,
    markdown_out: Option<PathBuf>,
) -> Result<(), String> {
    if let Some(path) = markdown_out {
        return match command {
            None => builtins::write_diagnostic_markdown(&path, config, false),
            Some(DiagnosticCommand::Full) => builtins::write_diagnostic_markdown(&path, config, true),
            _ => Err(
                "Markdown export is only supported for `tinfo diagnostic` and `tinfo diagnostic full`."
                    .to_string(),
            ),
        };
    }

    match command {
        Some(DiagnosticCommand::Network) => run_diagnostic_network(config.server_mode),
        Some(DiagnosticCommand::System) => run_diagnostic_system(config.server_mode),
        Some(DiagnosticCommand::Plugins) => run_diagnostic_plugins(),
        Some(DiagnosticCommand::Performance) => run_diagnostic_performance(config.server_mode),
        Some(DiagnosticCommand::Security) => {
            ensure_server_mode_enabled(config)?;
            run_diagnostic_security(config)
        }
        Some(DiagnosticCommand::Leaks) => {
            ensure_server_mode_enabled(config)?;
            run_diagnostic_leaks(config)
        }
        Some(DiagnosticCommand::Full) => run_diagnostic_full(config, config.server_mode),
        None => run_diagnostic_all(),
    }
}

fn handle_ping(config: &Config, host: Option<String>) -> Result<(), String> {
    if config.server_mode && is_full_probe_request(host.as_deref()) && !crate::output::json_output()
    {
        println!("[Server Mode Enabled]");
    }
    run_ping(host, config.server_mode)
}

fn handle_disk(command: Option<DiskCommand>) -> Result<(), String> {
    match command {
        None | Some(DiskCommand::Health) => disk::show_disk_health(),
        Some(DiskCommand::Smart) => disk::show_disk_smart(),
        Some(DiskCommand::Temperature) => disk::show_disk_temperature(),
        Some(DiskCommand::Reliability) => disk::show_disk_reliability(),
    }
}

fn handle_network(command: Option<NetworkCommand>) -> Result<(), String> {
    match command {
        None => show_network_info(),
        Some(NetworkCommand::Speed) => speedtest::show_network_speed(),
    }
}

fn handle_system(command: Option<SystemCommand>) -> Result<(), String> {
    match command {
        None => show_system_info(),
        Some(SystemCommand::Hardware) => hardware::show_system_hardware(),
    }
}

fn handle_storage(command: Option<StorageCommand>) -> Result<(), String> {
    match command {
        None | Some(StorageCommand::Usage) => storage::show_storage_usage(),
        Some(StorageCommand::Largest) => storage::show_storage_largest(),
        Some(StorageCommand::Analyze) => storage::show_storage_analyze(),
        Some(StorageCommand::Optimize) => storage::show_storage_optimize(),
    }
}

fn handle_latency(config: &Config, host: Option<String>) -> Result<(), String> {
    if config.server_mode && is_full_probe_request(host.as_deref()) && !crate::output::json_output()
    {
        println!("[Server Mode Enabled]");
    }
    run_ping(host, config.server_mode)
}

fn is_full_probe_request(host: Option<&str>) -> bool {
    matches!(host, Some(value) if value.eq_ignore_ascii_case("full"))
}

fn handle_external(args: Vec<String>) -> Result<(), String> {
    let Some((command, remaining)) = args.split_first() else {
        return Ok(());
    };

    run_plugin(command, remaining)
}

fn handle_weather(
    config: &mut Config,
    command: Option<WeatherCommand>,
    freeze: bool,
) -> Result<(), String> {
    match command {
        Some(WeatherCommand::Now { city }) => live_weather(config, WeatherView::Now(city), freeze),
        Some(WeatherCommand::Forecast { city }) => {
            live_weather(config, WeatherView::Forecast(city), freeze)
        }
        Some(WeatherCommand::Hourly { city }) => {
            live_weather(config, WeatherView::Hourly(city), freeze)
        }
        Some(WeatherCommand::Alerts { city }) => {
            live_weather(config, WeatherView::Alerts(city), freeze)
        }
        Some(WeatherCommand::Location { city }) => handle_location(config, city),
        Some(WeatherCommand::External(args)) => {
            let Some((first, _)) = args.split_first() else {
                return live_weather(config, WeatherView::Now(None), freeze);
            };
            live_weather(config, WeatherView::Now(Some(first.clone())), freeze)
        }
        None => live_weather(config, WeatherView::Now(None), freeze),
    }
}

enum WeatherView {
    Now(Option<String>),
    Forecast(Option<String>),
    Hourly(Option<String>),
    Alerts(Option<String>),
}

fn live_dashboard(config: &Config, freeze: bool) -> Result<(), String> {
    let effective_dashboard = config.effective_dashboard();
    let mut renderer = dashboard::DashboardRenderer::new(config.clone());
    run_live_loop(
        Duration::from_secs(effective_dashboard.refresh_interval.max(1)),
        freeze,
        || Ok(renderer.render()),
    )
}

fn live_time(city: Option<String>, freeze: bool) -> Result<(), String> {
    run_live_loop(Duration::from_secs(1), freeze, || {
        crate::builtins::time_output(city.clone())
    })
}

fn live_weather(config: &Config, view: WeatherView, freeze: bool) -> Result<(), String> {
    run_live_loop(Duration::from_secs(60), freeze, || match &view {
        WeatherView::Now(city) => handle_now(config, city.clone()),
        WeatherView::Forecast(city) => handle_forecast(config, city.clone()),
        WeatherView::Hourly(city) => handle_hourly(config, city.clone()),
        WeatherView::Alerts(city) => handle_alerts(config, city.clone()),
    })
}

fn run_live_loop<F>(interval: Duration, freeze: bool, mut render: F) -> Result<(), String>
where
    F: FnMut() -> Result<String, String>,
{
    if freeze {
        print!("{}", render()?);
        io::stdout()
            .flush()
            .map_err(|err| format!("Failed to flush output: {err}"))?;
        return Ok(());
    }

    let mut stdout = io::stdout();
    let _terminal = LiveTerminalGuard::enter(&mut stdout)?;

    loop {
        clear_screen(&mut stdout)?;
        write_live_frame(&mut stdout, &render()?)?;
        write!(stdout, "\r\nPress q or Ctrl+C to exit\r\n")
            .map_err(|err| format!("Failed to write exit hint: {err}"))?;
        stdout
            .flush()
            .map_err(|err| format!("Failed to flush output: {err}"))?;

        let deadline = Instant::now() + interval;
        while Instant::now() < deadline {
            if event::poll(Duration::from_millis(100))
                .map_err(|err| format!("Failed to read terminal input: {err}"))?
            {
                let next =
                    event::read().map_err(|err| format!("Failed to read terminal input: {err}"))?;
                if should_exit_live_view(&next) {
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn clear_screen(stdout: &mut io::Stdout) -> Result<(), String> {
    write!(stdout, "\x1B[2J\x1B[H").map_err(|err| format!("Failed to clear terminal screen: {err}"))
}

fn should_exit_live_view(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(key)
            if key.kind != KeyEventKind::Release
                && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
    )
}

fn write_live_frame(stdout: &mut io::Stdout, frame: &str) -> Result<(), String> {
    let normalized = frame.trim_end_matches('\n').replace('\n', "\r\n");
    write!(stdout, "{normalized}").map_err(|err| format!("Failed to write live frame: {err}"))
}

struct LiveTerminalGuard;

impl LiveTerminalGuard {
    fn enter(stdout: &mut io::Stdout) -> Result<Self, String> {
        terminal::enable_raw_mode().map_err(|err| format!("Failed to enable raw mode: {err}"))?;
        execute!(stdout, EnterAlternateScreen, Hide)
            .map_err(|err| format!("Failed to initialize terminal UI: {err}"))?;
        Ok(Self)
    }
}

impl Drop for LiveTerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn handle_config(config: &mut Config, command: Option<ConfigCommand>) -> Result<(), String> {
    match command {
        Some(ConfigCommand::Setup) => config_menu::run_first_run_setup(config),
        Some(ConfigCommand::Location { city }) => handle_location(config, city),
        Some(ConfigCommand::Units { units }) => match units {
            Some(UnitArg::Metric) => {
                config.units = Units::Metric;
                config.save()?;
                println!("Units set to metric.");
                Ok(())
            }
            Some(UnitArg::Imperial) => {
                config.units = Units::Imperial;
                config.save()?;
                println!("Units set to imperial.");
                Ok(())
            }
            None => {
                println!("Units: {}", config.units.label());
                Ok(())
            }
        },
        Some(ConfigCommand::Output { mode }) => match mode {
            Some(OutputArg::Plain) => {
                config.default_output = DefaultOutput::Plain;
                config.save()?;
                println!("Default output set to plain.");
                Ok(())
            }
            Some(OutputArg::Compact) => {
                config.default_output = DefaultOutput::Compact;
                config.save()?;
                println!("Default output set to compact.");
                Ok(())
            }
            Some(OutputArg::Color) => {
                config.default_output = DefaultOutput::Color;
                config.save()?;
                println!("Default output set to color.");
                Ok(())
            }
            None => {
                println!("Default output: {}", config.default_output.label());
                Ok(())
            }
        },
        Some(ConfigCommand::Theme { command }) => handle_theme_config(config, command),
        Some(ConfigCommand::Api { command }) => match command {
            Some(ApiCommand::Set { provider, key }) => {
                config.provider = Some(match provider {
                    ProviderArg::Openweather => ApiProvider::OpenWeather,
                });
                config.api_key = Some(key);
                config.save()?;
                println!("API provider saved.");
                Ok(())
            }
            Some(ApiCommand::Show) | None => {
                print_api_config(config);
                Ok(())
            }
        },
        Some(ConfigCommand::Server { command }) => handle_server_mode(config, command),
        Some(ConfigCommand::Widgets { command }) => handle_widgets_config(config, command),
        Some(ConfigCommand::Open) => handle_config_open(config),
        Some(ConfigCommand::Edit) => handle_config_edit(config),
        Some(ConfigCommand::Reset) => {
            config.reset();
            config.save()?;
            println!("Configuration reset.");
            Ok(())
        }
        Some(ConfigCommand::Doctor) => run_config_doctor(config),
        None => {
            show_config_menu(config)?;
            print_advanced_config_hint()?;
            Ok(())
        }
    }
}

const SUPPORTED_DASHBOARD_WIDGETS: &[&str] = &[
    "weather",
    "time",
    "network",
    "system",
    "timer",
    "tasks",
    "notes",
    "history",
    "reminders",
    "plugins",
];

fn normalize_widget_name(name: &str) -> Result<String, String> {
    let normalized = name.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err("Widget name cannot be empty.".to_string());
    }
    if SUPPORTED_DASHBOARD_WIDGETS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(format!(
            "Unsupported widget '{}'. Supported widgets: {}.",
            name,
            SUPPORTED_DASHBOARD_WIDGETS.join(", ")
        ))
    }
}

fn handle_widgets_config(
    config: &mut Config,
    command: Option<WidgetsCommand>,
) -> Result<(), String> {
    match command.unwrap_or(WidgetsCommand::Show) {
        WidgetsCommand::Show => {
            println!("Dashboard widgets: {}", config.dashboard.widgets.join(", "));
            Ok(())
        }
        WidgetsCommand::Add { name } => {
            let name = normalize_widget_name(&name)?;
            if config.dashboard.widgets.iter().any(|item| item == &name) {
                println!("Widget '{}' is already enabled.", name);
                return Ok(());
            }
            config.dashboard.widgets.push(name.clone());
            config.save()?;
            println!("Added widget '{}'.", name);
            Ok(())
        }
        WidgetsCommand::Remove { name } => {
            let name = normalize_widget_name(&name)?;
            let before = config.dashboard.widgets.len();
            config.dashboard.widgets.retain(|item| item != &name);
            if before == config.dashboard.widgets.len() {
                println!("Widget '{}' is not enabled.", name);
                return Ok(());
            }
            config.save()?;
            println!("Removed widget '{}'.", name);
            Ok(())
        }
        WidgetsCommand::Set { names } => {
            if names.is_empty() {
                return Err("Provide at least one widget name.".to_string());
            }
            let mut widgets = Vec::new();
            for name in names {
                let name = normalize_widget_name(&name)?;
                if !widgets.iter().any(|item| item == &name) {
                    widgets.push(name);
                }
            }
            config.dashboard.widgets = widgets;
            config.save()?;
            println!(
                "Dashboard widgets set to: {}",
                config.dashboard.widgets.join(", ")
            );
            Ok(())
        }
        WidgetsCommand::Reset => {
            config.dashboard.widgets = crate::config::DashboardConfig::default().widgets;
            config.save()?;
            println!(
                "Dashboard widgets reset to: {}",
                config.dashboard.widgets.join(", ")
            );
            Ok(())
        }
    }
}

fn handle_theme_config(config: &mut Config, command: Option<ThemeCommand>) -> Result<(), String> {
    match command.unwrap_or(ThemeCommand::Show) {
        ThemeCommand::Show => {
            println!("Border style: {}", config.theme.border_style.label());
            println!("Accent color: {}", config.theme.accent_color.label());
            println!(
                "Unicode borders: {}",
                if config.theme.unicode_enabled() {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            Ok(())
        }
        ThemeCommand::Border { style } => match style {
            Some(style) => {
                config.theme.border_style = style.into();
                config.save()?;
                set_theme(config.theme);
                println!("Theme border style set to {}.", config.theme.border_style.label());
                Ok(())
            }
            None => {
                println!("Border style: {}", config.theme.border_style.label());
                Ok(())
            }
        },
        ThemeCommand::Accent { color } => match color {
            Some(color) => {
                config.theme.accent_color = color.into();
                config.save()?;
                set_theme(config.theme);
                println!("Theme accent color set to {}.", config.theme.accent_color.label());
                Ok(())
            }
            None => {
                println!("Accent color: {}", config.theme.accent_color.label());
                Ok(())
            }
        },
        ThemeCommand::Unicode { enabled } => match enabled {
            Some(ToggleArg::On) => {
                config.theme.ascii_only = false;
                config.save()?;
                set_theme(config.theme);
                println!("Unicode borders enabled.");
                Ok(())
            }
            Some(ToggleArg::Off) => {
                config.theme.ascii_only = true;
                config.save()?;
                set_theme(config.theme);
                println!("Unicode borders disabled.");
                Ok(())
            }
            None => {
                println!(
                    "Unicode borders: {}",
                    if config.theme.unicode_enabled() {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                Ok(())
            }
        },
    }
}

pub(crate) fn handle_config_open(config: &Config) -> Result<(), String> {
    let path = config_path()?;
    if !path.exists() {
        config.save()?;
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = process::Command::new("open");
        command.arg(&path);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = process::Command::new("xdg-open");
        command.arg(&path);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = process::Command::new("cmd");
        command.arg("/C").arg("start").arg("").arg(&path);
        command
    };

    let status = command
        .status()
        .map_err(|err| format!("Failed to open config file '{}': {err}", path.display()))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Open command exited with status {}.",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ))
    }
}

pub(crate) fn handle_config_edit(config: &Config) -> Result<(), String> {
    let path = config_path()?;
    if !path.exists() {
        config.save()?;
    }
    let editor = preferred_editor()
        .ok_or_else(|| "No editor found. Set $EDITOR or install nano/vim.".to_string())?;

    let status = process::Command::new(&editor)
        .arg(&path)
        .status()
        .map_err(|err| format!("Failed to launch editor '{}': {err}", editor.to_string_lossy()))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Editor '{}' exited with status {}.",
            editor.to_string_lossy(),
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ))
    }
}

fn preferred_editor() -> Option<OsString> {
    if let Some(editor) = env::var_os("EDITOR").filter(|value| !value.is_empty()) {
        return Some(editor);
    }

    ["nano", "vim"]
        .into_iter()
        .find(|candidate| command_in_path(candidate))
        .map(OsString::from)
}

fn command_in_path(command: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path).any(|dir| dir.join(command).exists())
}

pub(crate) fn print_advanced_config_hint() -> Result<(), String> {
    let path = config_path()?;
    println!();
    println!("Advanced / more configuration");
    println!("Config file:");
    println!("{}", path.display());
    println!("Use `tinfo config open` to open the TOML config file.");
    println!("Use `tinfo config edit` to open it in $EDITOR, nano, or vim.");
    Ok(())
}

fn handle_server_mode(config: &mut Config, command: Option<ServerCommand>) -> Result<(), String> {
    match command.unwrap_or(ServerCommand::Status) {
        ServerCommand::Enable => enable_server_mode(config),
        ServerCommand::Disable => {
            config.server_mode = false;
            config.save()?;
            println!("Server mode disabled.");
            Ok(())
        }
        ServerCommand::Status => {
            println!(
                "Server mode: {}",
                if config.server_mode {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            Ok(())
        }
    }
}

fn enable_server_mode(config: &mut Config) -> Result<(), String> {
    if config.server_mode {
        println!("Server mode is already enabled.");
        return Ok(());
    }

    println!(
        "Server mode is designed for servers or VPS environments and is not recommended for regular desktop computers."
    );
    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Enable server mode?")
        .default(false)
        .interact()
        .map_err(|err| format!("Failed to read confirmation: {err}"))?;
    if !confirmed {
        println!("Server mode was not changed.");
        return Ok(());
    }

    config.server_mode = true;
    config.save()?;
    println!("Server mode enabled.");
    Ok(())
}

fn ensure_server_mode_enabled(config: &Config) -> Result<(), String> {
    if config.server_mode {
        Ok(())
    } else {
        Err(
            "This feature requires server mode.\nEnable it using: tinfo config server enable"
                .to_string(),
        )
    }
}

fn handle_profile(config: &mut Config, command: ProfileCommand) -> Result<(), String> {
    match command {
        ProfileCommand::Use { name } => {
            config.apply_profile(&name)?;
            config.save()?;
            println!("Using profile '{}'.", name);
            Ok(())
        }
        ProfileCommand::List => {
            if config.profile.is_empty() {
                println!("No profiles configured.");
                return Ok(());
            }

            for name in config.profile.keys() {
                if config.active_profile.as_deref() == Some(name.as_str()) {
                    println!("* {name}");
                } else {
                    println!("  {name}");
                }
            }

            Ok(())
        }
        ProfileCommand::Show { name } => {
            let profile = config
                .profile_named(&name)
                .ok_or_else(|| format!("Profile '{}' not found.", name))?;
            if crate::output::json_output() {
                println!(
                    "{}",
                    serde_json::to_string_pretty(profile).unwrap_or_else(|_| "{}".to_string())
                );
                return Ok(());
            }
            println!("Profile: {name}");
            println!(
                "Location: {}",
                profile.location.as_deref().unwrap_or("inherit")
            );
            println!(
                "Units: {}",
                profile
                    .units
                    .map(|units| units.label().to_string())
                    .unwrap_or_else(|| "inherit".to_string())
            );
            println!(
                "Provider: {}",
                profile
                    .provider
                    .map(|provider| match provider {
                        ApiProvider::OpenWeather => "openweather".to_string(),
                    })
                    .unwrap_or_else(|| "inherit".to_string())
            );
            println!(
                "API key: {}",
                profile
                    .api_key
                    .as_deref()
                    .map(|_| "set")
                    .unwrap_or("inherit")
            );
            if let Some(dashboard) = &profile.dashboard {
                println!("Dashboard widgets: {}", dashboard.widgets.join(", "));
                println!("Dashboard refresh: {}s", dashboard.refresh_interval);
                println!("Dashboard compact: {}", dashboard.compact_mode);
            } else {
                println!("Dashboard: inherit");
            }
            Ok(())
        }
        ProfileCommand::Add { name } => {
            config.add_profile_from_current(&name)?;
            config.save()?;
            println!("Added profile '{}'.", name);
            Ok(())
        }
        ProfileCommand::Remove { name } => {
            config.remove_profile(&name)?;
            config.save()?;
            println!("Removed profile '{}'.", name);
            Ok(())
        }
    }
}

fn handle_task(config: &Config, command: Option<TaskCommand>) -> Result<(), String> {
    match command {
        None => interactive_task_menu(config),
        Some(TaskCommand::Add { text }) => add_task(&text.join(" ")),
        Some(TaskCommand::List) => list_tasks(),
        Some(TaskCommand::Done { id }) => complete_task(id),
        Some(TaskCommand::Delete { id }) => delete_task(id),
    }
}

fn handle_timer(command: Option<TimerCommand>, freeze: bool, config: &Config) -> Result<(), String> {
    match command {
        Some(TimerCommand::Start { duration }) => start_timer(duration.as_deref(), &config.timer),
        Some(TimerCommand::Stop) => stop_timer(),
        None => {
            if config.timer.auto_start && !has_active_timer_state()? {
                start_timer(None, &config.timer)?;
            }
            run_live_loop(Duration::from_secs(1), freeze, timer_dashboard_output)
        }
    }
}

fn handle_stopwatch(command: StopwatchCommand) -> Result<(), String> {
    match command {
        StopwatchCommand::Start => start_stopwatch(),
        StopwatchCommand::Stop => stop_stopwatch(),
    }
}

fn handle_note(command: NoteCommand) -> Result<(), String> {
    match command {
        NoteCommand::Add { text } => add_note(&text.join(" ")),
        NoteCommand::List => list_notes(),
    }
}

fn handle_remind(config: &Config, time: Option<&str>, message: Option<&str>) -> Result<(), String> {
    let time = time
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(config.reminders.default_duration.as_str());
    add_reminder(time, message)?;
    println!("Note: reminders trigger while the dashboard is running.");
    live_dashboard(config, false)
}

fn handle_dashboard(
    config: &mut Config,
    command: Option<DashboardCommand>,
    freeze: bool,
) -> Result<(), String> {
    match command {
        None => live_dashboard(config, freeze),
        Some(DashboardCommand::Config) => {
            println!("Refresh interval: {}s", config.dashboard.refresh_interval);
            println!("Compact mode: {}", config.dashboard.compact_mode);
            println!("Freeze mode: {}", config.dashboard.freeze);
            println!("Enabled widgets: {}", config.dashboard.widgets.join(", "));
            Ok(())
        }
        Some(DashboardCommand::Reset) => {
            config.dashboard = crate::config::DashboardConfig::default();
            config.save()?;
            println!("Dashboard configuration reset.");
            Ok(())
        }
        Some(DashboardCommand::Notes { command }) => handle_dashboard_notes(command),
    }
}

fn handle_dashboard_notes(command: DashboardNotesCommand) -> Result<(), String> {
    match command {
        DashboardNotesCommand::Show => {
            list_notes()
        }
        DashboardNotesCommand::Set { text } => {
            let body = text.join(" ").trim().to_string();
            if body.is_empty() {
                return Err("Dashboard notes cannot be empty.".to_string());
            }
            replace_notes_with_single_entry(&body)?;
            println!("Dashboard notes updated.");
            Ok(())
        }
        DashboardNotesCommand::Clear => {
            clear_notes()?;
            println!("Dashboard notes cleared.");
            Ok(())
        }
    }
}

fn print_completions(shell: CompletionCommand) {
    let mut command = Cli::command();
    let mut stdout = std::io::stdout();
    match shell {
        CompletionCommand::Bash => generate(Shell::Bash, &mut command, "tinfo", &mut stdout),
        CompletionCommand::Zsh => generate(Shell::Zsh, &mut command, "tinfo", &mut stdout),
        CompletionCommand::Fish => generate(Shell::Fish, &mut command, "tinfo", &mut stdout),
        CompletionCommand::PowerShell => {
            generate(Shell::PowerShell, &mut command, "tinfo", &mut stdout)
        }
        CompletionCommand::Install | CompletionCommand::Uninstall | CompletionCommand::Status => {}
    }
}

fn handle_completion(command: CompletionCommand) {
    match command {
        CompletionCommand::Install => {
            if let Err(err) = install_completion_for_current_shell() {
                eprintln!("{err}");
                process::exit(1);
            }
        }
        CompletionCommand::Uninstall => {
            if let Err(err) = uninstall_completion_for_current_shell() {
                eprintln!("{err}");
                process::exit(1);
            }
        }
        CompletionCommand::Status => {
            if let Err(err) = completion_status_for_current_shell() {
                eprintln!("{err}");
                process::exit(1);
            }
        }
        shell => print_completions(shell),
    }
}

pub(crate) fn install_completion_for_current_shell() -> Result<(), String> {
    let (shell_cmd, path) = completion_install_target()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create completion directory: {err}"))?;
    }
    let mut command = Cli::command();
    let mut buffer = Vec::new();
    match shell_cmd {
        CompletionCommand::Bash => generate(Shell::Bash, &mut command, "tinfo", &mut buffer),
        CompletionCommand::Zsh => generate(Shell::Zsh, &mut command, "tinfo", &mut buffer),
        CompletionCommand::Fish => generate(Shell::Fish, &mut command, "tinfo", &mut buffer),
        CompletionCommand::PowerShell => {
            generate(Shell::PowerShell, &mut command, "tinfo", &mut buffer)
        }
        CompletionCommand::Install | CompletionCommand::Uninstall | CompletionCommand::Status => {
            unreachable!()
        }
    }
    fs::write(&path, buffer).map_err(|err| format!("Failed to install completion: {err}"))?;
    println!("Installed completion to {}", path.display());
    Ok(())
}

pub(crate) fn uninstall_completion_for_current_shell() -> Result<(), String> {
    let (_, path) = completion_install_target()?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("Failed to remove completion {}: {err}", path.display()))?;
        println!("Removed completion from {}", path.display());
    } else {
        println!("No installed completion found at {}", path.display());
    }
    Ok(())
}

pub(crate) fn completion_status_for_current_shell() -> Result<(), String> {
    let (shell, path) = completion_install_target()?;
    println!("Shell: {:?}", shell);
    println!("Path: {}", path.display());
    println!("Installed: {}", path.exists());
    Ok(())
}

fn completion_install_target() -> Result<(CompletionCommand, PathBuf), String> {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let home =
        std::env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    if shell.ends_with("zsh") {
        Ok((
            CompletionCommand::Zsh,
            PathBuf::from(&home).join(".zsh/completions/_tinfo"),
        ))
    } else if shell.ends_with("fish") {
        Ok((
            CompletionCommand::Fish,
            PathBuf::from(&home).join(".config/fish/completions/tinfo.fish"),
        ))
    } else if shell.to_ascii_lowercase().contains("powershell") {
        Ok((
            CompletionCommand::PowerShell,
            PathBuf::from(&home)
                .join("Documents")
                .join("PowerShell")
                .join("Completions")
                .join("tinfo.ps1"),
        ))
    } else {
        Ok((
            CompletionCommand::Bash,
            PathBuf::from(&home).join(".local/share/bash-completion/completions/tinfo"),
        ))
    }
}

fn handle_plugin(command: PluginCommand) -> Result<(), String> {
    match command {
        PluginCommand::List => list_plugins(),
        PluginCommand::Search { query } => {
            let query = if query.is_empty() {
                None
            } else {
                Some(query.join(" "))
            };
            search_plugins(query.as_deref())
        }
        PluginCommand::Browse { no_open } => plugin_browse(no_open),
        PluginCommand::Init { name } => init_plugin_template(name),
        PluginCommand::Keygen { output_dir } => plugin_keygen(output_dir),
        PluginCommand::Sign { file, key } => plugin_sign(&file, key.as_deref()),
        PluginCommand::Inspect => plugin_inspect(),
        PluginCommand::Test => plugin_test(),
        PluginCommand::Pack { from_dist } => plugin_pack(from_dist),
        PluginCommand::Install { name } => install_plugin(&name),
        PluginCommand::Trust { name } => set_plugin_trust(&name, true),
        PluginCommand::Untrust { name } => set_plugin_trust(&name, false),
        PluginCommand::Trusted => list_trusted_plugins(),
        PluginCommand::Info { name } => info_plugin(&name),
        PluginCommand::Verify => verify_plugins(),
        PluginCommand::Doctor => plugin_doctor(),
        PluginCommand::Lint => plugin_lint(),
        PluginCommand::PublishCheck => plugin_publish_check(),
        PluginCommand::Update { name } => update_plugin(&name),
        PluginCommand::UpgradeAll => upgrade_all_plugins(),
        PluginCommand::Remove { name } => remove_plugin(&name),
    }
}

fn handle_now(config: &Config, city: Option<String>) -> Result<String, String> {
    let client = WeatherClient::new();
    let city = resolve_city_for_now(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-now-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        config.cache.weather_ttl_secs,
        || client.current_weather(&city, config),
    )?;
    Ok(format_weather_report(&report, config.units))
}

fn handle_forecast(config: &Config, city: Option<String>) -> Result<String, String> {
    let client = WeatherClient::new();
    let city = resolve_city(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-forecast-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        config.cache.weather_ttl_secs,
        || client.forecast(&city, config),
    )?;
    Ok(format_forecast_report(&report, config.units))
}

fn handle_hourly(config: &Config, city: Option<String>) -> Result<String, String> {
    let client = WeatherClient::new();
    let city = resolve_city(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-hourly-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        config.cache.weather_ttl_secs,
        || client.hourly(&city, config),
    )?;
    Ok(format_hourly_report(&report, config.units))
}

fn handle_alerts(config: &Config, city: Option<String>) -> Result<String, String> {
    let client = WeatherClient::new();
    let city = resolve_city(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-alerts-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        config.cache.weather_ttl_secs,
        || client.alerts(&city, config),
    )?;
    Ok(format_alerts_report(&report))
}

fn handle_location(config: &mut Config, city: Option<String>) -> Result<(), String> {
    match city {
        Some(city) => {
            config.default_city = Some(city.clone());
            config.save()?;
            println!("Default location set to {city}.");
        }
        None => match &config.default_city {
            Some(city) => println!("Default location: {city}"),
            None => println!("No default location set."),
        },
    }

    Ok(())
}

fn handle_update() -> Result<(), String> {
    handle_update_inner(false)
}

fn handle_self_repair() -> Result<(), String> {
    println!("Running self-repair.");
    handle_update_inner(true)
}

fn handle_reinstall() -> Result<(), String> {
    println!("Reinstalling the latest Terminal Info release.");
    handle_update_inner(true)
}

fn handle_update_inner(force: bool) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|err| format!("Failed to locate current executable: {err}"))?;
    let install_dir = current_exe
        .parent()
        .ok_or_else(|| "Failed to determine installation directory.".to_string())?;

    println!("Checking current version ({})", env!("CARGO_PKG_VERSION"));

    if !directory_writable(install_dir) {
        println!("Terminal Info is installed in a system directory:");
        println!();
        println!("{}", current_exe.display());
        println!();
        println!("Updating requires elevated privileges.");
        println!();
        println!("Please run:");
        println!();
        println!("sudo tinfo update");
        return Ok(());
    }

    println!("Checking latest version");
    let release = fetch_terminal_info_release()?;
    if !force && release.tag_name == format!("v{}", env!("CARGO_PKG_VERSION")) {
        println!("Terminal Info is already up to date.");
        return Ok(());
    }

    println!("Downloading update");
    let temp_dir = prepare_update_dir()?;
    let archive_name = format!(
        "tinfo-{}.{}",
        update_target_triple(),
        update_archive_extension()
    );
    let archive_path = temp_dir.join(&archive_name);

    let result = (|| -> Result<(), String> {
        let asset = select_update_asset(&release.assets).ok_or_else(|| {
            format!(
                "No compatible release asset found for target '{}'.",
                update_target_triple()
            )
        })?;
        let signature_asset = select_update_signature_asset(&release.assets, &asset.name)
            .ok_or_else(|| format!("No minisign signature found for '{}'.", asset.name))?;
        download_to_path(&asset.browser_download_url, &archive_path)?;
        let signature = download_text(&signature_asset.browser_download_url, "update signature")?;
        if let Some(checksum_asset) = select_update_checksum_asset(&release.assets, &asset.name) {
            let expected_checksum =
                download_checksum(&checksum_asset.browser_download_url, &asset.name)?;
            verify_download_checksum(&archive_path, &expected_checksum)?;
        }
        verify_download_signature(&archive_path, &signature)?;

        println!("Extracting archive");
        let extracted_binary = extract_update_binary(&archive_path, &temp_dir)?;
        verify_extracted_binary(&extracted_binary)?;

        println!("Replacing binary");
        replace_binary_atomically(&extracted_binary, &current_exe)
    })();

    let _ = fs::remove_dir_all(&temp_dir);

    match result {
        Ok(()) => {
            println!("Updated Terminal Info to {}.", release.tag_name);
            Ok(())
        }
        Err(err) if is_permission_denied(&err) => {
            println!("Failed to update Terminal Info: permission denied.");
            println!();
            println!("The current installation location is:");
            println!();
            println!("{}", current_exe.display());
            println!();
            println!("Please run the update command with elevated privileges:");
            println!();
            println!("sudo tinfo update");
            Ok(())
        }
        Err(err) => Err(format!("Failed to update Terminal Info: {err}")),
    }
}

fn is_permission_denied(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("permission denied") || lower.contains("os error 13")
}

fn handle_uninstall(keep_data: bool) -> Result<(), String> {
    let binary_path = find_tinfo_binary()?;
    let data_path = terminal_info_data_dir()?;

    println!("Terminal Info will be removed.");
    println!();
    println!("Binary:");
    println!("  {}", binary_path.display());
    println!();
    println!("User data:");
    println!("  {}", data_path.display());
    println!();
    print!("Continue? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|err| format!("Failed to flush stdout: {err}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|err| format!("Failed to read confirmation: {err}"))?;

    if !matches!(input.trim(), "y" | "Y" | "yes" | "YES" | "Yes") {
        println!("Uninstall cancelled.");
        return Ok(());
    }

    fs::remove_file(&binary_path).map_err(|err| {
        if is_permission_denied(&err.to_string()) {
            format!(
                "Failed to remove {}: permission denied.",
                binary_path.display()
            )
        } else {
            format!("Failed to remove {}: {err}", binary_path.display())
        }
    })?;

    if !keep_data && data_path.exists() {
        fs::remove_dir_all(&data_path)
            .map_err(|err| format!("Failed to remove {}: {err}", data_path.display()))?;
    }

    println!("Terminal Info successfully removed.");
    Ok(())
}

#[cfg(test)]
mod dashboard_mode_tests {
    use super::*;

    #[test]
    fn dashboard_freeze_flag_overrides_live_and_config() {
        let cli = Cli {
            plain: false,
            compact: false,
            color: false,
            json: false,
            freeze: true,
            live: true,
            command: None,
        };
        let mut config = Config::default();
        config.dashboard.freeze = false;
        assert!(resolve_dashboard_freeze(&cli, &config));
    }

    #[test]
    fn dashboard_live_flag_overrides_config_freeze() {
        let cli = Cli {
            plain: false,
            compact: false,
            color: false,
            json: false,
            freeze: false,
            live: true,
            command: None,
        };
        let mut config = Config::default();
        config.dashboard.freeze = true;
        assert!(!resolve_dashboard_freeze(&cli, &config));
    }

    #[test]
    fn dashboard_config_freeze_applies_without_flags() {
        let cli = Cli {
            plain: false,
            compact: false,
            color: false,
            json: false,
            freeze: false,
            live: false,
            command: None,
        };
        let mut config = Config::default();
        config.dashboard.freeze = true;
        assert!(resolve_dashboard_freeze(&cli, &config));
    }
}

fn find_tinfo_binary() -> Result<PathBuf, String> {
    let output = process::Command::new("which")
        .arg("tinfo")
        .output()
        .map_err(|err| format!("Failed to locate tinfo with `which`: {err}"))?;

    if !output.status.success() {
        return Err("Could not locate tinfo in PATH.".to_string());
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let binary_path = PathBuf::from(path);
    validate_binary_path(&binary_path)?;
    Ok(binary_path)
}

fn validate_binary_path(path: &Path) -> Result<(), String> {
    let home =
        std::env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    let allowed_local = PathBuf::from(home).join(".local").join("bin").join("tinfo");
    let allowed_global = PathBuf::from("/usr/local/bin/tinfo");

    if path == allowed_global || path == allowed_local {
        Ok(())
    } else {
        Err(format!(
            "Refusing to remove unexpected binary path: {}",
            path.display()
        ))
    }
}

fn terminal_info_data_dir() -> Result<PathBuf, String> {
    let home =
        std::env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    Ok(PathBuf::from(home).join(".terminal-info"))
}

#[derive(Deserialize)]
struct TerminalInfoRelease {
    tag_name: String,
    assets: Vec<TerminalInfoAsset>,
}

#[derive(Deserialize)]
struct TerminalInfoAsset {
    name: String,
    browser_download_url: String,
}

fn fetch_terminal_info_release() -> Result<TerminalInfoRelease, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?
        .get("https://api.github.com/repos/T-1234567890/terminal-info/releases/latest")
        .header("User-Agent", format!("tinfo/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .map_err(|err| format!("Failed to check latest version: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to check latest version: {err}"))?
        .json()
        .map_err(|err| format!("Failed to parse release metadata: {err}"))
}

fn prepare_update_dir() -> Result<PathBuf, String> {
    #[cfg(unix)]
    let base = PathBuf::from("/tmp").join("tinfo-update");

    #[cfg(not(unix))]
    let base = std::env::temp_dir().join("tinfo-update");

    fs::create_dir_all(&base)
        .map_err(|err| format!("Failed to create temporary update directory: {err}"))?;

    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    let path = base.join(unique);
    fs::create_dir_all(&path)
        .map_err(|err| format!("Failed to create temporary update directory: {err}"))?;
    Ok(path)
}

fn update_target_triple() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    {
        "unknown-target"
    }
}

fn update_archive_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "zip"
    }

    #[cfg(not(target_os = "windows"))]
    {
        "tar.gz"
    }
}

fn select_update_asset(assets: &[TerminalInfoAsset]) -> Option<&TerminalInfoAsset> {
    let target = update_target_triple();
    let extension = update_archive_extension();
    let exact = format!("tinfo-{target}.{extension}");

    assets
        .iter()
        .find(|asset| asset.name == exact)
        .or_else(|| assets.iter().find(|asset| asset.name.contains(target)))
}

fn select_update_signature_asset<'a>(
    assets: &'a [TerminalInfoAsset],
    archive_name: &str,
) -> Option<&'a TerminalInfoAsset> {
    let signature_name = format!("{archive_name}.minisig");
    assets.iter().find(|asset| asset.name == signature_name)
}

fn select_update_checksum_asset<'a>(
    assets: &'a [TerminalInfoAsset],
    archive_name: &str,
) -> Option<&'a TerminalInfoAsset> {
    let checksum_name = format!("{archive_name}.sha256");
    assets.iter().find(|asset| asset.name == checksum_name)
}

fn download_to_path(url: &str, destination: &Path) -> Result<(), String> {
    let mut response = Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?
        .get(url)
        .header("User-Agent", format!("tinfo/{}", env!("CARGO_PKG_VERSION")))
        .header(ACCEPT_ENCODING, "identity")
        .send()
        .map_err(|err| format!("Failed to download update: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download update: {err}"))?;

    let mut file = File::create(destination)
        .map_err(|err| format!("Failed to create update archive: {err}"))?;
    response
        .copy_to(&mut file)
        .map_err(|err| format!("Failed to read update archive: {err}"))?;
    Ok(())
}

fn download_text(url: &str, label: &str) -> Result<String, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?
        .get(url)
        .header("User-Agent", format!("tinfo/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .map_err(|err| format!("Failed to download {label}: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download {label}: {err}"))?
        .text()
        .map_err(|err| format!("Failed to read {label}: {err}"))
}

fn download_checksum(url: &str, archive_name: &str) -> Result<String, String> {
    let body = Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?
        .get(url)
        .header("User-Agent", format!("tinfo/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .map_err(|err| format!("Failed to download update checksum: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download update checksum: {err}"))?
        .text()
        .map_err(|err| format!("Failed to read update checksum: {err}"))?;

    parse_checksum_file(&body, archive_name)
}

fn parse_checksum_file(contents: &str, archive_name: &str) -> Result<String, String> {
    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(checksum), Some(name)) = (parts.next(), parts.next()) {
            let normalized = name.trim_start_matches('*');
            if normalized == archive_name {
                validate_sha256_hex(checksum)?;
                return Ok(checksum.to_ascii_lowercase());
            }
        }
    }

    Err(format!(
        "Checksum file did not contain an entry for '{}'.",
        archive_name
    ))
}

fn verify_download_checksum(path: &Path, expected: &str) -> Result<(), String> {
    let bytes =
        fs::read(path).map_err(|err| format!("Failed to read downloaded archive: {err}"))?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err("Checksum verification failed for update archive.".to_string());
    }
    Ok(())
}

fn verify_download_signature(path: &Path, signature: &str) -> Result<(), String> {
    let bytes =
        fs::read(path).map_err(|err| format!("Failed to read downloaded archive: {err}"))?;
    let key = PublicKey::from_base64(terminal_info_update_public_key()?)
        .map_err(|err| format!("invalid embedded minisign public key: {err}"))?;
    let sig =
        Signature::decode(signature).map_err(|err| format!("invalid minisign signature: {err}"))?;
    key.verify(&bytes, &sig, false)
        .map_err(|err| format!("minisign verification failed: {err}"))
}

fn terminal_info_update_public_key() -> Result<&'static str, String> {
    include_str!("../keys/minisign.pub")
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("untrusted comment:"))
        .ok_or_else(|| "missing minisign public key in keys/minisign.pub".to_string())
}

fn extract_update_binary(archive_path: &Path, temp_dir: &Path) -> Result<PathBuf, String> {
    let destination = temp_dir.join(current_binary_name());

    #[cfg(target_os = "windows")]
    {
        let file = File::open(archive_path)
            .map_err(|err| format!("Failed to read update archive: {err}"))?;
        let mut archive =
            ZipArchive::new(file).map_err(|err| format!("Failed to read zip archive: {err}"))?;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|err| format!("Failed to read zip entry: {err}"))?;
            let name = Path::new(entry.name())
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if name == current_binary_name() {
                let mut output = File::create(&destination)
                    .map_err(|err| format!("Failed to create extracted binary: {err}"))?;
                io::copy(&mut entry, &mut output)
                    .map_err(|err| format!("Failed to extract update binary: {err}"))?;
                return Ok(destination);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let file = File::open(archive_path)
            .map_err(|err| format!("Failed to read update archive: {err}"))?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        let entries = archive
            .entries()
            .map_err(|err| format!("Failed to read tar archive: {err}"))?;
        for entry_result in entries {
            let mut entry =
                entry_result.map_err(|err| format!("Failed to read tar entry: {err}"))?;
            let entry_path = entry
                .path()
                .map_err(|err| format!("Failed to read tar entry path: {err}"))?
                .into_owned();
            if entry_path.file_name().and_then(|value| value.to_str())
                == Some(current_binary_name())
            {
                let mut output = File::create(&destination)
                    .map_err(|err| format!("Failed to create extracted binary: {err}"))?;
                io::copy(&mut entry, &mut output)
                    .map_err(|err| format!("Failed to extract update binary: {err}"))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&destination)
                        .map_err(|err| format!("Failed to read extracted binary metadata: {err}"))?
                        .permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&destination, perms).map_err(|err| {
                        format!("Failed to set extracted binary permissions: {err}")
                    })?;
                }
                return Ok(destination);
            }
        }
    }

    Err("Update archive did not contain a tinfo binary.".to_string())
}

fn verify_extracted_binary(path: &Path) -> Result<(), String> {
    let metadata =
        fs::metadata(path).map_err(|err| format!("Failed to verify extracted binary: {err}"))?;
    if metadata.len() == 0 {
        return Err("Extracted update binary is empty.".to_string());
    }
    Ok(())
}

fn replace_binary_atomically(new_binary: &Path, current_exe: &Path) -> Result<(), String> {
    let backup = current_exe.with_file_name(format!(
        ".tinfo-backup-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    ));

    fs::rename(current_exe, &backup).map_err(|err| err.to_string())?;
    match fs::rename(new_binary, current_exe) {
        Ok(()) => {
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(err) => {
            let _ = fs::rename(&backup, current_exe);
            Err(err.to_string())
        }
    }
}

fn current_binary_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "tinfo.exe"
    }

    #[cfg(not(target_os = "windows"))]
    {
        "tinfo"
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn validate_sha256_hex(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("checksum must be a 64-character SHA-256 hex string.".to_string());
    }
    Ok(())
}

fn directory_writable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => return false,
        };
        let mode = metadata.mode();

        if mode & 0o002 != 0 {
            return true;
        }

        if let Some(uid) = current_unix_uid() {
            if metadata.uid() == uid && mode & 0o200 != 0 {
                return true;
            }
        }

        false
    }

    #[cfg(not(unix))]
    {
        fs::metadata(path)
            .map(|metadata| !metadata.permissions().readonly())
            .unwrap_or(false)
    }
}

#[cfg(unix)]
fn current_unix_uid() -> Option<u32> {
    let output = process::Command::new("id").arg("-u").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn resolve_city(
    config: &Config,
    city: Option<String>,
    client: &WeatherClient,
) -> Result<String, String> {
    city.map(|value| config.resolve_location_alias(&value).to_string())
        .or_else(|| config.configured_location().map(str::to_string))
        .or_else(|| {
            if config.uses_auto_location() {
                client.detect_city_by_ip()
            } else {
                None
            }
        })
        .ok_or_else(|| {
            "No city provided. Use `tinfo config location <city>` to set a default location."
                .to_string()
        })
}

fn resolve_city_for_now(
    config: &Config,
    city: Option<String>,
    client: &WeatherClient,
) -> Result<String, String> {
    if let Some(city) = city {
        return Ok(config.resolve_location_alias(&city).to_string());
    }

    if let Some(city) = config.configured_location() {
        return Ok(city.to_string());
    }

    client.detect_city_by_ip().ok_or_else(|| {
        "Unable to detect location. Use `tinfo config location <city>` to set a default location."
            .to_string()
    })
}

pub(crate) fn format_weather_report(report: &WeatherReport, units: Units) -> String {
    if crate::output::json_output() {
        return serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string());
    }
    if matches!(crate::output::output_mode(), OutputMode::Compact) {
        return format!(
            "{}: {}, {:.1}{}, wind {:.1} {}",
            report.location_name,
            report.summary,
            report.temperature,
            units.temperature_symbol(),
            report.wind_speed,
            units.wind_speed_unit()
        );
    }

    let mut rows = vec![
        ("Location", report.location_name.clone()),
        ("Weather", report.summary.clone()),
        (
            "Temperature",
            format!("{:.1}{}", report.temperature, units.temperature_symbol()),
        ),
        (
            "Wind",
            format!("{:.1} {}", report.wind_speed, units.wind_speed_unit()),
        ),
    ];
    if let Some(humidity) = report.humidity {
        rows.push(("Humidity", format!("{humidity}%")));
    }
    format_table(&format!("{} Weather", report.location_name), &rows)
}

fn format_forecast_report(report: &ForecastReport, units: Units) -> String {
    if crate::output::json_output() {
        return serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string());
    }
    if matches!(crate::output::output_mode(), OutputMode::Compact) {
        return report
            .days
            .iter()
            .map(|day| {
                format!(
                    "{}:{} {:.1}{} / {:.1}{}",
                    day.label,
                    day.summary,
                    day.high,
                    units.temperature_symbol(),
                    day.low,
                    units.temperature_symbol()
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
    }
    let rows = report
        .days
        .iter()
        .map(|day| {
            (
                day.label.as_str(),
                format!(
                    "{} {:.1}{} / {:.1}{}",
                    day.summary,
                    day.high,
                    units.temperature_symbol(),
                    day.low,
                    units.temperature_symbol()
                ),
            )
        })
        .collect::<Vec<_>>();
    format_table(&format!("{} Forecast", report.location_name), &rows)
}

fn format_hourly_report(report: &HourlyReport, units: Units) -> String {
    if crate::output::json_output() {
        return serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string());
    }
    if matches!(crate::output::output_mode(), OutputMode::Compact) {
        return report
            .hours
            .iter()
            .map(|hour| {
                format!(
                    "{}:{} {:.1}{}",
                    hour.label,
                    hour.summary,
                    hour.temperature,
                    units.temperature_symbol()
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
    }
    let rows = report
        .hours
        .iter()
        .map(|hour| {
            (
                hour.label.as_str(),
                format!(
                    "{} {:.1}{}",
                    hour.summary,
                    hour.temperature,
                    units.temperature_symbol()
                ),
            )
        })
        .collect::<Vec<_>>();
    format_table(&format!("{} Hourly", report.location_name), &rows)
}

fn format_alerts_report(report: &AlertsReport) -> String {
    if crate::output::json_output() {
        return serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string());
    }
    if matches!(crate::output::output_mode(), OutputMode::Compact) {
        if report.alerts.is_empty() {
            return "alerts=none".to_string();
        }
        return report
            .alerts
            .iter()
            .map(|alert| format!("{}:{}", alert.level, alert.message))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    let rows = if report.alerts.is_empty() {
        vec![("Alerts", "No active alerts.".to_string())]
    } else {
        report
            .alerts
            .iter()
            .map(|alert| (alert.level.as_str(), alert.message.clone()))
            .collect::<Vec<_>>()
    };
    format_table(&format!("{} Alerts", report.location_name), &rows)
}

fn format_table(title: &str, rows: &[(&str, String)]) -> String {
    let rows = rows
        .iter()
        .map(|(label, value)| ((*label).to_string(), value.clone()))
        .collect::<Vec<_>>();
    format_box_table(title, &rows)
}

fn weather_cache_get_or_fetch<T, F>(key: &str, ttl_secs: u64, fetch: F) -> Result<T, String>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone,
    F: FnOnce() -> Result<T, String>,
{
    if let Some(value) = read_cache(key, ttl_secs) {
        return Ok(value);
    }
    let value = fetch()?;
    let _ = write_cache(key, &value);
    Ok(value)
}

fn print_api_config(config: &Config) {
    println!("Provider: {}", config.provider_label());
    println!(
        "API key: {}",
        config
            .masked_api_key()
            .unwrap_or_else(|| "Not set".to_string())
    );
}
