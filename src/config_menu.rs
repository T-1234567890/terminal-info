use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use dialoguer::{Confirm, Input, Password, Select, theme::ColorfulTheme};
use terminal_info::ai::chat::ProviderKind;
use terminal_info::ai::secret::{SecretStore, SystemSecretStore, remove_provider_key};

use crate::config::{
    AiApprovalMode, ApiProvider, Config, DashboardConfig, DashboardLayout, DefaultOutput,
    TaskSortOrder, TimerWidgetMode, Units, config_path,
};
use crate::dashboard::{
    WidgetDefinition, available_widget_definitions, default_enabled_widget_names,
};
use crate::theme::{AccentColor, BorderStyle};
use crate::weather::WeatherClient;
use crate::{
    completion_status_for_current_shell, handle_config_edit, handle_config_open,
    install_completion_for_current_shell, uninstall_completion_for_current_shell,
};

pub fn show_config_menu(config: &mut Config) -> Result<(), String> {
    let theme = ColorfulTheme::default();

    loop {
        let items = [
            "Guided Setup",
            "Location",
            "Dashboard",
            "Widgets",
            "Tasks",
            "Notes",
            "Timer",
            "Reminders",
            "Default Output",
            "Theme",
            "Shell Completions",
            "Units",
            "API Keys",
            "AI Features",
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
            Some(3) => show_widgets_menu(config)?,
            Some(4) => show_tasks_menu(config, &theme)?,
            Some(5) => show_notes_menu(config, &theme)?,
            Some(6) => show_timer_menu(config, &theme)?,
            Some(7) => show_reminders_menu(config, &theme)?,
            Some(8) => show_output_menu(config, &theme)?,
            Some(9) => show_theme_menu(config, &theme)?,
            Some(10) => show_completion_menu(&theme)?,
            Some(11) => show_units_menu(config, &theme)?,
            Some(12) => show_api_menu(config, &theme)?,
            Some(13) => show_ai_features_menu(config, &theme)?,
            Some(14) => show_server_mode_menu(config, &theme)?,
            Some(15) => show_advanced_config_menu(config, &theme)?,
            Some(16) => {
                config.reset();
                config.save()?;
                println!("Configuration reset.");
            }
            Some(17) | None => break,
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

fn show_tasks_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = [
            "Toggle show completed tasks",
            "Set default sort order",
            "Set max tasks displayed in widget",
            "Toggle auto-remove completed tasks",
            "Back",
        ];
        let selection = Select::with_theme(theme)
            .with_prompt("Tasks")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read tasks selection: {err}"))?;

        match selection {
            Some(0) => {
                config.tasks.show_completed = !config.tasks.show_completed;
                config.save()?;
                println!("Show completed tasks: {}", config.tasks.show_completed);
            }
            Some(1) => {
                let items = ["created", "status", "Keep current"];
                let default = match config.tasks.sort_order {
                    TaskSortOrder::Created => 0,
                    TaskSortOrder::Status => 1,
                };
                let selection = Select::with_theme(theme)
                    .with_prompt("Default task sort order")
                    .items(&items)
                    .default(default)
                    .interact_opt()
                    .map_err(|err| format!("Failed to read task sort order: {err}"))?;
                match selection {
                    Some(0) => config.tasks.sort_order = TaskSortOrder::Created,
                    Some(1) => config.tasks.sort_order = TaskSortOrder::Status,
                    _ => {}
                }
                config.save()?;
            }
            Some(2) => {
                let max_display: usize = Input::with_theme(theme)
                    .with_prompt("Max tasks displayed in widget")
                    .default(config.tasks.max_display.max(1))
                    .interact_text()
                    .map_err(|err| format!("Failed to read task widget limit: {err}"))?;
                config.tasks.max_display = max_display.max(1);
                config.save()?;
            }
            Some(3) => {
                config.tasks.auto_remove_completed = !config.tasks.auto_remove_completed;
                config.save()?;
                println!(
                    "Auto-remove completed tasks: {}",
                    config.tasks.auto_remove_completed
                );
            }
            Some(4) | None => break,
            Some(_) => {}
        }
    }
    Ok(())
}

fn show_notes_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = ["Set max notes stored", "Toggle notes widget", "Back"];
        let selection = Select::with_theme(theme)
            .with_prompt("Notes")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read notes selection: {err}"))?;
        match selection {
            Some(0) => {
                let max_stored: usize = Input::with_theme(theme)
                    .with_prompt("Max notes stored")
                    .default(config.notes.max_stored.max(1))
                    .interact_text()
                    .map_err(|err| format!("Failed to read max notes stored: {err}"))?;
                config.notes.max_stored = max_stored.max(1);
                config.save()?;
            }
            Some(1) => {
                config.notes.show_in_widget = !config.notes.show_in_widget;
                config.save()?;
                println!("Notes widget enabled: {}", config.notes.show_in_widget);
            }
            Some(2) | None => break,
            Some(_) => {}
        }
    }
    Ok(())
}

fn show_timer_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = [
            "Set default timer duration",
            "Toggle timer auto-start",
            "Toggle timer widget",
            "Toggle hide completed timer",
            "Set timer widget mode",
            "Back",
        ];
        let selection = Select::with_theme(theme)
            .with_prompt("Timer")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read timer selection: {err}"))?;
        match selection {
            Some(0) => {
                let duration: String = Input::with_theme(theme)
                    .with_prompt("Default timer duration")
                    .default(config.timer.default_duration.clone())
                    .interact_text()
                    .map_err(|err| format!("Failed to read default timer duration: {err}"))?;
                if !duration.trim().is_empty() {
                    config.timer.default_duration = duration.trim().to_string();
                    config.save()?;
                }
            }
            Some(1) => {
                config.timer.auto_start = !config.timer.auto_start;
                config.save()?;
                println!("Timer auto-start: {}", config.timer.auto_start);
            }
            Some(2) => {
                config.timer.show_in_widget = !config.timer.show_in_widget;
                config.save()?;
                println!("Timer widget enabled: {}", config.timer.show_in_widget);
            }
            Some(3) => {
                config.timer.hide_when_complete = !config.timer.hide_when_complete;
                config.save()?;
                println!("Hide completed timer: {}", config.timer.hide_when_complete);
            }
            Some(4) => {
                let items = ["full", "compact", "Keep current"];
                let default = match config.timer.mode {
                    TimerWidgetMode::Full => 0,
                    TimerWidgetMode::Compact => 1,
                };
                let selection = Select::with_theme(theme)
                    .with_prompt("Timer widget mode")
                    .items(&items)
                    .default(default)
                    .interact_opt()
                    .map_err(|err| format!("Failed to read timer widget mode: {err}"))?;
                match selection {
                    Some(0) => config.timer.mode = TimerWidgetMode::Full,
                    Some(1) => config.timer.mode = TimerWidgetMode::Compact,
                    _ => {}
                }
                config.save()?;
                println!("Timer widget mode: {}", config.timer.mode.label());
            }
            Some(5) | None => break,
            Some(_) => {}
        }
    }
    Ok(())
}

fn show_reminders_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = [
            "Set default reminder duration",
            "Toggle notifications",
            "Toggle sound alerts",
            "Toggle visual alerts",
            "Back",
        ];
        let selection = Select::with_theme(theme)
            .with_prompt("Reminders")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read reminder selection: {err}"))?;
        match selection {
            Some(0) => {
                let duration: String = Input::with_theme(theme)
                    .with_prompt("Default reminder duration")
                    .default(config.reminders.default_duration.clone())
                    .interact_text()
                    .map_err(|err| format!("Failed to read default reminder duration: {err}"))?;
                if !duration.trim().is_empty() {
                    config.reminders.default_duration = duration.trim().to_string();
                    config.save()?;
                }
            }
            Some(1) => {
                config.reminders.enable_notifications = !config.reminders.enable_notifications;
                config.save()?;
                println!(
                    "Reminder notifications enabled: {}",
                    config.reminders.enable_notifications
                );
            }
            Some(2) => {
                config.reminders.sound_alert = !config.reminders.sound_alert;
                config.save()?;
                println!("Reminder sound alerts: {}", config.reminders.sound_alert);
            }
            Some(3) => {
                config.reminders.visual_alert = !config.reminders.visual_alert;
                config.save()?;
                println!("Reminder visual alerts: {}", config.reminders.visual_alert);
            }
            Some(4) | None => break,
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
            "Set layout mode",
            "Set column count",
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
                println!(
                    "Dashboard refresh interval set to {}s.",
                    config.dashboard.refresh_interval
                );
            }
            Some(2) => {
                let items = ["vertical", "horizontal", "auto", "Keep current"];
                let default = match config.dashboard.layout {
                    DashboardLayout::Vertical => 0,
                    DashboardLayout::Horizontal => 1,
                    DashboardLayout::Auto => 2,
                };
                let selection = Select::with_theme(theme)
                    .with_prompt("Dashboard layout mode")
                    .items(&items)
                    .default(default)
                    .interact_opt()
                    .map_err(|err| format!("Failed to read dashboard layout mode: {err}"))?;
                match selection {
                    Some(0) => config.dashboard.layout = DashboardLayout::Vertical,
                    Some(1) => config.dashboard.layout = DashboardLayout::Horizontal,
                    Some(2) => config.dashboard.layout = DashboardLayout::Auto,
                    _ => {}
                }
                config.save()?;
                println!("Dashboard layout mode: {}", config.dashboard.layout.label());
            }
            Some(3) => {
                let prompt = match config.dashboard.columns {
                    Some(columns) => format!("Column count (current: {columns}, blank for auto)"),
                    None => "Column count (blank for auto)".to_string(),
                };
                let value = Input::<String>::with_theme(theme)
                    .with_prompt(prompt)
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|err| format!("Failed to read dashboard columns: {err}"))?;
                let trimmed = value.trim();
                config.dashboard.columns = if trimmed.is_empty() {
                    None
                } else {
                    Some(
                        trimmed
                            .parse::<usize>()
                            .map_err(|err| {
                                format!("Failed to parse dashboard columns '{trimmed}': {err}")
                            })?
                            .max(1),
                    )
                };
                config.save()?;
                println!(
                    "Dashboard columns: {}",
                    config
                        .dashboard
                        .columns
                        .map(|columns| columns.to_string())
                        .unwrap_or_else(|| "auto".to_string())
                );
            }
            Some(4) => {
                config.dashboard.compact_mode = !config.dashboard.compact_mode;
                config.save()?;
                println!("Dashboard compact mode: {}", config.dashboard.compact_mode);
            }
            Some(5) => {
                config.dashboard.freeze = !config.dashboard.freeze;
                config.save()?;
                println!("Dashboard freeze mode: {}", config.dashboard.freeze);
            }
            Some(6) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn show_widgets_menu(config: &mut Config) -> Result<(), String> {
    let theme = ColorfulTheme::default();
    loop {
        let items = [
            "Show current widget order",
            "Configure enabled widgets",
            "Reset enabled widgets",
            "Back",
        ];
        let selection = Select::with_theme(&theme)
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
                configure_widgets(config)?;
            }
            Some(2) => {
                config.dashboard.widgets = default_enabled_widget_names();
                config.save()?;
                println!("Enabled widgets reset to defaults.");
            }
            Some(3) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn configure_widgets(config: &mut Config) -> Result<(), String> {
    let definitions = available_widget_definitions();
    if definitions.is_empty() {
        println!("No dashboard widgets are currently available.");
        return Ok(());
    }

    let _raw_mode = RawModeGuard::enter()?;
    let mut stdout = io::stdout();
    let mut selected = 0usize;

    loop {
        render_widget_config_screen(&mut stdout, config, &definitions, selected)?;
        let next = event::read().map_err(|err| format!("Failed to read widget input: {err}"))?;
        let Event::Key(key) = next else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Up => {
                selected = selected.saturating_sub(1);
            }
            KeyCode::Down => {
                selected = (selected + 1).min(definitions.len().saturating_sub(1));
            }
            KeyCode::Enter | KeyCode::Right => {
                toggle_widget(config, &definitions, selected);
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                config.save()?;
                write!(stdout, "\x1B[2J\x1B[H")
                    .map_err(|err| format!("Failed to clear widgets screen: {err}"))?;
                stdout
                    .flush()
                    .map_err(|err| format!("Failed to flush widget screen: {err}"))?;
                println!("Enabled widgets: {}", config.dashboard.widgets.join(", "));
                return Ok(());
            }
            _ => {}
        }
    }
}

fn render_widget_config_screen(
    stdout: &mut io::Stdout,
    config: &Config,
    definitions: &[WidgetDefinition],
    selected: usize,
) -> Result<(), String> {
    write!(stdout, "\x1B[2J\x1B[H")
        .map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
    writeln!(stdout, "Widgets").map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
    writeln!(stdout).map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
    writeln!(stdout, "Advanced and unified widget configuration")
        .map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
    writeln!(stdout).map_err(|err| format!("Failed to draw widgets screen: {err}"))?;

    for (index, widget) in definitions.iter().enumerate() {
        let enabled = config
            .dashboard
            .widgets
            .iter()
            .any(|item| item == &widget.name);
        let cursor = if index == selected { ">" } else { " " };
        let check = if enabled { "✓" } else { " " };
        writeln!(stdout, "{} [{}] {}", cursor, check, widget.display_name)
            .map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
        if let Some(description) = &widget.description {
            writeln!(stdout, "      {}", description)
                .map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
        }
    }

    writeln!(stdout).map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
    writeln!(stdout, "↑ ↓ navigate   Enter toggle   q save and exit")
        .map_err(|err| format!("Failed to draw widgets screen: {err}"))?;
    stdout
        .flush()
        .map_err(|err| format!("Failed to flush widgets screen: {err}"))?;
    Ok(())
}

fn toggle_widget(config: &mut Config, definitions: &[WidgetDefinition], selected: usize) {
    let Some(widget) = definitions.get(selected) else {
        return;
    };
    if let Some(position) = config
        .dashboard
        .widgets
        .iter()
        .position(|value| value == &widget.name)
    {
        config.dashboard.widgets.remove(position);
        return;
    }

    let insert_at = definitions
        .iter()
        .skip(selected + 1)
        .find_map(|next_widget| {
            config
                .dashboard
                .widgets
                .iter()
                .position(|value| value == &next_widget.name)
        })
        .unwrap_or(config.dashboard.widgets.len());
    config
        .dashboard
        .widgets
        .insert(insert_at, widget.name.clone());
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self, String> {
        enable_raw_mode().map_err(|err| format!("Failed to enable raw mode: {err}"))?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn configure_dashboard_preferences(
    config: &mut Config,
    theme: &ColorfulTheme,
) -> Result<(), String> {
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
            config.dashboard.layout = DashboardLayout::Vertical;
            config.dashboard.columns = None;
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
            config.dashboard.layout = DashboardLayout::Auto;
            config.dashboard.columns = Some(2);
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
    let items = [
        "Enable Unicode borders",
        "Use ASCII-only borders",
        "Keep current",
    ];
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
            "Set OpenRouter API key (Recommended · multi-model support)",
            "Set OpenAI API key",
            "Set Claude API key",
            "Clear an API key",
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
                save_ai_provider_key(theme, config, ProviderKind::OpenRouter)?;
            }
            Some(2) => {
                save_ai_provider_key(theme, config, ProviderKind::OpenAi)?;
            }
            Some(3) => {
                save_ai_provider_key(theme, config, ProviderKind::Anthropic)?;
            }
            Some(4) => clear_api_key_menu(config, theme)?,
            Some(5) => show_current_api_config(config),
            Some(6) | None => break,
            Some(_) => {}
        }
    }

    Ok(())
}

fn save_ai_provider_key(
    theme: &ColorfulTheme,
    config: &mut Config,
    provider: ProviderKind,
) -> Result<(), String> {
    let key = Password::with_theme(theme)
        .with_prompt(format!("{} API key", provider.display_name()))
        .allow_empty_password(true)
        .interact()
        .map_err(|err| format!("Failed to read API key: {err}"))?;

    if key.trim().is_empty() {
        println!("API key was not changed.");
        return Ok(());
    }

    SystemSecretStore.save_provider_key(provider, &key)?;
    config.ai.default_provider = Some(provider.config_key().to_string());
    match provider {
        ProviderKind::OpenAi => config.ai.providers.openai.api_key = None,
        ProviderKind::Anthropic => config.ai.providers.anthropic.api_key = None,
        ProviderKind::OpenRouter => config.ai.providers.openrouter.api_key = None,
    }
    config.save()?;
    println!("{} API key saved.", provider.display_name());
    Ok(())
}

fn clear_api_key_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    let items = [
        "OpenWeather",
        "OpenRouter (Recommended · multi-model support)",
        "OpenAI",
        "Claude",
        "Back",
    ];
    let selection = Select::with_theme(theme)
        .with_prompt("Clear which API key?")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| format!("Failed to read API key selection: {err}"))?;

    match selection {
        Some(0) => {
            config.api_key = None;
            config.provider = None;
            config.save()?;
            println!("OpenWeather API key cleared.");
        }
        Some(1) => clear_ai_provider_key(config, ProviderKind::OpenRouter)?,
        Some(2) => clear_ai_provider_key(config, ProviderKind::OpenAi)?,
        Some(3) => clear_ai_provider_key(config, ProviderKind::Anthropic)?,
        Some(4) | None => {}
        Some(_) => {}
    }

    Ok(())
}

fn clear_ai_provider_key(config: &mut Config, provider: ProviderKind) -> Result<(), String> {
    remove_provider_key(provider)?;
    if config.ai.default_provider.as_deref() == Some(provider.config_key()) {
        config.ai.default_provider = None;
    }
    config.save()?;
    println!("{} API key cleared.", provider.display_name());
    Ok(())
}

fn show_current_api_config(config: &Config) {
    println!("Weather provider: {}", config.provider_label());
    println!(
        "OpenWeather API key: {}",
        config
            .masked_api_key()
            .unwrap_or_else(|| "Not set".to_string())
    );
    show_ai_key_status(
        "OpenRouter (Recommended · multi-model support)",
        ProviderKind::OpenRouter,
    );
    show_ai_key_status("OpenAI", ProviderKind::OpenAi);
    show_ai_key_status("Claude", ProviderKind::Anthropic);
}

fn show_ai_key_status(label: &str, provider: ProviderKind) {
    let value = match SystemSecretStore.load_provider_key(provider) {
        Ok(Some(key)) => mask_value(&key),
        Ok(None) => "Not set".to_string(),
        Err(err) => format!("Unavailable ({err})"),
    };
    println!("{label} API key: {value}");
}

fn mask_value(value: &str) -> String {
    if value.len() <= 4 {
        "*".repeat(value.len())
    } else {
        format!("{}{}", "*".repeat(value.len() - 4), &value[value.len() - 4..])
    }
}

fn show_ai_features_menu(config: &mut Config, theme: &ColorfulTheme) -> Result<(), String> {
    loop {
        let items = [
            "Toggle chat history",
            "Toggle chat context",
            "Toggle simple AI mode (disable auto context)",
            "Toggle persisted transcripts",
            "Edit default AI system prompt",
            "Set default AI screen",
            "Toggle remember last AI screen",
            "Toggle AI tips",
            "Set agent approval mode",
            "Toggle AI audit log",
            "Toggle AI web companion",
            "Set AI refresh interval",
            "Toggle compact agent activity",
            "Back",
        ];
        let selection = Select::with_theme(theme)
            .with_prompt("AI Features")
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read AI features selection: {err}"))?;

        match selection {
            Some(0) => {
                config.ai.runtime.chat_history = !config.ai.runtime.chat_history;
                config.save()?;
                println!("AI chat history: {}", config.ai.runtime.chat_history);
            }
            Some(1) => {
                config.ai.runtime.chat_context = !config.ai.runtime.chat_context;
                config.save()?;
                println!("AI chat context: {}", config.ai.runtime.chat_context);
            }
            Some(2) => {
                config.ai.runtime.auto_context = !config.ai.runtime.auto_context;
                config.save()?;
                println!(
                    "Automatic context gathering: {}",
                    config.ai.runtime.auto_context
                );
            }
            Some(3) => {
                config.ai.runtime.persist_chat_transcripts = !config.ai.runtime.persist_chat_transcripts;
                config.save()?;
                println!(
                    "Persist chat transcripts: {}",
                    config.ai.runtime.persist_chat_transcripts
                );
            }
            Some(4) => {
                let prompt = Input::<String>::with_theme(theme)
                    .with_prompt("Default AI system prompt (leave blank to clear)")
                    .allow_empty(true)
                    .with_initial_text(config.ai.system_prompt.clone().unwrap_or_default())
                    .interact_text()
                    .map_err(|err| format!("Failed to read system prompt: {err}"))?;
                config.ai.system_prompt = if prompt.trim().is_empty() {
                    None
                } else {
                    Some(prompt.trim().to_string())
                };
                config.save()?;
                println!("Default AI system prompt updated.");
            }
            Some(5) => {
                let items = ["agent", "chat", "dashboard", "Keep current"];
                let default = match config.ai.ui.default_view.as_str() {
                    "chat" => 1,
                    "dashboard" => 2,
                    _ => 0,
                };
                let selection = Select::with_theme(theme)
                    .with_prompt("Default AI screen")
                    .items(&items)
                    .default(default)
                    .interact_opt()
                    .map_err(|err| format!("Failed to read default AI screen: {err}"))?;
                match selection {
                    Some(0) => config.ai.ui.default_view = "agent".to_string(),
                    Some(1) => config.ai.ui.default_view = "chat".to_string(),
                    Some(2) => config.ai.ui.default_view = "dashboard".to_string(),
                    _ => {}
                }
                config.save()?;
                println!("Default AI screen: {}", config.ai.ui.default_view);
            }
            Some(6) => {
                config.ai.ui.remember_last_view = !config.ai.ui.remember_last_view;
                config.save()?;
                println!("Remember last AI screen: {}", config.ai.ui.remember_last_view);
            }
            Some(7) => {
                config.ai.ui.show_tips = !config.ai.ui.show_tips;
                config.save()?;
                println!("AI tips enabled: {}", config.ai.ui.show_tips);
            }
            Some(8) => {
                let items = ["manual", "auto", "Keep current"];
                let default = match config.ai.agent.approval_mode {
                    AiApprovalMode::Manual => 0,
                    AiApprovalMode::Auto => 1,
                };
                let selection = Select::with_theme(theme)
                    .with_prompt("Agent approval mode")
                    .items(&items)
                    .default(default)
                    .interact_opt()
                    .map_err(|err| format!("Failed to read approval mode: {err}"))?;
                match selection {
                    Some(0) => config.ai.agent.approval_mode = AiApprovalMode::Manual,
                    Some(1) => config.ai.agent.approval_mode = AiApprovalMode::Auto,
                    _ => {}
                }
                config.save()?;
                println!(
                    "Agent approval mode: {}",
                    match config.ai.agent.approval_mode {
                        AiApprovalMode::Manual => "manual",
                        AiApprovalMode::Auto => "auto",
                    }
                );
            }
            Some(9) => {
                config.ai.agent.audit_log = !config.ai.agent.audit_log;
                config.save()?;
                println!("AI audit log: {}", config.ai.agent.audit_log);
            }
            Some(10) => {
                config.ai.ui.web_enabled = !config.ai.ui.web_enabled;
                config.save()?;
                println!("AI web companion: {}", config.ai.ui.web_enabled);
            }
            Some(11) => {
                let refresh: u64 = Input::with_theme(theme)
                    .with_prompt("AI refresh interval in milliseconds")
                    .default(config.ai.ui.refresh_ms.max(100))
                    .interact_text()
                    .map_err(|err| format!("Failed to read AI refresh interval: {err}"))?;
                config.ai.ui.refresh_ms = refresh.max(100);
                config.save()?;
                println!("AI refresh interval: {} ms", config.ai.ui.refresh_ms);
            }
            Some(12) => {
                config.ai.agent.compact_activity = !config.ai.agent.compact_activity;
                config.save()?;
                println!("Compact agent activity: {}", config.ai.agent.compact_activity);
            }
            Some(13) | None => break,
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
        let items = [
            "Edit config file",
            "Open config file",
            "Show config path",
            "Back",
        ];
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
