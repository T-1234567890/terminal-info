use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, System};

use crate::builtins::{DashboardSnapshot, build_dashboard_snapshot, memory_line};
use crate::config::Config;
use crate::output::{OutputMode, output_mode};
use crate::plugin::{PluginWidget, PluginWidgetBody, dashboard_widgets};
use crate::productivity::ProductivityWidgetManager;
use crate::theme::format_box_table;

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
        let widgets = normalized_widgets(&effective_dashboard.widgets);
        let compact = effective_dashboard.compact_mode || matches!(output_mode(), OutputMode::Compact);
        let plugin_widgets = self.plugin_widgets(compact);
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
                                .map(|widget| format!("{}:{}", widget.title, compact_widget_summary(widget, true)))
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
                let rows = dashboard_rows(
                    &location,
                    &snapshot,
                    &widgets,
                    &plugin_widgets,
                    &reminder_messages,
                    self,
                    false,
                );
                format_box_table(title, &rows)
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
        self.built_in_cache.insert(id.to_string(), CachedNote {
            widget: widget.clone(),
            refresh_at: Instant::now() + refresh_after,
        });
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

fn normalized_widgets(widgets: &[String]) -> Vec<String> {
    let mut normalized = widgets
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| {
            matches!(
                value.as_str(),
                "weather" | "time" | "network" | "system" | "timer" | "tasks" | "notes"
                    | "history" | "reminders" | "plugins"
            )
        })
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        normalized = vec![
            "weather".to_string(),
            "time".to_string(),
            "network".to_string(),
            "system".to_string(),
            "timer".to_string(),
            "tasks".to_string(),
            "notes".to_string(),
            "history".to_string(),
            "reminders".to_string(),
            "plugins".to_string(),
        ];
    }
    normalized
}

fn dashboard_rows(
    location: &str,
    snapshot: &crate::builtins::DashboardSnapshot,
    widgets: &[String],
    plugin_widgets: &[PluginWidget],
    reminder_messages: &[String],
    renderer: &mut DashboardRenderer,
    compact: bool,
) -> Vec<(String, String)> {
    let mut rows = vec![("Location".to_string(), location.to_string())];
    for message in reminder_messages {
        rows.push(("Alert".to_string(), format!("Reminder: {message}")));
    }
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
            "timer" | "tasks" | "notes" | "history" | "reminders" => {
                if let Some(widget) = renderer.built_in_widget(widget, compact) {
                    rows.extend(render_widget_rows(&widget, compact));
                }
            }
            "plugins" if !plugin_widgets.is_empty() => {
                for widget in plugin_widgets {
                    rows.extend(render_widget_rows(widget, compact));
                }
            }
            _ => {}
        }
    }
    rows
}

fn render_widget_rows(widget: &PluginWidget, compact: bool) -> Vec<(String, String)> {
    match widget.body(compact) {
        PluginWidgetBody::Text { content } => vec![(widget.title.clone(), content.clone())],
        PluginWidgetBody::List { items } => {
            let mut rows = Vec::new();
            for (index, item) in items.iter().enumerate() {
                rows.push((
                    if index == 0 {
                        widget.title.clone()
                    } else {
                        String::new()
                    },
                    item.clone(),
                ));
            }
            rows
        }
        PluginWidgetBody::Table { headers, rows } => {
            let mut rendered = Vec::new();
            for (index, row) in rows.iter().enumerate() {
                if row.len() >= 2 {
                    rendered.push((
                        if index == 0 {
                            format!("{} {}", widget.title, row[0])
                        } else {
                            row[0].clone()
                        },
                        row[1..].join(" | "),
                    ));
                } else if let Some(value) = row.first() {
                    rendered.push((
                        if index == 0 {
                            widget.title.clone()
                        } else {
                            headers.first().cloned().unwrap_or_default()
                        },
                        value.clone(),
                    ));
                }
            }
            rendered
        }
    }
}

fn compact_widget_summary(widget: &PluginWidget, compact: bool) -> String {
    match widget.body(compact) {
        PluginWidgetBody::Text { content } => content.clone(),
        PluginWidgetBody::List { items } => items.iter().take(3).cloned().collect::<Vec<_>>().join(", "),
        PluginWidgetBody::Table { rows, .. } => rows
            .iter()
            .take(2)
            .map(|row| row.join("/"))
            .collect::<Vec<_>>()
            .join("; "),
    }
}
