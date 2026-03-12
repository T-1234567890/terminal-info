mod config;
mod weather;

use std::io::{self, Write};
use std::process;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

use crate::config::{ApiProvider, Config, Units};
use crate::weather::{ForecastReport, WeatherClient, WeatherReport};

#[derive(Parser, Debug)]
#[command(name = "tw", version, about = "Terminal Weather CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
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
    /// Manage provider and unit configuration
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Show or set API provider configuration
    Api {
        #[command(subcommand)]
        command: Option<ApiCommand>,
    },
    /// Set temperature units
    Units { units: UnitArg },
}

#[derive(Subcommand, Debug)]
enum ApiCommand {
    /// Save an API provider and key
    Set { provider: ProviderArg, key: String },
    /// Show the current provider configuration
    Show,
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
        Some(Command::Now { city }) => handle_now(&config, city),
        Some(Command::Forecast { city }) => handle_forecast(&config, city),
        Some(Command::Location { city }) => handle_location(&mut config, city),
        Some(Command::Config { command }) => handle_config(&mut config, command),
        None => {
            println!("{}", Cli::command().render_help());
            Ok(())
        }
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
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

fn handle_config(config: &mut Config, command: Option<ConfigCommand>) -> Result<(), String> {
    match command {
        Some(ConfigCommand::Api { command }) => match command {
            Some(ApiCommand::Set { provider, key }) => {
                config.provider = Some(match provider {
                    ProviderArg::Openweather => ApiProvider::OpenWeather,
                });
                config.api_key = Some(key);
                config.save()?;
                println!("API provider saved.");
            }
            Some(ApiCommand::Show) | None => print_api_config(config),
        },
        Some(ConfigCommand::Units { units }) => {
            config.units = match units {
                UnitArg::Metric => Units::Metric,
                UnitArg::Imperial => Units::Imperial,
            };
            config.save()?;
            println!("Units set to {}.", config.units.label());
        }
        None => run_config_menu(config)?,
    }

    Ok(())
}

fn resolve_city(config: &Config, city: Option<String>) -> Result<String, String> {
    city.or_else(|| config.default_city.clone()).ok_or_else(|| {
        "No city provided. Use `tw location <city>` to set a default location.".to_string()
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
        "Unable to detect location. Use `tw location <city>` to set a default location.".to_string()
    })
}

fn print_weather_report(report: &WeatherReport, units: Units) {
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

fn print_boxed_title(title: &str) {
    let inner_width = title.len() + 2;
    let border = format!("+{}+", "-".repeat(inner_width));
    println!("{border}");
    println!("| {title} |");
    println!("{border}");
}

fn print_config_summary(config: &Config) {
    println!("Provider: {}", config.provider_label());
    println!("Units: {}", config.units.label());
    println!(
        "Default location: {}",
        config.default_city.as_deref().unwrap_or("Not set")
    );
    println!(
        "API key: {}",
        config
            .masked_api_key()
            .unwrap_or_else(|| "Not set".to_string())
    );
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

fn run_config_menu(config: &mut Config) -> Result<(), String> {
    loop {
        println!();
        println!("Terminal Weather Configuration");
        println!();
        println!("1. Set default location");
        println!("2. Use IP location as default");
        println!("3. Remove default location");
        println!("4. Set units");
        println!("5. Show current config");
        println!("6. Exit");
        println!();

        let choice = prompt("Choose an option: ")?;
        match choice.trim() {
            "1" => set_default_location(config)?,
            "2" => set_ip_location(config)?,
            "3" => remove_default_location(config)?,
            "4" => set_units_interactive(config)?,
            "5" => print_config_summary(config),
            "6" => break,
            _ => println!("Invalid option. Enter 1-6."),
        }
    }

    Ok(())
}

fn set_default_location(config: &mut Config) -> Result<(), String> {
    let city = prompt("Enter city name: ")?;
    let city = city.trim();

    if city.is_empty() {
        println!("Location was not changed.");
        return Ok(());
    }

    config.default_city = Some(city.to_string());
    config.save()?;
    println!("Default location set to {city}.");
    Ok(())
}

fn set_ip_location(config: &mut Config) -> Result<(), String> {
    let client = WeatherClient::new();
    let Some(city) = client.detect_city_by_ip() else {
        println!("Unable to detect location. Use `tw location <city>` to set a default location.");
        return Ok(());
    };

    config.default_city = Some(city.clone());
    config.save()?;
    println!("Default location set to {city}.");
    Ok(())
}

fn remove_default_location(config: &mut Config) -> Result<(), String> {
    config.default_city = None;
    config.save()?;
    println!("Default location removed.");
    Ok(())
}

fn set_units_interactive(config: &mut Config) -> Result<(), String> {
    println!();
    println!("1. metric (Celsius)");
    println!("2. imperial (Fahrenheit)");

    let choice = prompt("Choose units: ")?;
    match choice.trim() {
        "1" => {
            config.units = Units::Metric;
            config.save()?;
            println!("Units set to metric.");
        }
        "2" => {
            config.units = Units::Imperial;
            config.save()?;
            println!("Units set to imperial.");
        }
        _ => println!("Invalid option. Units were not changed."),
    }

    Ok(())
}

fn prompt(message: &str) -> Result<String, String> {
    print!("{message}");
    io::stdout()
        .flush()
        .map_err(|err| format!("Failed to flush stdout: {err}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|err| format!("Failed to read input: {err}"))?;

    Ok(input)
}
