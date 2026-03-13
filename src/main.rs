mod builtins;
mod cache;
mod config;
mod config_menu;
mod dashboard;
mod output;
mod plugin;
mod weather;

use std::fs;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use flate2::read::GzDecoder;
use minisign_verify::{PublicKey, Signature};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tar::Archive;
#[cfg(target_os = "windows")]
use zip::ZipArchive;

use crate::builtins::{
    run_config_doctor, run_diagnostic_all, run_diagnostic_network, run_diagnostic_system, run_ping,
    show_network_info, show_system_info, show_time,
};
use crate::cache::{read_cache, write_cache};
use crate::config::{ApiProvider, Config, Units};
use crate::config_menu::show_config_menu;
use crate::dashboard::show_dashboard;
use crate::output::{OutputMode, set_json_output, set_output_mode};
use crate::plugin::{
    info_plugin, init_plugin_template, install_plugin, list_plugins, list_trusted_plugins,
    remove_plugin, run_diagnostic_plugins, run_plugin, search_plugins, set_plugin_trust,
    update_plugin, upgrade_all_plugins, verify_plugins,
};
use crate::weather::{AlertsReport, ForecastReport, HourlyReport, WeatherClient, WeatherReport};

const TERMINAL_INFO_UPDATE_PUBLIC_KEY: &str =
    "RWSd4eW2pwv6W8pQv4wKp0l6rXqWw0v0gkYfY8G8I7v7k5M2nQ8m7D3O";

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
        shell: CompletionCommand,
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
enum CompletionCommand {
    Bash,
    Zsh,
    Fish,
    Install,
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
            handle_completion(shell);
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
        Some(WeatherCommand::Hourly { city }) => handle_hourly(config, city),
        Some(WeatherCommand::Alerts { city }) => handle_alerts(config, city),
        Some(WeatherCommand::Location { city }) => handle_location(config, city),
        Some(WeatherCommand::External(args)) => {
            let Some((first, _)) = args.split_first() else {
                return handle_now(config, None);
            };
            handle_now(config, Some(first.clone()))
        }
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
    }
}

fn print_completions(shell: CompletionCommand) {
    let mut command = Cli::command();
    let mut stdout = std::io::stdout();
    match shell {
        CompletionCommand::Bash => generate(Shell::Bash, &mut command, "tinfo", &mut stdout),
        CompletionCommand::Zsh => generate(Shell::Zsh, &mut command, "tinfo", &mut stdout),
        CompletionCommand::Fish => generate(Shell::Fish, &mut command, "tinfo", &mut stdout),
        CompletionCommand::Install => {}
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
        shell => print_completions(shell),
    }
}

fn install_completion_for_current_shell() -> Result<(), String> {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let home =
        std::env::var("HOME").map_err(|_| "Failed to determine home directory.".to_string())?;
    let (shell_cmd, path) = if shell.ends_with("zsh") {
        (
            CompletionCommand::Zsh,
            PathBuf::from(&home).join(".zsh/completions/_tinfo"),
        )
    } else if shell.ends_with("fish") {
        (
            CompletionCommand::Fish,
            PathBuf::from(&home).join(".config/fish/completions/tinfo.fish"),
        )
    } else {
        (
            CompletionCommand::Bash,
            PathBuf::from(&home).join(".local/share/bash-completion/completions/tinfo"),
        )
    };

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
        CompletionCommand::Install => unreachable!(),
    }
    fs::write(&path, buffer).map_err(|err| format!("Failed to install completion: {err}"))?;
    println!("Installed completion to {}", path.display());
    Ok(())
}

fn handle_plugin(command: PluginCommand) -> Result<(), String> {
    match command {
        PluginCommand::List => list_plugins(),
        PluginCommand::Search => search_plugins(),
        PluginCommand::Init { name } => init_plugin_template(name),
        PluginCommand::Install { name } => install_plugin(&name),
        PluginCommand::Trust { name } => set_plugin_trust(&name, true),
        PluginCommand::Untrust { name } => set_plugin_trust(&name, false),
        PluginCommand::Trusted => list_trusted_plugins(),
        PluginCommand::Info { name } => info_plugin(&name),
        PluginCommand::Verify => verify_plugins(),
        PluginCommand::Update { name } => update_plugin(&name),
        PluginCommand::UpgradeAll => upgrade_all_plugins(),
        PluginCommand::Remove { name } => remove_plugin(&name),
    }
}

fn handle_now(config: &Config, city: Option<String>) -> Result<(), String> {
    let client = WeatherClient::new();
    let city = resolve_city_for_now(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-now-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        60,
        || client.current_weather(&city, config),
    )?;
    print_weather_report(&report, config.units);
    Ok(())
}

fn handle_forecast(config: &Config, city: Option<String>) -> Result<(), String> {
    let client = WeatherClient::new();
    let city = resolve_city(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-forecast-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        60,
        || client.forecast(&city, config),
    )?;
    print_forecast_report(&report, config.units);
    Ok(())
}

fn handle_hourly(config: &Config, city: Option<String>) -> Result<(), String> {
    let client = WeatherClient::new();
    let city = resolve_city(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-hourly-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        60,
        || client.hourly(&city, config),
    )?;
    print_hourly_report(&report, config.units);
    Ok(())
}

fn handle_alerts(config: &Config, city: Option<String>) -> Result<(), String> {
    let client = WeatherClient::new();
    let city = resolve_city(config, city, &client)?;
    let report = weather_cache_get_or_fetch(
        &format!(
            "weather-alerts-{}-{}",
            city.to_ascii_lowercase(),
            config.units.label()
        ),
        60,
        || client.alerts(&city, config),
    )?;
    print_alerts_report(&report);
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
    if release.tag_name == format!("v{}", env!("CARGO_PKG_VERSION")) {
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
        let checksum_asset = select_update_checksum_asset(&release.assets, &asset.name)
            .ok_or_else(|| format!("No checksum asset found for '{}'.", asset.name))?;

        download_to_path(&asset.browser_download_url, &archive_path)?;
        let signature = download_text(&signature_asset.browser_download_url, "update signature")?;
        let expected_checksum =
            download_checksum(&checksum_asset.browser_download_url, &asset.name)?;
        verify_download_checksum(&archive_path, &expected_checksum)?;
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
    let bytes = Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {err}"))?
        .get(url)
        .header("User-Agent", format!("tinfo/{}", env!("CARGO_PKG_VERSION")))
        .send()
        .map_err(|err| format!("Failed to download update: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download update: {err}"))?
        .bytes()
        .map_err(|err| format!("Failed to read update archive: {err}"))?;

    fs::write(destination, &bytes).map_err(|err| format!("Failed to write update archive: {err}"))
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
    let key = PublicKey::from_base64(TERMINAL_INFO_UPDATE_PUBLIC_KEY)
        .map_err(|err| format!("invalid embedded minisign public key: {err}"))?;
    let sig =
        Signature::decode(signature).map_err(|err| format!("invalid minisign signature: {err}"))?;
    key.verify(&bytes, &sig, false)
        .map_err(|err| format!("minisign verification failed: {err}"))
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

pub(crate) fn print_weather_report(report: &WeatherReport, units: Units) {
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }
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
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }
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

fn print_hourly_report(report: &HourlyReport, units: Units) {
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }
    print_boxed_title(&format!("{} Hourly", report.location_name));
    for hour in &report.hours {
        println!(
            "  {}: {}  {:.1}{}",
            hour.label,
            hour.summary,
            hour.temperature,
            units.temperature_symbol()
        );
    }
}

fn print_alerts_report(report: &AlertsReport) {
    if crate::output::json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }
    print_boxed_title(&format!("{} Alerts", report.location_name));
    if report.alerts.is_empty() {
        println!("  No active alerts.");
    } else {
        for alert in &report.alerts {
            println!("  {}: {}", alert.level, alert.message);
        }
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
