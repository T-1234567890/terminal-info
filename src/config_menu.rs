use dialoguer::{Confirm, Input, Password, Select, theme::ColorfulTheme};

use crate::config::{ApiProvider, Config, Units, config_path};
use crate::{handle_config_edit, handle_config_open};
use crate::weather::WeatherClient;

pub fn show_config_menu(config: &mut Config) -> Result<(), String> {
    let theme = ColorfulTheme::default();

    loop {
        let items = [
            "Location",
            "Units",
            "API Keys",
            "Server Mode",
            "Advanced and More Config",
            "Reset Config",
            "Exit",
        ];
        let selection = Select::with_theme(&theme)
            .with_prompt("Terminal Info Configuration")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read menu selection: {err}"))?;

        match selection {
            Some(0) => show_location_menu(config, &theme)?,
            Some(1) => show_units_menu(config, &theme)?,
            Some(2) => show_api_menu(config, &theme)?,
            Some(3) => show_server_mode_menu(config, &theme)?,
            Some(4) => show_advanced_config_menu(config, &theme)?,
            Some(5) => {
                config.reset();
                config.save()?;
                println!("Configuration reset.");
            }
            Some(6) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn show_location_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = ["Set location manually", "Use IP location", "Back"];
        let selection = Select::with_theme(theme)
            .with_prompt("Location")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read location selection: {err}"))?;

        match selection {
            Some(0) => {
                let city: String = Input::with_theme(theme)
                    .with_prompt("Location")
                    .interact_text()
                    .map_err(|err| format!("Failed to read location: {err}"))?;
                let city = city.trim();
                if city.is_empty() {
                    println!("Location was not changed.");
                } else {
                    config.default_city = Some(city.to_string());
                    config.save()?;
                    println!("Default location set to {city}.");
                }
            }
            Some(1) => {
                let client = WeatherClient::new();
                match client.detect_city_by_ip() {
                    Some(city) => {
                        config.default_city = Some(city.clone());
                        config.save()?;
                        println!("Default location set to {city}.");
                    }
                    None => println!("Unable to detect location."),
                }
            }
            Some(2) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn show_units_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = ["metric", "imperial", "Back"];
    let default = match config.units {
        Units::Metric => 0,
        Units::Imperial => 1,
    };

    let selection = Select::with_theme(theme)
        .with_prompt("Units")
        .items(&items)
        .default(default)
        .interact_opt()
        .map_err(|err| format!("Failed to read units selection: {err}"))?;

    match selection {
        Some(0) => {
            config.units = Units::Metric;
            config.save()?;
            println!("Units set to metric.");
        }
        Some(1) => {
            config.units = Units::Imperial;
            config.save()?;
            println!("Units set to imperial.");
        }
        Some(2) | None => {}
        Some(_) => {}
    }

    Ok(())
}

fn show_api_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = [
            "Set OpenWeather API key",
            "Clear API key",
            "Show current API config",
            "Back",
        ];
        let selection = Select::with_theme(theme)
            .with_prompt("API Keys")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read API selection: {err}"))?;

        match selection {
            Some(0) => {
                let key = Password::with_theme(theme)
                    .with_prompt("OpenWeather API key")
                    .allow_empty_password(true)
                    .interact()
                    .map_err(|err| format!("Failed to read API key: {err}"))?;

                if key.trim().is_empty() {
                    println!("API key was not changed.");
                } else {
                    config.provider = Some(ApiProvider::OpenWeather);
                    config.api_key = Some(key);
                    config.save()?;
                    println!("API key saved.");
                }
            }
            Some(1) => {
                config.api_key = None;
                config.provider = None;
                config.save()?;
                println!("API key cleared.");
            }
            Some(2) => {
                println!("Provider: {}", config.provider_label());
                println!(
                    "API key: {}",
                    config
                        .masked_api_key()
                        .unwrap_or_else(|| "Not set".to_string())
                );
            }
            Some(3) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn show_server_mode_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = ["Enable", "Disable", "Status", "Back"];
        let selection = Select::with_theme(theme)
            .with_prompt("Server Mode")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read server mode selection: {err}"))?;

        match selection {
            Some(0) => {
                if config.server_mode {
                    println!("Server mode is already enabled.");
                    continue;
                }
                println!(
                    "Server mode is designed for servers or VPS environments and is not recommended for regular desktop computers."
                );
                let confirmed = Confirm::with_theme(theme)
                    .with_prompt("Enable server mode?")
                    .default(false)
                    .interact()
                    .map_err(|err| format!("Failed to read confirmation: {err}"))?;
                if confirmed {
                    config.server_mode = true;
                    config.save()?;
                    println!("Server mode enabled.");
                } else {
                    println!("Server mode was not changed.");
                }
            }
            Some(1) => {
                config.server_mode = false;
                config.save()?;
                println!("Server mode disabled.");
            }
            Some(2) => {
                println!(
                    "Server mode: {}",
                    if config.server_mode {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            Some(3) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn show_advanced_config_menu(config: &Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = ["RunEdit", "RunOpen", "Show config path", "Back"];
        let selection = Select::with_theme(theme)
            .with_prompt("Advanced and More Config")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read advanced config selection: {err}"))?;

        match selection {
            Some(0) => handle_config_edit(config)?,
            Some(1) => handle_config_open(config)?,
            Some(2) => {
                let path = config_path()?;
                println!("Config file:");
                println!("{}", path.display());
            }
            Some(3) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}
