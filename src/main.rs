mod builtins;
mod config;
mod config_menu;
mod dashboard;
mod plugin;
mod weather;

use std::process;

use clap::{Parser, Subcommand, ValueEnum};

use crate::builtins::{run_doctor, run_ping, show_network_info, show_system_info, show_time};
use crate::config::{ApiProvider, Config, Units};
use crate::config_menu::show_config_menu;
use crate::dashboard::show_dashboard;
use crate::plugin::{install_plugin, list_plugins, remove_plugin, run_plugin, search_plugins};
use crate::weather::{ForecastReport, WeatherClient, WeatherReport};

#[derive(Parser, Debug)]
#[command(name = "tinfo", version, about = "Terminal Info CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Weather related commands
    Weather {
        #[command(subcommand)]
        command: WeatherCommand,
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
    /// Run simple diagnostics
    Doctor,
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    /// Download and install the latest released version of tinfo
    Update,
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
    /// Install a plugin
    Install { name: String },
    /// Remove a plugin
    Remove { name: String },
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
        Some(Command::Doctor) => run_doctor(),
        Some(Command::Config { command }) => handle_config(&mut config, command),
        Some(Command::Plugin { command }) => handle_plugin(command),
        Some(Command::Update) => handle_update(),
        Some(Command::External(args)) => handle_external(args),
        None => show_dashboard(&config),
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn handle_external(args: Vec<String>) -> Result<(), String> {
    let Some((command, remaining)) = args.split_first() else {
        return Ok(());
    };

    run_plugin(command, remaining)
}

fn handle_weather(config: &mut Config, command: WeatherCommand) -> Result<(), String> {
    match command {
        WeatherCommand::Now { city } => handle_now(config, city),
        WeatherCommand::Forecast { city } => handle_forecast(config, city),
        WeatherCommand::Location { city } => handle_location(config, city),
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

fn handle_plugin(command: PluginCommand) -> Result<(), String> {
    match command {
        PluginCommand::List => list_plugins(),
        PluginCommand::Search => search_plugins(),
        PluginCommand::Install { name } => install_plugin(&name),
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
    let city = resolve_city(config, city)?;
    let client = WeatherClient::new();
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
    let status = self_update::backends::github::Update::configure()
        .repo_owner("T-1234567890")
        .repo_name("terminal-info")
        .bin_name("tinfo")
        .show_download_progress(true)
        .current_version(self_update::cargo_crate_version!())
        .build()
        .map_err(|err| format!("Failed to prepare updater: {err}"))?
        .update()
        .map_err(|err| format!("Failed to update tinfo: {err}"))?;

    println!("Updated tinfo to {}.", status.version());
    Ok(())
}

fn resolve_city(config: &Config, city: Option<String>) -> Result<String, String> {
    city.or_else(|| config.default_city.clone()).ok_or_else(|| {
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

    if let Some(city) = config.default_city.clone() {
        return Ok(city);
    }

    client.detect_city_by_ip().ok_or_else(|| {
        "Unable to detect location. Use `tinfo config location <city>` to set a default location."
            .to_string()
    })
}

pub(crate) fn print_weather_report(report: &WeatherReport, units: Units) {
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
