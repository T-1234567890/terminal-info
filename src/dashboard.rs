use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::terminal;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, System};

use crate::builtins::{DashboardSnapshot, build_dashboard_snapshot, memory_line};
use crate::config::{Config, DashboardLayout};
use crate::output::{OutputMode, output_mode};
use crate::plugin::{PluginWidget, PluginWidgetBody, dashboard_widgets};
use crate::productivity::ProductivityWidgetManager;
use crate::theme::format_box_table_with_width;

#[derive(Clone, Debug)]
pub struct WidgetDefinition {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled_by_default: bool,
}

pub struct DashboardRenderer {
    config: Config,
    frozen_output: Option<String>,
    plugin_cache: Option<CachedWidgets>,
    built_in_cache: BTreeMap<String, CachedNote>,
    system_sampler: DashboardSystemSampler,
    productivity_widgets: ProductivityWidgetManager,
    reminder_notices: Vec<ReminderNotice>,
}

struct CachedWidgets {
    widgets: Vec<PluginWidget>,
    refresh_at: Instant,
}

struct CachedNote {
    widget: Option<PluginWidget>,
    refresh_at: Instant,
}

struct ReminderNotice {
    message: String,
    expires_at: Instant,
}

struct DashboardSystemSampler {
    system: System,
}

struct DashboardSection {
    title: String,
    rows: Vec<(String, String)>,
}

struct RenderedSection {
    lines: Vec<String>,
    width: usize,
}

#[allow(dead_code)]
pub fn dashboard_output(config: &Config) -> String {
    DashboardRenderer::new(config.clone()).render()
}

impl DashboardRenderer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            frozen_output: None,
            plugin_cache: None,
            built_in_cache: BTreeMap::new(),
            system_sampler: DashboardSystemSampler::new(),
            productivity_widgets: ProductivityWidgetManager::new(),
            reminder_notices: Vec::new(),
        }
    }

    pub fn render(&mut self) -> String {
        if self.config.effective_dashboard().freeze {
            if let Some(output) = &self.frozen_output {
                return output.clone();
            }
        }

        let output = self.render_now();
        if self.config.effective_dashboard().freeze {
            self.frozen_output = Some(output.clone());
        }
        output
    }

    fn render_now(&mut self) -> String {
        self.refresh_reminder_notices();
        let title = "Terminal Info";
        let snapshot = self.system_sampler.snapshot(&self.config);
        let effective_dashboard = self.config.effective_dashboard();
        let compact =
            effective_dashboard.compact_mode || matches!(output_mode(), OutputMode::Compact);
        let plugin_widgets = self.plugin_widgets(compact);
        let widgets = normalized_widgets(&effective_dashboard.widgets, &plugin_widgets);
        let location = if self.config.uses_auto_location() {
            "auto".to_string()
        } else {
            self.config
                .configured_location()
                .unwrap_or("unknown")
                .to_string()
        };

        match if compact {
            OutputMode::Compact
        } else {
            output_mode()
        } {
            OutputMode::Compact => {
                let mut fields = vec![format!("location={location}")];
                for notice in &self.reminder_notices {
                    fields.push(format!("alert={}", notice.message));
                }
                for widget in &widgets {
                    match widget.as_str() {
                        "weather" => fields.push(format!("weather={}", snapshot.weather.line)),
                        "time" => fields.push(format!("time={}", snapshot.time)),
                        "network" => fields.push(format!("net={}", snapshot.network)),
                        "system" => {
                            fields.push(format!("cpu={}", snapshot.cpu));
                            fields.push(format!("mem={}", snapshot.memory));
                        }
                        "notes" | "timer" | "tasks" | "history" | "reminders" => {
                            if let Some(widget) = self.built_in_widget(widget, true) {
                                fields.push(format!(
                                    "{}={}",
                                    widget.title.to_ascii_lowercase(),
                                    compact_widget_summary(&widget, true)
                                ));
                            }
                        }
                        "plugins" if !plugin_widgets.is_empty() => fields.push(format!(
                            "plugins={}",
                            plugin_widgets
                                .iter()
                                .map(|widget| format!(
                                    "{}:{}",
                                    widget.title,
                                    compact_widget_summary(widget, true)
                                ))
                                .collect::<Vec<_>>()
                                .join("|")
                        )),
                        _ => {}
                    }
                }
                format!("{}\n", fields.join(" "))
            }
            OutputMode::Plain | OutputMode::Color => {
                let reminder_messages = self
                    .reminder_notices
                    .iter()
                    .map(|notice| notice.message.clone())
                    .collect::<Vec<_>>();
                let sections = dashboard_sections(
                    &location,
                    &snapshot,
                    &widgets,
                    &plugin_widgets,
                    &reminder_messages,
                    self,
                    false,
                );
                render_dashboard_sections(title, sections, &effective_dashboard)
            }
        }
    }

    fn plugin_widgets(&mut self, compact: bool) -> Vec<PluginWidget> {
        if let Some(cache) = &self.plugin_cache {
            if Instant::now() < cache.refresh_at {
                return cache.widgets.clone();
            }
        }

        let widgets = dashboard_widgets(compact);
        let refresh_after = widgets
            .iter()
            .map(PluginWidget::refresh_interval_secs)
            .min()
            .unwrap_or(5);
        self.plugin_cache = Some(CachedWidgets {
            widgets: widgets.clone(),
            refresh_at: Instant::now() + Duration::from_secs(refresh_after),
        });
        widgets
    }

    fn built_in_widget(&mut self, id: &str, compact: bool) -> Option<PluginWidget> {
        if let Some(cache) = self.built_in_cache.get(id) {
            if Instant::now() < cache.refresh_at {
                return cache.widget.clone();
            }
        }

        let (widget, refresh_after) = match self.productivity_widgets.render(id, compact) {
            Ok(Some((widget, refresh))) => (Some(widget), refresh),
            Ok(None) => (None, Duration::from_secs(5)),
            Err(_) => (None, Duration::from_secs(5)),
        };
        self.built_in_cache.insert(
            id.to_string(),
            CachedNote {
                widget: widget.clone(),
                refresh_at: Instant::now() + refresh_after,
            },
        );
        widget
    }

    fn refresh_reminder_notices(&mut self) {
        self.reminder_notices
            .retain(|notice| Instant::now() < notice.expires_at);
        let triggered = match crate::productivity::trigger_due_reminders() {
            Ok(messages) => messages,
            Err(_) => Vec::new(),
        };
        if self.config.reminders.visual_alert {
            for message in triggered {
                self.reminder_notices.push(ReminderNotice {
                    message,
                    expires_at: Instant::now() + Duration::from_secs(10),
                });
            }
        }
    }
}

impl DashboardSystemSampler {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_usage();
        Self { system }
    }

    fn snapshot(&mut self, config: &Config) -> DashboardSnapshot {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        let mut snapshot = build_dashboard_snapshot(config);
        snapshot.cpu = format!("{:.1}%", self.system.global_cpu_usage());
        snapshot.memory = memory_line(&self.system);
        snapshot
    }
}

pub fn available_widget_definitions() -> Vec<WidgetDefinition> {
    let mut definitions = core_widget_definitions();
    for widget in dashboard_widgets(false) {
        let name = widget.key();
        if definitions.iter().any(|entry| entry.name == name) {
            continue;
        }
        definitions.push(WidgetDefinition {
            name,
            display_name: widget.label(),
            description: widget.description.clone(),
            enabled_by_default: widget.enabled_by_default,
        });
    }
    definitions
}

pub fn default_enabled_widget_names() -> Vec<String> {
    available_widget_definitions()
        .into_iter()
        .filter(|widget| widget.enabled_by_default)
        .map(|widget| widget.name)
        .collect()
}

fn core_widget_definitions() -> Vec<WidgetDefinition> {
    vec![
        WidgetDefinition {
            name: "weather".to_string(),
            display_name: "Weather".to_string(),
            description: Some("Shows current weather conditions".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "time".to_string(),
            display_name: "Time".to_string(),
            description: Some("Shows the current time".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "network".to_string(),
            display_name: "Network".to_string(),
            description: Some("Shows the current network status".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "system".to_string(),
            display_name: "System".to_string(),
            description: Some("Shows CPU and memory usage".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "timer".to_string(),
            display_name: "Timers".to_string(),
            description: Some("Shows active countdowns and stopwatches".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "tasks".to_string(),
            display_name: "Tasks".to_string(),
            description: Some("Shows pending task items".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "notes".to_string(),
            display_name: "Notes".to_string(),
            description: Some("Shows recent notes".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "history".to_string(),
            display_name: "History".to_string(),
            description: Some("Shows recent shell history".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "reminders".to_string(),
            display_name: "Reminders".to_string(),
            description: Some("Shows upcoming reminders".to_string()),
            enabled_by_default: true,
        },
        WidgetDefinition {
            name: "plugins".to_string(),
            display_name: "Plugin widgets".to_string(),
            description: Some("Shows dashboard widgets provided by trusted plugins".to_string()),
            enabled_by_default: true,
        },
    ]
}

fn normalized_widgets(widgets: &[String], plugin_widgets: &[PluginWidget]) -> Vec<String> {
    let mut allowed = core_widget_definitions()
        .into_iter()
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    for widget in plugin_widgets {
        let name = widget.key();
        if !allowed.iter().any(|entry| entry == &name) {
            allowed.push(name);
        }
    }
    let mut normalized = widgets
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| allowed.iter().any(|entry| entry == value))
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        normalized = default_enabled_widget_names();
    }
    normalized
}

fn dashboard_sections(
    location: &str,
    snapshot: &crate::builtins::DashboardSnapshot,
    widgets: &[String],
    plugin_widgets: &[PluginWidget],
    reminder_messages: &[String],
    renderer: &mut DashboardRenderer,
    compact: bool,
) -> Vec<DashboardSection> {
    let mut sections = Vec::new();
    let mut main_rows = vec![("Location".to_string(), location.to_string())];
    for message in reminder_messages {
        main_rows.push(("Alert".to_string(), format!("Reminder: {message}")));
    }
    let plugin_widget_map = plugin_widgets
        .iter()
        .map(|widget| (widget.key(), widget))
        .collect::<BTreeMap<_, _>>();

    for widget in widgets {
        match widget.as_str() {
            "weather" => {
                main_rows.push(("Weather".to_string(), snapshot.weather.line.clone()));
                if let Some(city) = &snapshot.weather.detected_location {
                    main_rows.push(("Detected".to_string(), city.clone()));
                }
                if let Some(hint) = &snapshot.weather.hint {
                    for line in hint.lines() {
                        main_rows.push(("Tip".to_string(), normalize_tip_line(line)));
                    }
                }
            }
            "time" => main_rows.push(("Time".to_string(), snapshot.time.clone())),
            "network" => main_rows.push(("Network".to_string(), snapshot.network.clone())),
            "system" => {
                main_rows.push(("CPU".to_string(), snapshot.cpu.clone()));
                main_rows.push(("Memory".to_string(), snapshot.memory.clone()));
            }
            "timer" | "tasks" | "notes" | "history" | "reminders" => {
                if let Some(widget) = renderer.built_in_widget(widget, compact) {
                    sections.push(DashboardSection {
                        title: widget.title.clone(),
                        rows: render_widget_rows(&widget, compact),
                    });
                }
            }
            "plugins" if !plugin_widgets.is_empty() => {
                for widget in plugin_widgets {
                    sections.push(DashboardSection {
                        title: widget.label(),
                        rows: render_widget_rows(widget, compact),
                    });
                }
            }
            _ => {
                if let Some(widget) = plugin_widget_map.get(widget) {
                    sections.push(DashboardSection {
                        title: widget.label(),
                        rows: render_widget_rows(widget, compact),
                    });
                }
            }
        }
    }
    sections.insert(
        0,
        DashboardSection {
            title: "Terminal Info".to_string(),
            rows: main_rows,
        },
    );
    sections
}

fn render_widget_rows(widget: &PluginWidget, compact: bool) -> Vec<(String, String)> {
    match widget.body(compact) {
        PluginWidgetBody::Text { content } => content
            .lines()
            .enumerate()
            .map(|(index, line)| parse_text_widget_line(line, index))
            .collect(),
        PluginWidgetBody::List { items } => {
            let mut rows = Vec::new();
            for (index, item) in items.iter().enumerate() {
                rows.push((
                    if index == 0 { "Item" } else { "-" }.to_string(),
                    item.clone(),
                ));
            }
            rows
        }
        PluginWidgetBody::Table { headers, rows } => {
            let mut rendered = Vec::new();
            for (index, row) in rows.iter().enumerate() {
                if row.len() >= 2 {
                    rendered.push((row[0].clone(), row[1..].join(" | ")));
                } else if let Some(value) = row.first() {
                    rendered.push((
                        if index == 0 {
                            headers
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "Value".to_string())
                        } else {
                            "-".to_string()
                        },
                        value.clone(),
                    ));
                }
            }
            rendered
        }
    }
}

fn render_dashboard_sections(
    title: &str,
    sections: Vec<DashboardSection>,
    dashboard: &crate::config::DashboardConfig,
) -> String {
    let terminal_width = terminal::size().ok().map(|(width, _)| width as usize);
    let columns = resolved_columns(
        dashboard.layout,
        dashboard.columns,
        terminal_width,
        sections.len(),
    );
    if columns <= 1 {
        let width = terminal_width.map(|width| width.saturating_sub(4));
        return sections
            .into_iter()
            .map(|section| {
                let section_title = if section.title == "Terminal Info" {
                    title.to_string()
                } else {
                    section.title
                };
                format_box_table_with_width(&section_title, &section.rows, width)
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    let gap = 3;
    let term_width = terminal_width.unwrap_or(120);
    let cell_width = ((term_width.saturating_sub(gap * (columns - 1))) / columns).max(24);
    let content_width = cell_width.saturating_sub(4);
    let rendered = sections
        .into_iter()
        .map(|section| {
            let section_title = if section.title == "Terminal Info" {
                title.to_string()
            } else {
                section.title
            };
            let body =
                format_box_table_with_width(&section_title, &section.rows, Some(content_width));
            RenderedSection {
                lines: body.lines().map(|line| line.to_string()).collect(),
                width: cell_width,
            }
        })
        .collect::<Vec<_>>();

    render_section_grid(&rendered, columns, gap)
}

fn resolved_columns(
    layout: DashboardLayout,
    configured_columns: Option<usize>,
    terminal_width: Option<usize>,
    section_count: usize,
) -> usize {
    if section_count <= 1 {
        return 1;
    }

    let mut columns = match layout {
        DashboardLayout::Vertical => 1,
        DashboardLayout::Horizontal => configured_columns.unwrap_or(2),
        DashboardLayout::Auto => {
            configured_columns.unwrap_or_else(|| match terminal_width.unwrap_or(0) {
                0..=100 => 1,
                101..=150 => 2,
                _ => 3,
            })
        }
    }
    .max(1)
    .min(section_count);

    if columns > 1 {
        let gap = 3;
        let terminal_width = terminal_width.unwrap_or(120);
        while columns > 1 && (terminal_width.saturating_sub(gap * (columns - 1))) / columns < 28 {
            columns -= 1;
        }
    }

    columns.max(1)
}

fn render_section_grid(sections: &[RenderedSection], columns: usize, gap: usize) -> String {
    let mut lines = Vec::new();
    for row in sections.chunks(columns) {
        let height = row
            .iter()
            .map(|section| section.lines.len())
            .max()
            .unwrap_or(0);
        for line_idx in 0..height {
            let parts = row
                .iter()
                .map(|section| {
                    section.lines.get(line_idx).map_or_else(
                        || " ".repeat(section.width),
                        |line| pad_grid_line(line, section.width),
                    )
                })
                .collect::<Vec<_>>();
            lines.push(parts.join(&" ".repeat(gap)));
        }
        lines.push(String::new());
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    format!("{}\n", lines.join("\n"))
}

fn pad_grid_line(line: &str, width: usize) -> String {
    let visible = visible_width(line);
    if visible >= width {
        line.to_string()
    } else {
        format!("{line}{}", " ".repeat(width - visible))
    }
}

fn visible_width(line: &str) -> usize {
    let mut width = 0usize;
    let chars = line.chars().collect::<Vec<_>>();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '\x1b' {
            i += 1;
            if i < chars.len() && chars[i] == '[' {
                i += 1;
                while i < chars.len() {
                    let ch = chars[i];
                    i += 1;
                    if ('@'..='~').contains(&ch) {
                        break;
                    }
                }
                continue;
            }
            continue;
        }

        width += 1;
        i += 1;
    }

    width
}

fn normalize_tip_line(line: &str) -> String {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("Tip:")
        .map(str::trim)
        .unwrap_or(trimmed)
        .to_string()
}

fn parse_text_widget_line(line: &str, index: usize) -> (String, String) {
    let trimmed = line.trim();
    if let Some((label, value)) = trimmed.split_once(':') {
        return (label.trim().to_string(), value.trim().to_string());
    }
    (
        if index == 0 {
            "Status".to_string()
        } else {
            "More".to_string()
        },
        trimmed.to_string(),
    )
}

fn compact_widget_summary(widget: &PluginWidget, compact: bool) -> String {
    match widget.body(compact) {
        PluginWidgetBody::Text { content } => content.clone(),
        PluginWidgetBody::List { items } => {
            items.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
        }
        PluginWidgetBody::Table { rows, .. } => rows
            .iter()
            .take(2)
            .map(|row| row.join("/"))
            .collect::<Vec<_>>()
            .join("; "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_enabled_widgets_include_core_defaults() {
        let widgets = default_enabled_widget_names();
        assert!(widgets.iter().any(|widget| widget == "weather"));
        assert!(widgets.iter().any(|widget| widget == "time"));
        assert!(widgets.iter().any(|widget| widget == "plugins"));
    }
}
