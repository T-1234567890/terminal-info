use dialoguer::{Confirm, Input, Password, Select, theme::ColorfulTheme};

use crate::config::{ApiProvider, Config, DashboardConfig, DefaultOutput, Units, config_path};
use crate::theme::{AccentColor, BorderStyle};
use crate::{
    completion_status_for_current_shell, handle_config_edit, handle_config_open,
    install_completion_for_current_shell, uninstall_completion_for_current_shell,
};
use crate::weather::WeatherClient;

pub fn show_config_menu(config: &mut Config) -> Result<(), String> {
    let theme = ColorfulTheme::default();

    loop {
        let items = [
            "Guided Setup",
            "Location",
            "Dashboard",
            "Widgets",
            "Default Output",
            "Theme",
            "Shell Completions",
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
            Some(0) => run_first_run_setup(config)?,
            Some(1) => show_location_menu(config, &theme)?,
            Some(2) => show_dashboard_menu(config, &theme)?,
            Some(3) => show_widgets_menu(config, &theme)?,
            Some(4) => show_output_menu(config, &theme)?,
            Some(5) => show_theme_menu(config, &theme)?,
            Some(6) => show_completion_menu(&theme)?,
            Some(7) => show_units_menu(config, &theme)?,
            Some(8) => show_api_menu(config, &theme)?,
            Some(9) => show_server_mode_menu(config, &theme)?,
            Some(10) => show_advanced_config_menu(config, &theme)?,
            Some(11) => {
                config.reset();
                config.save()?;
                println!("Configuration reset.");
            }
            Some(12) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

pub fn run_first_run_setup(config: &mut Config) -> Result<(), String> {
    let theme = ColorfulTheme::default();

    println!("Welcome to Terminal Info.");
    println!("This setup takes about a minute and configures the basics.");

    configure_location(config, &theme)?;
    configure_dashboard_preferences(config, &theme)?;
    configure_output_preference(config, &theme)?;
    maybe_install_completions(&theme)?;

    config.setup_complete = true;
    config.save()?;

    println!();
    println!("Setup complete.");
    println!(
        "Run `tinfo config` any time to adjust location, dashboard, output, theme, or shell integrations."
    );
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

fn configure_location(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = [
        "Detect location automatically",
        "Enter a location manually",
        "Skip for now",
    ];
    let selection = Select::with_theme(theme)
        .with_prompt("Choose a default location")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| format!("Failed to read location setup selection: {err}"))?;

    match selection {
        Some(0) => {
            let client = WeatherClient::new();
            match client.detect_city_by_ip() {
                Some(city) => {
                    config.default_city = Some(city.clone());
                    println!("Detected location: {city}");
                }
                None => {
                    println!("Unable to detect a location automatically.");
                }
            }
        }
        Some(1) => {
            let city: String = Input::with_theme(theme)
                .with_prompt("Default location")
                .interact_text()
                .map_err(|err| format!("Failed to read location: {err}"))?;
            let city = city.trim();
            if !city.is_empty() {
                config.default_city = Some(city.to_string());
            }
        }
        Some(2) | None => {}
        Some(_) => {}
    }

    config.save()
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

fn show_dashboard_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = [
            "Apply a dashboard preset",
            "Set refresh interval",
            "Toggle compact mode",
            "Toggle freeze mode",
            "Back",
        ];
        let selection = Select::with_theme(theme)
            .with_prompt("Dashboard Preferences")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read dashboard selection: {err}"))?;

        match selection {
            Some(0) => configure_dashboard_preferences(config, theme)?,
            Some(1) => {
                let refresh: u64 = Input::with_theme(theme)
                    .with_prompt("Refresh interval in seconds")
                    .default(config.dashboard.refresh_interval.max(1))
                    .interact_text()
                    .map_err(|err| format!("Failed to read refresh interval: {err}"))?;
                config.dashboard.refresh_interval = refresh.max(1);
                config.save()?;
                println!("Dashboard refresh interval set to {}s.", config.dashboard.refresh_interval);
            }
            Some(2) => {
                config.dashboard.compact_mode = !config.dashboard.compact_mode;
                config.save()?;
                println!("Dashboard compact mode: {}", config.dashboard.compact_mode);
            }
            Some(3) => {
                config.dashboard.freeze = !config.dashboard.freeze;
                config.save()?;
                println!("Dashboard freeze mode: {}", config.dashboard.freeze);
            }
            Some(4) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

const SUPPORTED_WIDGETS: &[&str] = &["weather", "time", "network", "system", "notes", "plugins"];

fn show_widgets_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = [
            "Show current widget order",
            "Toggle widgets",
            "Reset widget order",
            "Back",
        ];
        let selection = Select::with_theme(theme)
            .with_prompt("Widgets")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read widgets selection: {err}"))?;

        match selection {
            Some(0) => {
                println!("Dashboard widgets: {}", config.dashboard.widgets.join(", "));
            }
            Some(1) => {
                toggle_widgets(config, theme)?;
            }
            Some(2) => {
                config.dashboard.widgets = DashboardConfig::default().widgets;
                config.save()?;
                println!("Widget order reset to defaults.");
            }
            Some(3) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn toggle_widgets(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let current = SUPPORTED_WIDGETS
            .iter()
            .map(|widget| {
                if config.dashboard.widgets.iter().any(|value| value == widget) {
                    format!("[x] {widget}")
                } else {
                    format!("[ ] {widget}")
                }
            })
            .chain(std::iter::once("Back".to_string()))
            .collect::<Vec<_>>();

        let selection = Select::with_theme(theme)
            .with_prompt("Toggle dashboard widgets")
            .items(&current)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read widget toggle selection: {err}"))?;

        match selection {
            Some(index) if index < SUPPORTED_WIDGETS.len() => {
                let widget = SUPPORTED_WIDGETS[index];
                if let Some(position) = config
                    .dashboard
                    .widgets
                    .iter()
                    .position(|value| value == widget)
                {
                    config.dashboard.widgets.remove(position);
                    println!("Disabled widget '{widget}'.");
                } else {
                    config.dashboard.widgets.push(widget.to_string());
                    println!("Enabled widget '{widget}'.");
                }
                config.save()?;
            }
            Some(_) | None => break,
        }
    }

    Ok(())
}

fn configure_dashboard_preferences(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = ["Standard", "Minimal", "Developer", "Keep current"];
    let selection = Select::with_theme(theme)
        .with_prompt("Choose a dashboard layout")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| format!("Failed to read dashboard preference: {err}"))?;

    match selection {
        Some(0) => {
            config.dashboard = DashboardConfig::default();
        }
        Some(1) => {
            config.dashboard.widgets = vec![
                "weather".to_string(),
                "time".to_string(),
                "plugins".to_string(),
            ];
            config.dashboard.refresh_interval = 2;
            config.dashboard.compact_mode = true;
        }
        Some(2) => {
            config.dashboard.widgets = vec![
                "weather".to_string(),
                "time".to_string(),
                "network".to_string(),
                "system".to_string(),
                "plugins".to_string(),
            ];
            config.dashboard.refresh_interval = 1;
            config.dashboard.compact_mode = false;
        }
        Some(3) | None => {}
        Some(_) => {}
    }

    config.save()
}

fn show_output_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    configure_output_preference(config, theme)
}

fn show_theme_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = ["Border Style", "Accent Color", "Unicode Borders", "Back"];
        let selection = Select::with_theme(theme)
            .with_prompt("Theme")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read theme selection: {err}"))?;

        match selection {
            Some(0) => configure_border_style(config, theme)?,
            Some(1) => configure_accent_color(config, theme)?,
            Some(2) => configure_unicode_preference(config, theme)?,
            Some(3) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn configure_border_style(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = ["sharp", "rounded", "Keep current"];
    let default = match config.theme.border_style {
        BorderStyle::Sharp => 0,
        BorderStyle::Rounded => 1,
    };
    let selection = Select::with_theme(theme)
        .with_prompt("Choose a border style")
        .items(&items)
        .default(default)
        .interact_opt()
        .map_err(|err| format!("Failed to read border style: {err}"))?;

    match selection {
        Some(0) => config.theme.border_style = BorderStyle::Sharp,
        Some(1) => config.theme.border_style = BorderStyle::Rounded,
        Some(2) | None => return Ok(()),
        Some(_) => {}
    }

    config.save()?;
    println!("Border style set to {}.", config.theme.border_style.label());
    Ok(())
}

fn configure_accent_color(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = [
        "auto",
        "cyan",
        "blue",
        "green",
        "magenta",
        "yellow",
        "red",
        "Keep current",
    ];
    let default = match config.theme.accent_color {
        AccentColor::Auto => 0,
        AccentColor::Cyan => 1,
        AccentColor::Blue => 2,
        AccentColor::Green => 3,
        AccentColor::Magenta => 4,
        AccentColor::Yellow => 5,
        AccentColor::Red => 6,
    };
    let selection = Select::with_theme(theme)
        .with_prompt("Choose an accent color")
        .items(&items)
        .default(default)
        .interact_opt()
        .map_err(|err| format!("Failed to read accent color: {err}"))?;

    match selection {
        Some(0) => config.theme.accent_color = AccentColor::Auto,
        Some(1) => config.theme.accent_color = AccentColor::Cyan,
        Some(2) => config.theme.accent_color = AccentColor::Blue,
        Some(3) => config.theme.accent_color = AccentColor::Green,
        Some(4) => config.theme.accent_color = AccentColor::Magenta,
        Some(5) => config.theme.accent_color = AccentColor::Yellow,
        Some(6) => config.theme.accent_color = AccentColor::Red,
        Some(7) | None => return Ok(()),
        Some(_) => {}
    }

    config.save()?;
    println!("Accent color set to {}.", config.theme.accent_color.label());
    Ok(())
}

fn configure_unicode_preference(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = ["Enable Unicode borders", "Use ASCII-only borders", "Keep current"];
    let default = if config.theme.unicode_enabled() { 0 } else { 1 };
    let selection = Select::with_theme(theme)
        .with_prompt("Choose border character set")
        .items(&items)
        .default(default)
        .interact_opt()
        .map_err(|err| format!("Failed to read Unicode preference: {err}"))?;

    match selection {
        Some(0) => config.theme.ascii_only = false,
        Some(1) => config.theme.ascii_only = true,
        Some(2) | None => return Ok(()),
        Some(_) => {}
    }

    config.save()?;
    println!(
        "Unicode borders {}.",
        if config.theme.unicode_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );
    Ok(())
}

fn configure_output_preference(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = ["color", "compact", "plain", "Keep current"];
    let default = match config.default_output {
        DefaultOutput::Color => 0,
        DefaultOutput::Compact => 1,
        DefaultOutput::Plain => 2,
    };
    let selection = Select::with_theme(theme)
        .with_prompt("Choose a default output mode")
        .items(&items)
        .default(default)
        .interact_opt()
        .map_err(|err| format!("Failed to read output preference: {err}"))?;

    match selection {
        Some(0) => config.default_output = DefaultOutput::Color,
        Some(1) => config.default_output = DefaultOutput::Compact,
        Some(2) => config.default_output = DefaultOutput::Plain,
        Some(3) | None => return Ok(()),
        Some(_) => {}
    }

    config.save()?;
    println!("Default output set to {}.", config.default_output.label());
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

fn show_completion_menu(theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = ["Install", "Status", "Uninstall", "Back"];
        let selection = Select::with_theme(theme)
            .with_prompt("Shell Completions")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read completion selection: {err}"))?;

        match selection {
            Some(0) => install_completion_for_current_shell()?,
            Some(1) => completion_status_for_current_shell()?,
            Some(2) => uninstall_completion_for_current_shell()?,
            Some(3) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn maybe_install_completions(theme: &ColorfulTheme) -> Result<(), String> {
    let confirmed = Confirm::with_theme(theme)
        .with_prompt("Install shell completions for the current shell?")
        .default(true)
        .interact()
        .map_err(|err| format!("Failed to read completion confirmation: {err}"))?;
    if !confirmed {
        return Ok(());
    }

    match install_completion_for_current_shell() {
        Ok(()) => Ok(()),
        Err(err) => {
            println!("Shell completion install skipped: {err}");
            Ok(())
        }
    }
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
        let items = ["Edit config file", "Open config file", "Show config path", "Back"];
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
