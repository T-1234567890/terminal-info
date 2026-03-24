use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, System};

use crate::builtins::{DashboardSnapshot, build_dashboard_snapshot, memory_line};
use crate::config::{Config, dashboard_notes_path};
use crate::output::{OutputMode, output_mode};
use crate::plugin::{PluginWidget, PluginWidgetBody, dashboard_widgets};
use crate::theme::format_box_table;

pub struct DashboardRenderer {
    config: Config,
    frozen_output: Option<String>,
    plugin_cache: Option<CachedWidgets>,
    notes_cache: Option<CachedNote>,
    system_sampler: DashboardSystemSampler,
}

struct CachedWidgets {
    widgets: Vec<PluginWidget>,
    refresh_at: Instant,
}

struct CachedNote {
    widget: Option<PluginWidget>,
    refresh_at: Instant,
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
            notes_cache: None,
            system_sampler: DashboardSystemSampler::new(),
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
        let title = "Terminal Info";
        let snapshot = self.system_sampler.snapshot(&self.config);
        let effective_dashboard = self.config.effective_dashboard();
        let widgets = normalized_widgets(&effective_dashboard.widgets);
        let compact = effective_dashboard.compact_mode || matches!(output_mode(), OutputMode::Compact);
        let plugin_widgets = self.plugin_widgets(compact);
        let notes_widget = self.notes_widget(compact);
        let location = if self.config.uses_auto_location() {
            "auto"
        } else {
            self.config.configured_location().unwrap_or("unknown")
        };

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
                        "notes" => {
                            if let Some(widget) = &notes_widget {
                                fields.push(format!(
                                    "notes={}",
                                    compact_widget_summary(widget, true)
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
                let rows = dashboard_rows(
                    location,
                    &snapshot,
                    &widgets,
                    &plugin_widgets,
                    notes_widget.as_ref(),
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

    fn notes_widget(&mut self, compact: bool) -> Option<PluginWidget> {
        if let Some(cache) = &self.notes_cache {
            if Instant::now() < cache.refresh_at {
                return cache.widget.clone();
            }
        }

        let widget = load_notes_widget(compact);
        self.notes_cache = Some(CachedNote {
            widget: widget.clone(),
            refresh_at: Instant::now() + Duration::from_secs(5),
        });
        widget
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
                "weather" | "time" | "network" | "system" | "notes" | "plugins"
            )
        })
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        normalized = vec![
            "weather".to_string(),
            "time".to_string(),
            "network".to_string(),
            "system".to_string(),
            "notes".to_string(),
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
    notes_widget: Option<&PluginWidget>,
    compact: bool,
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
            "notes" => {
                if let Some(widget) = notes_widget {
                    rows.extend(render_widget_rows(widget, compact));
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

fn load_notes_widget(_compact: bool) -> Option<PluginWidget> {
    let path = dashboard_notes_path().ok()?;
    let contents = fs::read_to_string(path).ok()?;
    let lines = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }

    let full = lines.iter().take(5).cloned().collect::<Vec<_>>();
    let compact = lines.first().cloned().unwrap_or_default();
    Some(PluginWidget {
        title: "Notes".to_string(),
        refresh_interval_secs: Some(5),
        full: PluginWidgetBody::List { items: full },
        compact: Some(PluginWidgetBody::Text { content: compact }),
    })
}
