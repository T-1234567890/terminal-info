use crate::builtins::build_dashboard_snapshot;
use crate::config::Config;
use crate::output::{OutputMode, output_mode};
use crate::plugin::dashboard_widgets;

pub fn dashboard_output(config: &Config) -> String {
    let title = "Terminal Info";
    let snapshot = build_dashboard_snapshot(config);
    let plugin_widgets = dashboard_widgets();
    let location = if config.uses_auto_location() {
        "auto"
    } else {
        config.configured_location().unwrap_or("unknown")
    };
    let effective_dashboard = config.effective_dashboard();
    let widgets = normalized_widgets(&effective_dashboard.widgets);

    let compact = effective_dashboard.compact_mode || matches!(output_mode(), OutputMode::Compact);

    match if compact {
        OutputMode::Compact
    } else {
        output_mode()
    } {
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
            format!("{}\n", fields.join(" "))
        }
        OutputMode::Plain | OutputMode::Color => {
            let rows = dashboard_rows(location, &snapshot, &widgets, &plugin_widgets);
            format_table(title, &rows)
        }
    }
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

fn dashboard_rows(
    location: &str,
    snapshot: &crate::builtins::DashboardSnapshot,
    widgets: &[String],
    plugin_widgets: &[crate::plugin::PluginWidget],
) -> Vec<(String, String)> {
    let mut rows = vec![("Location".to_string(), location.to_string())];
    for widget in widgets {
        match widget.as_str() {
            "weather" => {
                rows.push(("Weather".to_string(), snapshot.weather.line.clone()));
                if let Some(city) = &snapshot.weather.detected_location {
                    rows.push(("Detected".to_string(), city.clone()));
                }
                if let Some(hint) = &snapshot.weather.hint {
                    for line in hint.lines() {
                        rows.push(("Tip".to_string(), line.to_string()));
                    }
                }
            }
            "time" => rows.push(("Time".to_string(), snapshot.time.clone())),
            "network" => rows.push(("Network".to_string(), snapshot.network.clone())),
            "system" => {
                rows.push(("CPU".to_string(), snapshot.cpu.clone()));
                rows.push(("Memory".to_string(), snapshot.memory.clone()));
            }
            "plugins" if !plugin_widgets.is_empty() => {
                for widget in plugin_widgets {
                    rows.push((widget.title.clone(), widget.content.clone()));
                }
            }
            _ => {}
        }
    }
    rows
}

fn format_table(title: &str, rows: &[(String, String)]) -> String {
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
