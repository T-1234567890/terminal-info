mod builtins;
mod cache;
mod config;
mod config_menu;
mod dashboard;
mod migration;
mod output;
mod plugin;
mod weather;

use std::fs;
use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
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
    run_config_doctor, run_diagnostic_all, run_diagnostic_full, run_diagnostic_network,
    run_diagnostic_system, run_ping, show_network_info, show_system_info,
};
use crate::cache::{read_cache, write_cache};
use crate::config::{ApiProvider, Config, Units};
use crate::config_menu::show_config_menu;
use crate::output::{OutputMode, set_json_output, set_output_mode};
use crate::plugin::{
    info_plugin, init_plugin_template, install_plugin, list_plugins, list_trusted_plugins,
    plugin_doctor, plugin_keygen, plugin_lint, plugin_publish_check, plugin_sign, remove_plugin,
    run_diagnostic_plugins, run_plugin, search_plugins, set_plugin_trust, update_plugin,
    upgrade_all_plugins, verify_plugins,
};
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
    #[arg(long, global = true)]
    freeze: bool,
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
    Network,
    /// Show system information
    System,
    /// Show local or global times
    Time {
        /// Optional city name
        city: Option<String>,
    },
    /// Run diagnostics
    Diagnostic {
        #[command(subcommand)]
        command: Option<DiagnosticCommand>,
    },
    /// Manage configuration
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
        command: DashboardCommand,
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
    /// Show or set the default location
    Location {
        /// City name to store
        city: Option<String>,
    },
    /// Show or set units
    Units { units: Option<UnitArg> },
    /// Show or set API provider configuration
    Api {
        #[command(subcommand)]
        command: Option<ApiCommand>,
    },
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
enum ApiCommand {
    /// Save an API provider and key
    Set { provider: ProviderArg, key: String },
    /// Show the current API configuration
    Show,
}

#[derive(Subcommand, Debug)]
enum PluginCommand {
    /// List installed plugins
    List,
    /// Search for plugins
    Search,
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
    /// Run a comprehensive diagnostic pass
    Full,
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

fn main() {
    let cli = Cli::parse();
    set_output_mode(resolve_output_mode(&cli));
    set_json_output(cli.json);
    let freeze = should_freeze(&cli);
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

    let result = match cli.command {
        Some(Command::Weather { command }) => handle_weather(&mut config, command, freeze),
        Some(Command::Ping { host }) => run_ping(host),
        Some(Command::Latency { host }) => run_ping(host),
        Some(Command::Network) => show_network_info(),
        Some(Command::System) => show_system_info(),
        Some(Command::Time { city }) => live_time(city, freeze),
        Some(Command::Diagnostic { command }) => handle_diagnostic(command),
        Some(Command::Config { command }) => handle_config(&mut config, command),
        Some(Command::Profile { command }) => handle_profile(&mut config, command),
        Some(Command::Completion { shell }) => {
            handle_completion(shell);
            Ok(())
        }
        Some(Command::Dashboard { command }) => handle_dashboard(&mut config, command),
        Some(Command::Plugin { command }) => handle_plugin(command),
        Some(Command::Update) => handle_update(),
        Some(Command::SelfRepair) => handle_self_repair(),
        Some(Command::Reinstall) => handle_reinstall(),
        Some(Command::Uninstall { keep_data }) => handle_uninstall(keep_data),
        Some(Command::External(args)) => handle_external(args),
        None => live_dashboard(&config, freeze),
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn should_freeze(cli: &Cli) -> bool {
    cli.freeze || cli.json || !io::stdout().is_terminal()
}

fn resolve_output_mode(cli: &Cli) -> OutputMode {
    if cli.plain {
        OutputMode::Plain
    } else if cli.compact {
        OutputMode::Compact
    } else {
        OutputMode::Color
    }
}

fn handle_diagnostic(command: Option<DiagnosticCommand>) -> Result<(), String> {
    match command {
        Some(DiagnosticCommand::Network) => run_diagnostic_network(),
        Some(DiagnosticCommand::System) => run_diagnostic_system(),
        Some(DiagnosticCommand::Plugins) => run_diagnostic_plugins(),
        Some(DiagnosticCommand::Full) => run_diagnostic_full(),
        None => run_diagnostic_all(),
    }
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
    run_live_loop(
        Duration::from_secs(effective_dashboard.refresh_interval.max(1)),
        freeze,
        || Ok(dashboard::dashboard_output(config)),
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
        Some(ConfigCommand::Reset) => {
            config.reset();
            config.save()?;
            println!("Configuration reset.");
            Ok(())
        }
        Some(ConfigCommand::Doctor) => run_config_doctor(config),
        None => show_config_menu(config),
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

fn handle_dashboard(config: &mut Config, command: DashboardCommand) -> Result<(), String> {
    match command {
        DashboardCommand::Config => {
            println!("Refresh interval: {}s", config.dashboard.refresh_interval);
            println!("Compact mode: {}", config.dashboard.compact_mode);
            println!("Enabled widgets: {}", config.dashboard.widgets.join(", "));
            Ok(())
        }
        DashboardCommand::Reset => {
            config.dashboard = crate::config::DashboardConfig::default();
            config.save()?;
            println!("Dashboard configuration reset.");
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

fn install_completion_for_current_shell() -> Result<(), String> {
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

fn uninstall_completion_for_current_shell() -> Result<(), String> {
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

fn completion_status_for_current_shell() -> Result<(), String> {
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
        PluginCommand::Search => search_plugins(),
        PluginCommand::Init { name } => init_plugin_template(name),
        PluginCommand::Keygen { output_dir } => plugin_keygen(output_dir),
        PluginCommand::Sign { file, key } => plugin_sign(&file, key.as_deref()),
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
    let content_width = rows
        .iter()
        .map(|(label, value)| label.len() + 2 + value.len())
        .max()
        .unwrap_or(0)
        .max(title.len());
    let top = format!("┌{}┐", "─".repeat(content_width + 2));
    let middle = format!("├{}┤", "─".repeat(content_width + 2));
    let bottom = format!("└{}┘", "─".repeat(content_width + 2));
    let mut lines = vec![
        top,
        format!("│ {} │", center_line(title, content_width)),
        middle,
    ];
    for (label, value) in rows {
        lines.push(format!(
            "│ {} │",
            pad_line(&format!("{label}: {value}"), content_width)
        ));
    }
    lines.push(bottom);
    format!("{}\n", lines.join("\n"))
}

fn pad_line(value: &str, width: usize) -> String {
    format!("{value:<width$}")
}

fn center_line(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(value.len());
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
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
