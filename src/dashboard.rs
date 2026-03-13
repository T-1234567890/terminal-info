use crate::builtins::build_dashboard_snapshot;
use crate::config::Config;
use crate::output::{OutputMode, output_mode};
use crate::plugin::dashboard_widgets;

pub fn show_dashboard(config: &Config) -> Result<(), String> {
    let title = "Terminal Info";
    let snapshot = build_dashboard_snapshot(config);
    let plugin_widgets = dashboard_widgets();
    let location = if config.uses_auto_location() {
        "auto"
    } else {
        config.configured_location().unwrap_or("unknown")
    };
    let widgets = normalized_widgets(&config.dashboard.widgets);

    match output_mode() {
        OutputMode::Compact => {
            let mut fields = vec![format!("location={location}")];
            for widget in &widgets {
                match widget.as_str() {
                    "weather" => fields.push(format!("weather={}", snapshot.weather.line)),
                    "time" => fields.push(format!("time={}", snapshot.time)),
                    "network" => fields.push(format!("net={}", snapshot.network)),
                    "system" => {
                        fields.push(format!("cpu={}", snapshot.cpu));
                        fields.push(format!("mem={}", snapshot.memory));
                    }
                    "plugins" if !plugin_widgets.is_empty() => fields.push(format!(
                        "plugins={}",
                        plugin_widgets
                            .iter()
                            .map(|widget| format!("{}:{}", widget.title, widget.content))
                            .collect::<Vec<_>>()
                            .join("|")
                    )),
                    _ => {}
                }
            }
            println!("{}", fields.join(" "));
        }
        OutputMode::Plain => {
            println!("{title}");
            render_dashboard_lines(location, &snapshot, &widgets, &plugin_widgets);
        }
        OutputMode::Color => {
            let border = format!("+{}+", "-".repeat(title.len() + 2));
            println!("{border}");
            println!("| {title} |");
            println!("{border}");
            render_dashboard_lines(location, &snapshot, &widgets, &plugin_widgets);
        }
    }

    Ok(())
}

fn normalized_widgets(widgets: &[String]) -> Vec<String> {
    let mut normalized = widgets
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| {
            matches!(
                value.as_str(),
                "weather" | "time" | "network" | "system" | "plugins"
            )
        })
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        normalized = vec![
            "weather".to_string(),
            "time".to_string(),
            "network".to_string(),
            "system".to_string(),
            "plugins".to_string(),
        ];
    }
    normalized
}

fn render_dashboard_lines(
    location: &str,
    snapshot: &crate::builtins::DashboardSnapshot,
    widgets: &[String],
    plugin_widgets: &[crate::plugin::PluginWidget],
) {
    println!("Location: {location}");
    for widget in widgets {
        match widget.as_str() {
            "weather" => {
                println!("Weather: {}", snapshot.weather.line);
                if let Some(city) = &snapshot.weather.detected_location {
                    println!("Detected location: {city}");
                }
                if let Some(hint) = &snapshot.weather.hint {
                    println!("{hint}");
                }
            }
            "time" => println!("Time: {}", snapshot.time),
            "network" => println!("Network: {}", snapshot.network),
            "system" => {
                println!("CPU: {}", snapshot.cpu);
                println!("Memory: {}", snapshot.memory);
            }
            "plugins" if !plugin_widgets.is_empty() => {
                println!("Plugins:");
                for widget in plugin_widgets {
                    println!("{}: {}", widget.title, widget.content);
                }
            }
            _ => {}
        }
    }
}
