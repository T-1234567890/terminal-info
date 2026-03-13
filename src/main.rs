mod builtins;
mod config;
mod config_menu;
mod dashboard;
mod output;
mod plugin;
mod weather;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};

use crate::builtins::{
    run_diagnostic_all, run_diagnostic_network, run_diagnostic_system, run_ping, show_network_info,
    show_system_info, show_time,
};
use crate::config::{ApiProvider, Config, Units};
use crate::config_menu::show_config_menu;
use crate::dashboard::show_dashboard;
use crate::output::{OutputMode, set_output_mode};
use crate::plugin::{
    init_plugin_template, install_plugin, list_plugins, remove_plugin, run_diagnostic_plugins,
    run_plugin, search_plugins, update_plugin, upgrade_all_plugins,
};
use crate::weather::{ForecastReport, WeatherClient, WeatherReport};

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
        shell: CompletionShell,
    },
    /// Manage plugins and scaffold new plugin projects
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    /// Download and install the latest released version of tinfo
    Update,
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
    /// Show or set the default location
    Location {
        /// City name to store as the default location
        city: Option<String>,
    },
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
}

#[derive(Subcommand, Debug)]
enum ProfileCommand {
    /// Use a profile
    Use { name: String },
    /// List profiles
    List,
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
    /// Install a plugin
    Install { name: String },
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
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
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
    let mut config = match Config::load_or_create() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    };

    let result = match cli.command {
        Some(Command::Weather { command }) => handle_weather(&mut config, command),
        Some(Command::Ping { host }) => run_ping(host),
        Some(Command::Network) => show_network_info(),
        Some(Command::System) => show_system_info(),
        Some(Command::Time { city }) => show_time(city),
        Some(Command::Diagnostic { command }) => handle_diagnostic(command),
        Some(Command::Config { command }) => handle_config(&mut config, command),
        Some(Command::Profile { command }) => handle_profile(&mut config, command),
        Some(Command::Completion { shell }) => {
            print_completions(shell);
            Ok(())
        }
        Some(Command::Plugin { command }) => handle_plugin(command),
        Some(Command::Update) => handle_update(),
        Some(Command::Uninstall { keep_data }) => handle_uninstall(keep_data),
        Some(Command::External(args)) => handle_external(args),
        None => show_dashboard(&config),
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
    }
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
        None => run_diagnostic_all(),
    }
}

fn handle_external(args: Vec<String>) -> Result<(), String> {
    let Some((command, remaining)) = args.split_first() else {
        return Ok(());
    };

    run_plugin(command, remaining)
}

fn handle_weather(config: &mut Config, command: Option<WeatherCommand>) -> Result<(), String> {
    match command {
        Some(WeatherCommand::Now { city }) => handle_now(config, city),
        Some(WeatherCommand::Forecast { city }) => handle_forecast(config, city),
        Some(WeatherCommand::Location { city }) => handle_location(config, city),
        None => handle_now(config, None),
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
    }
}

fn print_completions(shell: CompletionShell) {
    let mut command = Cli::command();
    let mut stdout = std::io::stdout();
    match shell {
        CompletionShell::Bash => generate(Shell::Bash, &mut command, "tinfo", &mut stdout),
        CompletionShell::Zsh => generate(Shell::Zsh, &mut command, "tinfo", &mut stdout),
        CompletionShell::Fish => generate(Shell::Fish, &mut command, "tinfo", &mut stdout),
    }
}

fn handle_plugin(command: PluginCommand) -> Result<(), String> {
    match command {
        PluginCommand::List => list_plugins(),
        PluginCommand::Search => search_plugins(),
        PluginCommand::Init { name } => init_plugin_template(name),
        PluginCommand::Install { name } => install_plugin(&name),
        PluginCommand::Update { name } => update_plugin(&name),
        PluginCommand::UpgradeAll => upgrade_all_plugins(),
        PluginCommand::Remove { name } => remove_plugin(&name),
    }
}

fn handle_now(config: &Config, city: Option<String>) -> Result<(), String> {
    let client = WeatherClient::new();
    let city = resolve_city_for_now(config, city, &client)?;
    let report = client.current_weather(&city, config)?;
    print_weather_report(&report, config.units);
    Ok(())
}

fn handle_forecast(config: &Config, city: Option<String>) -> Result<(), String> {
    let client = WeatherClient::new();
    let city = resolve_city(config, city, &client)?;
    let report = client.forecast(&city, config)?;
    print_forecast_report(&report, config.units);
    Ok(())
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
    let updater = self_update::backends::github::Update::configure()
        .repo_owner("T-1234567890")
        .repo_name("terminal-info")
        .bin_name("tinfo")
        .show_download_progress(true)
        .current_version(self_update::cargo_crate_version!())
        .build()
        .map_err(|err| format!("Failed to prepare updater: {err}"))?;

    let current_exe = std::env::current_exe().ok();
    let status = match updater.update() {
        Ok(status) => status,
        Err(err) => {
            if is_permission_denied(&err.to_string())
                && current_exe
                    .as_deref()
                    .is_some_and(|path| path.starts_with(Path::new("/usr/local/bin")))
            {
                println!(
                    "This installation requires administrator permission. Run: sudo tinfo update"
                );
                return Ok(());
            }

            return Err(format!("Failed to update tinfo: {err}"));
        }
    };

    println!("Updated tinfo to {}.", status.version());
    Ok(())
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

fn resolve_city(
    config: &Config,
    city: Option<String>,
    client: &WeatherClient,
) -> Result<String, String> {
    city.or_else(|| config.configured_location().map(str::to_string))
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
        return Ok(city);
    }

    if let Some(city) = config.configured_location() {
        return Ok(city.to_string());
    }

    client.detect_city_by_ip().ok_or_else(|| {
        "Unable to detect location. Use `tinfo config location <city>` to set a default location."
            .to_string()
    })
}

pub(crate) fn print_weather_report(report: &WeatherReport, units: Units) {
    match crate::output::output_mode() {
        OutputMode::Compact => {
            println!(
                "{}: {}, {:.1}{}, wind {:.1} {}",
                report.location_name,
                report.summary,
                report.temperature,
                units.temperature_symbol(),
                report.wind_speed,
                units.wind_speed_unit()
            );
        }
        OutputMode::Plain => {
            println!("{} Weather", report.location_name);
            println!("Weather: {}", report.summary);
            println!(
                "Temperature: {:.1}{}",
                report.temperature,
                units.temperature_symbol()
            );
            println!("Wind: {:.1} {}", report.wind_speed, units.wind_speed_unit());
            if let Some(humidity) = report.humidity {
                println!("Humidity: {humidity}%");
            }
        }
        OutputMode::Color => {
            print_boxed_title(&format!("{} Weather", report.location_name));
            println!("  {}", report.summary);
            println!(
                "  Temperature: {:.1}{}",
                report.temperature,
                units.temperature_symbol()
            );
            println!(
                "  Wind: {:.1} {}",
                report.wind_speed,
                units.wind_speed_unit()
            );
            if let Some(humidity) = report.humidity {
                println!("  Humidity: {humidity}%");
            }
        }
    }
}

fn print_forecast_report(report: &ForecastReport, units: Units) {
    print_boxed_title(&format!("{} Forecast", report.location_name));
    for day in &report.days {
        println!(
            "  {}: {}  {:.1}{} / {:.1}{}",
            day.label,
            day.summary,
            day.high,
            units.temperature_symbol(),
            day.low,
            units.temperature_symbol()
        );
    }
}

pub(crate) fn print_boxed_title(title: &str) {
    if matches!(
        crate::output::output_mode(),
        OutputMode::Plain | OutputMode::Compact
    ) {
        println!("{title}");
        return;
    }
    let inner_width = title.len() + 2;
    let border = format!("+{}+", "-".repeat(inner_width));
    println!("{border}");
    println!("| {title} |");
    println!("{border}");
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
