use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Local, LocalResult, NaiveTime, TimeZone};
use dialoguer::{Input, Select, theme::ColorfulTheme};
use serde::{Deserialize, Serialize};

use crate::config::{
    Config, RemindersConfig, TaskSortOrder, TasksConfig, TimerConfig, TimerWidgetMode,
    data_dir_path,
    home_dir_path,
};
use crate::output::{OutputMode, json_output, output_mode};
use crate::plugin::{PluginWidget, PluginWidgetBody};
use crate::theme::format_box_table;

const TIMER_FILE: &str = "timer.json";
const TASKS_FILE: &str = "tasks.json";
const NOTES_FILE: &str = "notes.json";
const REMINDERS_FILE: &str = "reminders.json";
const DELETED_TASK_RETENTION_SECS: u64 = 7 * 24 * 60 * 60;
const COMPLETED_TIMER_GRACE_SECS: u64 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TimerState {
    started_at: u64,
    duration_secs: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TimerStore {
    countdown: Option<TimerState>,
    stopwatch: Option<TimerState>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TaskStore {
    next_id: u64,
    #[serde(default)]
    tasks: Vec<TaskItem>,
    #[serde(default)]
    deleted_tasks: Vec<DeletedTaskItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TaskItem {
    id: u64,
    text: String,
    done: bool,
    #[serde(default)]
    created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DeletedTaskItem {
    id: u64,
    text: String,
    done: bool,
    #[serde(default)]
    created_at: u64,
    deleted_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct NoteStore {
    next_id: u64,
    notes: Vec<NoteItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NoteItem {
    id: u64,
    text: String,
    created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ReminderItem {
    id: String,
    message: String,
    trigger_at: u64,
    triggered: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ReminderStore {
    reminders: Vec<ReminderItem>,
}

#[derive(Clone, Debug, Deserialize)]
struct LegacyReminderStore {
    #[allow(dead_code)]
    next_id: u64,
    reminders: Vec<LegacyReminderItem>,
}

#[derive(Clone, Debug, Deserialize)]
struct LegacyReminderItem {
    id: u64,
    due_at: u64,
    message: String,
}

pub trait DashboardDataWidget {
    fn refresh_interval(&self) -> Duration;
    fn render(&self, compact: bool) -> Result<Option<PluginWidget>, String>;
}

pub struct TimerWidget;
pub struct TaskWidget;
pub struct NotesWidget;
pub struct HistoryWidget;
pub struct ReminderWidget;

#[derive(Clone, Copy, Debug)]
pub enum TimerLiveTarget {
    Countdown,
    Stopwatch,
}

pub fn timer_dashboard_output() -> Result<String, String> {
    let store = load_timer_store()?;
    let settings = runtime_config().timer;
    if json_output() {
        let json = serde_json::json!({
            "countdown": visible_countdown(&store, &settings).map(|state| serde_json::json!({
                "status": countdown_status_line(state, &settings),
                "started_at": state.started_at,
                "duration_secs": state.duration_secs.unwrap_or_default(),
            })),
            "stopwatch": store.stopwatch.as_ref().map(|state| serde_json::json!({
                "status": stopwatch_status_line(state, &settings),
                "started_at": state.started_at,
            })),
        });
        return serde_json::to_string_pretty(&json)
            .map(|body| format!("{body}\n"))
            .map_err(|err| format!("Failed to serialize timer dashboard: {err}"));
    }

    let rows = timer_dashboard_rows(store, &settings);
    match output_mode() {
        OutputMode::Compact => {
            let line = rows
                .iter()
                .map(|(label, value)| format!("{}={}", label.to_ascii_lowercase(), value))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(format!("{line}\n"))
        }
        OutputMode::Plain | OutputMode::Color => Ok(format_box_table("Terminal Info Timer", &rows)),
    }
}

pub fn start_timer(duration: Option<&str>, settings: &TimerConfig) -> Result<(), String> {
    let duration = duration
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(settings.default_duration.as_str());
    let duration_secs = parse_duration(duration)?;
    let mut store = load_timer_store()?;
    store.countdown = Some(TimerState {
        started_at: now_unix(),
        duration_secs: Some(duration_secs),
    });
    save_timer_store(&store)?;
    println!("Started countdown timer for {}.", format_duration(duration_secs));
    Ok(())
}

pub fn stop_timer() -> Result<(), String> {
    let mut store = load_timer_store()?;
    store.countdown = None;
    if store.stopwatch.is_none() {
        clear_file(TIMER_FILE)?;
    } else {
        save_timer_store(&store)?;
    }
    println!("Stopped countdown timer.");
    Ok(())
}

pub fn start_stopwatch() -> Result<(), String> {
    let mut store = load_timer_store()?;
    store.stopwatch = Some(TimerState {
        started_at: now_unix(),
        duration_secs: None,
    });
    save_timer_store(&store)?;
    println!("Started stopwatch.");
    Ok(())
}

pub fn stop_stopwatch() -> Result<(), String> {
    let mut store = load_timer_store()?;
    store.stopwatch = None;
    if store.countdown.is_none() {
        clear_file(TIMER_FILE)?;
    } else {
        save_timer_store(&store)?;
    }
    println!("Stopped stopwatch.");
    Ok(())
}

pub fn has_active_timer_state() -> Result<bool, String> {
    let store = load_timer_store()?;
    Ok(store.countdown.is_some() || store.stopwatch.is_some())
}

pub fn timer_live_active(target: TimerLiveTarget) -> Result<bool, String> {
    let store = load_timer_store()?;
    Ok(match target {
        TimerLiveTarget::Countdown => store
            .countdown
            .as_ref()
            .is_some_and(countdown_is_running),
        TimerLiveTarget::Stopwatch => store.stopwatch.is_some(),
    })
}

pub fn add_task(text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Task text cannot be empty.".to_string());
    }

    let mut store = load_tasks()?;
    let id = next_id(&mut store.next_id);
    store.tasks.push(TaskItem {
        id,
        text: text.to_string(),
        done: false,
        created_at: now_unix(),
    });
    save_tasks(&store)?;
    println!("Added task #{id}: {text}");
    Ok(())
}

pub fn list_tasks() -> Result<(), String> {
    let config = runtime_config();
    let tasks = display_tasks(&load_tasks()?.tasks, &config.tasks, None);
    if tasks.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    for task in &tasks {
        println!(
            "{} {} {}",
            task.id,
            if task.done { "[x]" } else { "[ ]" },
            task.text
        );
    }
    Ok(())
}

pub fn complete_task(id: u64) -> Result<(), String> {
    let settings = runtime_config().tasks;
    let mut store = load_tasks()?;
    let task_index = store
        .tasks
        .iter()
        .position(|task| task.id == id)
        .ok_or_else(|| format!("Task #{id} was not found."))?;
    if settings.auto_remove_completed {
        store.tasks.remove(task_index);
        save_tasks(&store)?;
        println!("Completed and removed task #{id}.");
        return Ok(());
    }
    store.tasks[task_index].done = true;
    save_tasks(&store)?;
    println!("Completed task #{id}.");
    Ok(())
}

pub fn delete_task(id: u64) -> Result<(), String> {
    let mut store = load_tasks()?;
    let task_index = store
        .tasks
        .iter()
        .position(|task| task.id == id)
        .ok_or_else(|| format!("Task #{id} was not found."))?;
    let task = store.tasks.remove(task_index);
    store.deleted_tasks.push(DeletedTaskItem {
        id: task.id,
        text: task.text,
        done: task.done,
        created_at: task.created_at,
        deleted_at: now_unix(),
    });
    save_tasks(&store)?;
    println!("Deleted task #{id}. You can recover it within seven days.");
    Ok(())
}

pub fn recover_task(id: u64) -> Result<(), String> {
    let mut store = load_tasks()?;
    let deleted_index = store
        .deleted_tasks
        .iter()
        .position(|task| task.id == id)
        .ok_or_else(|| format!("Deleted task #{id} was not found."))?;
    let task = store.deleted_tasks.remove(deleted_index);
    store.tasks.push(TaskItem {
        id: task.id,
        text: task.text,
        done: task.done,
        created_at: task.created_at,
    });
    save_tasks(&store)?;
    println!("Recovered task #{id}.");
    Ok(())
}

struct DeletedTaskDisplay {
    text: String,
    seconds_left: u64,
}

fn deleted_display_tasks(tasks: &[DeletedTaskItem]) -> Vec<DeletedTaskDisplay> {
    let now = now_unix();
    tasks.iter()
        .map(|task| DeletedTaskDisplay {
            text: task.text.clone(),
            seconds_left: task
                .deleted_at
                .saturating_add(DELETED_TASK_RETENTION_SECS)
                .saturating_sub(now),
        })
        .collect()
}

fn purge_expired_deleted_tasks(store: &mut TaskStore) -> bool {
    let now = now_unix();
    let original_len = store.deleted_tasks.len();
    store.deleted_tasks.retain(|task| {
        now < task
            .deleted_at
            .saturating_add(DELETED_TASK_RETENTION_SECS)
    });
    original_len != store.deleted_tasks.len()
}

fn choose_deleted_task_to_recover(theme: &ColorfulTheme, tasks: &[DeletedTaskItem]) -> Result<(), String> {
    if tasks.is_empty() {
        println!("No deleted tasks.");
        return Ok(());
    }

    let items = deleted_display_tasks(tasks)
        .into_iter()
        .map(|task| format!("{} (recover, {} left)", task.text, format_duration(task.seconds_left)))
        .chain(std::iter::once("Exit".to_string()))
        .collect::<Vec<_>>();

    let selection = Select::with_theme(theme)
        .with_prompt("Deleted tasks")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| format!("Failed to read deleted task selection: {err}"))?;

    match selection {
        Some(index) if index < tasks.len() => recover_task(tasks[index].id),
        _ => Ok(()),
    }
}

fn choose_task_to_toggle(theme: &ColorfulTheme, tasks: &[TaskItem], settings: &TasksConfig) -> Result<(), String> {
    if tasks.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    let items = tasks
        .iter()
        .map(|task| format!("{} {}", if task.done { "[x]" } else { "[ ]" }, task.text))
        .chain(std::iter::once("Exit".to_string()))
        .collect::<Vec<_>>();

    let selection = Select::with_theme(theme)
        .with_prompt("All tasks")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| format!("Failed to read task selection: {err}"))?;

    match selection {
        Some(index) if index < tasks.len() => toggle_task(tasks[index].id, settings),
        _ => Ok(()),
    }
}

pub fn add_note(text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Note text cannot be empty.".to_string());
    }

    let mut store = load_notes()?;
    let id = next_id(&mut store.next_id);
    store.notes.push(NoteItem {
        id,
        text: text.to_string(),
        created_at: now_unix(),
    });
    trim_notes(&mut store.notes, runtime_config().notes.max_stored);
    save_notes(&store)?;
    println!("Added note #{id}.");
    Ok(())
}

pub fn list_notes() -> Result<(), String> {
    let store = load_notes()?;
    if store.notes.is_empty() {
        println!("No notes.");
        return Ok(());
    }

    for note in &store.notes {
        println!("{} {}", note.id, note.text);
    }
    Ok(())
}

pub fn replace_notes_with_single_entry(text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        save_notes(&NoteStore::default())?;
        return Ok(());
    }

    let store = NoteStore {
        next_id: 2,
        notes: vec![NoteItem {
            id: 1,
            text: text.to_string(),
            created_at: now_unix(),
        }],
    };
    save_notes(&store)
}

pub fn clear_notes() -> Result<(), String> {
    save_notes(&NoteStore::default())?;
    Ok(())
}

pub fn interactive_task_menu(config: &Config) -> Result<(), String> {
    let theme = ColorfulTheme::default();

    loop {
        let store = load_tasks()?;
        let tasks = display_tasks(&store.tasks, &config.tasks, None);
        let mut items = tasks
            .iter()
            .map(|task| {
                format!(
                    "{} {}",
                    if task.done { "[x]" } else { "[ ]" },
                    task.text
                )
            })
            .collect::<Vec<_>>();
        items.push("List all tasks".to_string());
        items.push("Deleted tasks".to_string());
        items.push("Add task".to_string());
        items.push("Delete task".to_string());
        items.push("Exit".to_string());

        let selection = Select::with_theme(&theme)
            .with_prompt(task_menu_prompt(&config.tasks))
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(|err| format!("Failed to read task menu selection: {err}"))?;

        match selection {
            Some(index) if index < tasks.len() => {
                toggle_task(tasks[index].id, &config.tasks)?;
            }
            Some(index) if index == tasks.len() => {
                choose_task_to_toggle(&theme, &store.tasks, &config.tasks)?;
            }
            Some(index) if index == tasks.len() + 1 => {
                choose_deleted_task_to_recover(&theme, &store.deleted_tasks)?;
            }
            Some(index) if index == tasks.len() + 2 => {
                if let Some(text) = prompt_task_text()? {
                    add_task(&text)?;
                }
            }
            Some(index) if index == tasks.len() + 3 => {
                choose_task_to_delete(&theme, &tasks)?;
            }
            Some(_) | None => return Ok(()),
        }
    }
}

pub fn show_history(limit: usize) -> Result<(), String> {
    let items = recent_history(limit)?;
    if items.is_empty() {
        println!("No recent history found.");
        return Ok(());
    }

    for item in items {
        println!("{item}");
    }
    Ok(())
}

pub fn add_reminder(time: &str, message: Option<&str>) -> Result<(), String> {
    let (trigger_at, scheduled_for) = parse_reminder_target(time)?;
    let message = message
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Terminal Info reminder")
        .to_string();

    let mut store = load_reminders()?;
    let id = format!("r-{}-{}", trigger_at, store.reminders.len() + 1);
    store.reminders.push(ReminderItem {
        id,
        message,
        trigger_at,
        triggered: false,
    });
    save_reminders(&store)?;
    println!("Reminder scheduled for {scheduled_for}");
    Ok(())
}

pub fn dashboard_note_lines() -> Result<Vec<String>, String> {
    let config = runtime_config();
    let store = load_notes()?;
    Ok(store
        .notes
        .iter()
        .rev()
        .take(config.notes.max_stored.min(5))
        .map(|note| note.text.clone())
        .collect::<Vec<_>>())
}

impl DashboardDataWidget for TimerWidget {
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn render(&self, _compact: bool) -> Result<Option<PluginWidget>, String> {
        if !runtime_config().timer.show_in_widget {
            return Ok(None);
        }
        let config = runtime_config();
        let lines = timer_dashboard_lines(&config.timer)?;
        if lines.is_empty() {
            return Ok(None);
        }
        let summary = lines.join(" | ");
        let full_content = match config.timer.mode {
            TimerWidgetMode::Compact => lines.join(" | "),
            TimerWidgetMode::Full => lines.join("\n"),
        };
        Ok(Some(PluginWidget {
            name: "timer".to_string(),
            display_name: "Timers".to_string(),
            description: Some("Shows active countdowns and stopwatches".to_string()),
            enabled_by_default: true,
            title: "Timers".to_string(),
            refresh_interval_secs: Some(1),
            full: PluginWidgetBody::Text { content: full_content },
            compact: Some(PluginWidgetBody::Text { content: summary }),
        }))
    }
}

impl DashboardDataWidget for TaskWidget {
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn render(&self, compact: bool) -> Result<Option<PluginWidget>, String> {
        let config = runtime_config();
        let tasks = display_tasks(
            &load_tasks()?.tasks,
            &config.tasks,
            Some(if compact { 3 } else { config.tasks.max_display }),
        );
        if tasks.is_empty() {
            return Ok(None);
        }

        let items = tasks
            .iter()
            .map(|task| {
                format!(
                    "{} {}",
                    if task.done { "[x]" } else { "[ ]" },
                    task.text
                )
            })
            .collect::<Vec<_>>();

        Ok(Some(PluginWidget {
            name: "tasks".to_string(),
            display_name: "Tasks".to_string(),
            description: Some("Shows pending task items".to_string()),
            enabled_by_default: true,
            title: "Tasks".to_string(),
            refresh_interval_secs: Some(5),
            full: PluginWidgetBody::List {
                items: items.clone(),
            },
            compact: Some(PluginWidgetBody::Text {
                content: items.first().cloned().unwrap_or_default(),
            }),
        }))
    }
}

impl DashboardDataWidget for NotesWidget {
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn render(&self, _compact: bool) -> Result<Option<PluginWidget>, String> {
        if !runtime_config().notes.show_in_widget {
            return Ok(None);
        }
        let items = dashboard_note_lines()?;
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(PluginWidget {
            name: "notes".to_string(),
            display_name: "Notes".to_string(),
            description: Some("Shows recent quick notes".to_string()),
            enabled_by_default: true,
            title: "Notes".to_string(),
            refresh_interval_secs: Some(5),
            full: PluginWidgetBody::List {
                items: items.clone(),
            },
            compact: Some(PluginWidgetBody::Text {
                content: items.first().cloned().unwrap_or_default(),
            }),
        }))
    }
}

impl DashboardDataWidget for HistoryWidget {
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn render(&self, compact: bool) -> Result<Option<PluginWidget>, String> {
        let items = recent_history(if compact { 3 } else { 5 })?;
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(PluginWidget {
            name: "history".to_string(),
            display_name: "History".to_string(),
            description: Some("Shows recent shell commands".to_string()),
            enabled_by_default: true,
            title: "History".to_string(),
            refresh_interval_secs: Some(10),
            full: PluginWidgetBody::List {
                items: items.clone(),
            },
            compact: Some(PluginWidgetBody::Text {
                content: items.first().cloned().unwrap_or_default(),
            }),
        }))
    }
}

impl DashboardDataWidget for ReminderWidget {
    fn refresh_interval(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn render(&self, compact: bool) -> Result<Option<PluginWidget>, String> {
        let reminders = upcoming_reminders()?;
        if reminders.is_empty() {
            return Ok(None);
        }

        let items = reminders
            .into_iter()
            .take(if compact { 3 } else { 5 })
            .map(|reminder| {
                let remaining = reminder.trigger_at.saturating_sub(now_unix());
                if remaining == 0 {
                    format!("⏳ {} due now", reminder.message)
                } else {
                    format!("⏳ {} in {}", reminder.message, format_duration(remaining))
                }
            })
            .collect::<Vec<_>>();

        Ok(Some(PluginWidget {
            name: "reminders".to_string(),
            display_name: "Reminders".to_string(),
            description: Some("Shows upcoming reminders".to_string()),
            enabled_by_default: true,
            title: "Reminders".to_string(),
            refresh_interval_secs: Some(5),
            full: PluginWidgetBody::List {
                items: items.clone(),
            },
            compact: Some(PluginWidgetBody::Text {
                content: items.first().cloned().unwrap_or_default(),
            }),
        }))
    }
}

pub struct ProductivityWidgetManager {
    timer: TimerWidget,
    tasks: TaskWidget,
    notes: NotesWidget,
    history: HistoryWidget,
    reminders: ReminderWidget,
}

impl ProductivityWidgetManager {
    pub fn new() -> Self {
        Self {
            timer: TimerWidget,
            tasks: TaskWidget,
            notes: NotesWidget,
            history: HistoryWidget,
            reminders: ReminderWidget,
        }
    }

    pub fn render(&self, id: &str, compact: bool) -> Result<Option<(PluginWidget, Duration)>, String> {
        let widget: &dyn DashboardDataWidget = match id {
            "timer" => &self.timer,
            "tasks" => &self.tasks,
            "notes" => &self.notes,
            "history" => &self.history,
            "reminders" => &self.reminders,
            _ => return Ok(None),
        };
        Ok(widget
            .render(compact)?
            .map(|payload| (payload, widget.refresh_interval())))
    }
}

fn runtime_config() -> Config {
    Config::load_or_create().unwrap_or_default()
}

fn display_tasks(tasks: &[TaskItem], settings: &TasksConfig, limit: Option<usize>) -> Vec<TaskItem> {
    let mut items = tasks
        .iter()
        .filter(|task| settings.show_completed || !task.done)
        .cloned()
        .collect::<Vec<_>>();

    match settings.sort_order {
        TaskSortOrder::Created => items.sort_by_key(|task| (task.created_at, task.id)),
        TaskSortOrder::Status => items.sort_by_key(|task| (task.done, task.created_at, task.id)),
    }

    if let Some(limit) = limit {
        items.truncate(limit.max(1));
    }

    items
}

fn toggle_task(id: u64, settings: &TasksConfig) -> Result<(), String> {
    let mut store = load_tasks()?;
    let task_index = store
        .tasks
        .iter()
        .position(|task| task.id == id)
        .ok_or_else(|| format!("Task #{id} was not found."))?;

    if store.tasks[task_index].done {
        store.tasks[task_index].done = false;
    } else if settings.auto_remove_completed {
        store.tasks.remove(task_index);
        save_tasks(&store)?;
        return Ok(());
    } else {
        store.tasks[task_index].done = true;
    }

    save_tasks(&store)
}

fn task_menu_prompt(settings: &TasksConfig) -> String {
    format!(
        "Tasks | show_completed={} sort={} max_display={} auto_remove_completed={}",
        settings.show_completed,
        match settings.sort_order {
            TaskSortOrder::Created => "created",
            TaskSortOrder::Status => "status",
        },
        settings.max_display,
        settings.auto_remove_completed
    )
}

fn choose_task_to_delete(theme: &ColorfulTheme, tasks: &[TaskItem]) -> Result<(), String> {
    if tasks.is_empty() {
        println!("No tasks to delete.");
        return Ok(());
    }

    let items = tasks
        .iter()
        .map(|task| {
            format!(
                "{} {}",
                if task.done { "[x]" } else { "[ ]" },
                task.text
            )
        })
        .chain(std::iter::once("Exit".to_string()))
        .collect::<Vec<_>>();

    let selection = Select::with_theme(theme)
        .with_prompt("Delete task")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| format!("Failed to read delete task selection: {err}"))?;

    match selection {
        Some(index) if index < tasks.len() => delete_task(tasks[index].id),
        _ => Ok(()),
    }
}

fn prompt_task_text() -> Result<Option<String>, String> {
    let result = Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("New task")
        .allow_empty(true)
        .interact_text()
        .map_err(|err| format!("Failed to read task text: {err}"));

    let value = result?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn trim_notes(notes: &mut Vec<NoteItem>, max_stored: usize) {
    let max_stored = max_stored.max(1);
    if notes.len() > max_stored {
        let remove_count = notes.len() - max_stored;
        notes.drain(0..remove_count);
    }
}

fn timer_dashboard_lines(settings: &TimerConfig) -> Result<Vec<String>, String> {
    let store = load_timer_store()?;
    Ok(timer_dashboard_rows(store, settings)
        .into_iter()
        .map(|(label, value)| format!("{label}: {value}"))
        .collect())
}

fn timer_dashboard_rows(store: TimerStore, settings: &TimerConfig) -> Vec<(String, String)> {
    let mut lines = Vec::new();
    if let Some(countdown) = visible_countdown(&store, settings) {
        lines.push((
            "Timer".to_string(),
            countdown_status_line(countdown, settings),
        ));
    }
    if let Some(stopwatch) = store.stopwatch {
        lines.push((
            "Stopwatch".to_string(),
            stopwatch_status_line(&stopwatch, settings),
        ));
    }
    if lines.is_empty() {
        lines.push((
            "Status".to_string(),
            "No active timers or stopwatches.".to_string(),
        ));
    }
    lines
}

fn stopwatch_status_line(state: &TimerState, settings: &TimerConfig) -> String {
    let elapsed = now_unix().saturating_sub(state.started_at);
    match settings.mode {
        TimerWidgetMode::Compact => format_hms(elapsed),
        TimerWidgetMode::Full => format!("{} elapsed", format_hms(elapsed)),
    }
}

fn countdown_status_line(state: &TimerState, settings: &TimerConfig) -> String {
    let duration = state.duration_secs.unwrap_or_default();
    let elapsed = now_unix().saturating_sub(state.started_at);
    let remaining = duration.saturating_sub(elapsed);
    if remaining == 0 {
        "completed".to_string()
    } else {
        match settings.mode {
            TimerWidgetMode::Compact => format_hms(remaining),
            TimerWidgetMode::Full => format!("{} remaining", format_hms(remaining)),
        }
    }
}

fn visible_countdown<'a>(store: &'a TimerStore, settings: &TimerConfig) -> Option<&'a TimerState> {
    let countdown = store.countdown.as_ref()?;
    if !settings.hide_when_complete {
        return Some(countdown);
    }
    if countdown_is_running(countdown) || countdown_completed_recently(countdown) {
        Some(countdown)
    } else {
        None
    }
}

fn countdown_is_running(state: &TimerState) -> bool {
    let duration = state.duration_secs.unwrap_or_default();
    let elapsed = now_unix().saturating_sub(state.started_at);
    elapsed < duration
}

fn countdown_completed_recently(state: &TimerState) -> bool {
    let duration = state.duration_secs.unwrap_or_default();
    if duration == 0 {
        return false;
    }
    let completed_at = state.started_at.saturating_add(duration);
    let now = now_unix();
    now >= completed_at && now.saturating_sub(completed_at) <= COMPLETED_TIMER_GRACE_SECS
}

fn recent_history(limit: usize) -> Result<Vec<String>, String> {
    let path = history_file_path()?;
    let contents = fs::read_to_string(path).unwrap_or_default();
    let mut lines = contents
        .lines()
        .map(parse_history_line)
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.reverse();
    lines.truncate(limit.max(1));
    Ok(lines)
}

fn parse_history_line(line: &str) -> &str {
    if let Some(idx) = line.find(';') {
        if line.starts_with(':') {
            return &line[idx + 1..];
        }
    }
    line
}

fn history_file_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("HISTFILE") {
        return Ok(PathBuf::from(path));
    }
    let home = home_dir_path();
    let shell = std::env::var("SHELL").unwrap_or_default();
    let path = if shell.contains("zsh") {
        home.join(".zsh_history")
    } else if shell.contains("fish") {
        home.join(".local/share/fish/fish_history")
    } else {
        home.join(".bash_history")
    };
    Ok(path)
}

fn notify_user(message: &str, settings: &RemindersConfig) -> io::Result<()> {
    if !settings.enable_notifications {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if settings.visual_alert {
            let script = format!("display notification {:?} with title \"Terminal Info\"", message);
            let _ = Command::new("osascript").arg("-e").arg(script).status();
        }
    }
    #[cfg(target_os = "linux")]
    {
        if settings.visual_alert {
            let _ = Command::new("notify-send")
                .arg("Terminal Info")
                .arg(message)
                .status();
        }
    }
    #[cfg(target_os = "windows")]
    {
        let command = if settings.sound_alert {
            format!("[console]::beep(1200,300); Write-Output {:?}", message)
        } else {
            format!("Write-Output {:?}", message)
        };
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &command])
            .status();
    }

    let mut stderr = io::stderr();
    if settings.sound_alert {
        writeln!(stderr, "\x07Reminder: {message}")?;
    } else {
        writeln!(stderr, "Reminder: {message}")?;
    }
    stderr.flush()
}

pub fn trigger_due_reminders() -> Result<Vec<String>, String> {
    let settings = runtime_config().reminders;
    let mut store = load_reminders()?;
    let now = now_unix();
    let mut triggered_messages = Vec::new();
    let mut changed = false;

    for reminder in &mut store.reminders {
        if !reminder.triggered && now >= reminder.trigger_at {
            reminder.triggered = true;
            triggered_messages.push(reminder.message.clone());
            changed = true;
        }
    }

    if changed {
        save_reminders(&store)?;
    }

    for message in &triggered_messages {
        let _ = notify_user(message, &settings);
    }

    Ok(triggered_messages)
}

fn upcoming_reminders() -> Result<Vec<ReminderItem>, String> {
    let mut reminders = load_reminders()?
        .reminders
        .into_iter()
        .filter(|item| !item.triggered)
        .collect::<Vec<_>>();
    reminders.sort_by_key(|item| item.trigger_at);
    Ok(reminders)
}

fn load_timer_store() -> Result<TimerStore, String> {
    load_json_file(TIMER_FILE)
}

fn save_timer_store(store: &TimerStore) -> Result<(), String> {
    write_json_file(TIMER_FILE, store)
}

fn load_tasks() -> Result<TaskStore, String> {
    let mut store: TaskStore = load_json_file(TASKS_FILE)?;
    if purge_expired_deleted_tasks(&mut store) {
        save_tasks(&store)?;
    }
    Ok(store)
}

fn save_tasks(store: &TaskStore) -> Result<(), String> {
    write_json_file(TASKS_FILE, store)
}

fn load_notes() -> Result<NoteStore, String> {
    load_json_file(NOTES_FILE)
}

fn save_notes(store: &NoteStore) -> Result<(), String> {
    write_json_file(NOTES_FILE, store)
}

fn load_reminders() -> Result<ReminderStore, String> {
    let path = data_file(REMINDERS_FILE)?;
    if !path.exists() {
        return Ok(ReminderStore::default());
    }

    let contents =
        fs::read_to_string(&path).map_err(|err| format!("Failed to read {REMINDERS_FILE}: {err}"))?;
    if contents.trim().is_empty() {
        return Ok(ReminderStore::default());
    }

    serde_json::from_str::<Vec<ReminderItem>>(&contents)
        .map(|reminders| ReminderStore { reminders })
        .or_else(|_| serde_json::from_str::<ReminderStore>(&contents))
        .or_else(|_| {
            serde_json::from_str::<LegacyReminderStore>(&contents).map(|legacy| ReminderStore {
                reminders: legacy
                    .reminders
                    .into_iter()
                    .map(|item| ReminderItem {
                        id: format!("r-legacy-{}", item.id),
                        message: item.message,
                        trigger_at: item.due_at,
                        triggered: false,
                    })
                    .collect(),
            })
        })
        .map_err(|err| format!("Failed to parse {REMINDERS_FILE}: {err}"))
}

fn save_reminders(store: &ReminderStore) -> Result<(), String> {
    write_json_file(REMINDERS_FILE, &store.reminders)
}

fn clear_file(name: &str) -> Result<(), String> {
    let path = data_file(name)?;
    if path.exists() {
        fs::remove_file(path).map_err(|err| format!("Failed to remove {name}: {err}"))?;
    }
    Ok(())
}

fn data_file(name: &str) -> Result<PathBuf, String> {
    let dir = data_dir_path()?;
    fs::create_dir_all(&dir).map_err(|err| format!("Failed to create data directory: {err}"))?;
    Ok(dir.join(name))
}

fn load_json_file<T>(name: &str) -> Result<T, String>
where
    T: Default + for<'de> Deserialize<'de>,
{
    let path = data_file(name)?;
    if !path.exists() {
        return Ok(T::default());
    }
    let contents = fs::read_to_string(path).map_err(|err| format!("Failed to read {name}: {err}"))?;
    serde_json::from_str(&contents).map_err(|err| format!("Failed to parse {name}: {err}"))
}

fn write_json_file<T>(name: &str, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let path = data_file(name)?;
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("Failed to serialize {name}: {err}"))?;
    fs::write(path, format!("{json}\n")).map_err(|err| format!("Failed to write {name}: {err}"))
}

fn next_id(current: &mut u64) -> u64 {
    let next = (*current).max(1);
    *current = next.saturating_add(1);
    next
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

fn parse_duration(input: &str) -> Result<u64, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Duration cannot be empty.".to_string());
    }

    let mut total = 0_u64;
    let mut digits = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }

        if digits.is_empty() {
            return Err(format!("Invalid duration '{}'.", input));
        }

        let value: u64 = digits
            .parse()
            .map_err(|_| format!("Invalid duration '{}'.", input))?;
        digits.clear();
        total = total.saturating_add(match ch {
            's' | 'S' => value,
            'm' | 'M' => value.saturating_mul(60),
            'h' | 'H' => value.saturating_mul(60 * 60),
            'd' | 'D' => value.saturating_mul(60 * 60 * 24),
            _ => return Err(format!("Invalid duration unit '{}' in '{}'.", ch, input)),
        });
    }

    if !digits.is_empty() {
        let value: u64 = digits
            .parse()
            .map_err(|_| format!("Invalid duration '{}'.", input))?;
        total = total.saturating_add(value.saturating_mul(60));
    }

    if total == 0 {
        return Err("Duration must be greater than zero.".to_string());
    }
    Ok(total)
}

fn parse_reminder_target(input: &str) -> Result<(u64, String), String> {
    let trimmed = input.trim();
    if trimmed.contains(':') {
        let clock =
            NaiveTime::parse_from_str(trimmed, "%H:%M").map_err(|_| format!("Invalid time '{}'. Use HH:MM or a duration like 15m.", input))?;
        let now = Local::now();
        let today = now.date_naive();
        let mut target = match Local.from_local_datetime(&today.and_time(clock)) {
            LocalResult::Single(value) => value,
            LocalResult::Ambiguous(first, _) => first,
            LocalResult::None => {
                return Err(format!("Invalid local reminder time '{}'.", input));
            }
        };
        if target.timestamp() <= now.timestamp() {
            let tomorrow = today
                .succ_opt()
                .ok_or_else(|| "Failed to schedule reminder for tomorrow.".to_string())?;
            target = match Local.from_local_datetime(&tomorrow.and_time(clock)) {
                LocalResult::Single(value) => value,
                LocalResult::Ambiguous(first, _) => first,
                LocalResult::None => {
                    return Err(format!("Invalid local reminder time '{}'.", input));
                }
            };
        }
        return Ok((
            target.timestamp().max(0) as u64,
            target.format("%Y-%m-%d %H:%M").to_string(),
        ));
    }

    let duration_secs = parse_duration(trimmed)?;
    let trigger_at = now_unix().saturating_add(duration_secs);
    let formatted = Local
        .timestamp_opt(trigger_at as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| format_duration(duration_secs));
    Ok((trigger_at, formatted))
}

fn format_duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

fn format_hms(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duration_units() {
        assert_eq!(parse_duration("90").unwrap(), 5400);
        assert_eq!(parse_duration("15m").unwrap(), 900);
        assert_eq!(parse_duration("1h30m").unwrap(), 5400);
    }

    #[test]
    fn parses_zsh_history_lines() {
        assert_eq!(parse_history_line(": 1710000000:0;git status"), "git status");
        assert_eq!(parse_history_line("cargo test"), "cargo test");
    }

    #[test]
    fn countdown_running_detects_completion() {
        let running = TimerState {
            started_at: now_unix().saturating_sub(2),
            duration_secs: Some(5),
        };
        let completed = TimerState {
            started_at: now_unix().saturating_sub(5),
            duration_secs: Some(3),
        };

        assert!(countdown_is_running(&running));
        assert!(!countdown_is_running(&completed));
    }
}
