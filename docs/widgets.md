# Widgets

Terminal Info widgets are small dashboard data sources. They do not own layout or terminal rendering. The dashboard loop collects widget data, chooses compact or full mode, and renders everything centrally.

Widgets come in two forms:

- built-in widgets such as `weather`, `time`, `network`, `system`, `timer`, `tasks`, `calendar`, `notes`, `history`, and `reminders`
- plugin widgets returned by trusted plugins through `--widget`

## Dashboard Configuration

Widget order is configured in `~/.tinfo/config.toml`:

```toml
[dashboard]
widgets = ["weather", "time", "network", "system", "timer", "tasks", "calendar", "notes", "history", "reminders", "plugins"]
refresh_interval = 1
layout = "auto"
columns = 2
compact_mode = false
freeze = false
```

Supported built-in widget names:

- `weather`
- `time`
- `network`
- `system`
- `timer`
- `tasks`
- `calendar`
- `notes`
- `history`
- `reminders`
- `plugins`

Notes:

- `plugins` renders all trusted plugin widgets
- productivity widgets read local state from `~/.tinfo/data/`
- `notes` renders recent note entries from `~/.tinfo/data/notes.json`
- `tasks` renders the latest tasks from `~/.tinfo/data/tasks.json`
- `calendar` renders upcoming tasks with a calendar date from `~/.tinfo/data/tasks.json`
- `timer` renders the active stopwatch or countdown from `~/.tinfo/data/timer.json`
- `history` renders recent shell history entries
- `reminders` renders the next scheduled reminder from `~/.tinfo/data/reminders.json`
- reminder entries include `id`, `message`, `trigger_at`, and `triggered`
- `freeze = true` captures one snapshot and reuses it instead of refreshing live
- `refresh_interval` controls the dashboard loop; plugin widgets can also provide their own refresh hint
- `layout = "auto"` uses a responsive multi-column layout on wider terminals
- `layout = "horizontal"` forces side-by-side widget boxes
- `columns` can pin the preferred number of dashboard columns

Quick widget commands:

```bash
tinfo config widgets show
tinfo config widgets add timer
tinfo config widgets remove network
tinfo config widgets set weather time system timer tasks calendar notes history reminders plugins
tinfo config widgets reset
```

Interactive widget configuration is available from:

```bash
tinfo config
```

Then open the `Widgets` submenu.

## Productivity Widgets

Built-in productivity widgets use small JSON files in:

```text
~/.tinfo/data/
```

Commands:

```bash
tinfo timer
tinfo timer start
tinfo timer start 25m
tinfo timer stop
tinfo stopwatch start
tinfo stopwatch stop
tinfo task
tinfo task add finish README
tinfo task list
tinfo task done 1
tinfo task delete 1
tinfo note add remember to rotate keys
tinfo note list
tinfo history --limit 10
tinfo remind
tinfo remind 15m stand up
tinfo remind 14:30 stand up
tinfo dashboard notes show
tinfo dashboard notes set remember to rotate keys
tinfo dashboard notes clear
```

Rendering behavior:

- `timer` shows the active countdown and stopwatch state together and refreshes every second
- `timer.hide_when_complete = true` keeps `completed` visible briefly before removing the finished countdown
- `timer.mode = "compact"` shortens timer text in the dashboard widget
- `tasks` shows recent tasks based on the task settings in `config.toml`
- `calendar` shows the next upcoming scheduled task, with a countdown for timed events, and up to three events in full mode
- deleted tasks are not shown in the task widget
- deleted tasks remain recoverable for 7 days before automatic removal
- `notes` shows recent note entries as a list
- `history` shows recent shell commands without requiring shell integration beyond a normal history file
- `reminders` shows upcoming reminders as a list and refreshes every few seconds
- compact mode reduces each widget to a single summary line
- all productivity widgets are re-read on short intervals and do not block the rest of the dashboard
- `tinfo remind ...` schedules the reminder and opens the live dashboard so the scheduler is active immediately
- dashboard widget boxes adapt to terminal width and truncate overly long rows instead of overflowing

## Plugin Widget API

Plugins should return structured widget data, not terminal UI.

In the Rust SDK, a plugin registers a widget handler with:

```rust
Plugin::new("weather")
    .widget(dashboard_widget)
    .dispatch();
```

The handler signature is:

```rust
fn dashboard_widget(ctx: tinfo_plugin::Context, mode: tinfo_plugin::WidgetMode)
    -> tinfo_plugin::PluginResult<tinfo_plugin::Widget>
```

Available widget mode values:

- `WidgetMode::Compact`
- `WidgetMode::Full`

Available widget body types:

- `WidgetBody::text(...)`
- `WidgetBody::list(...)`
- `WidgetBody::table(...)`

Minimal example:

```rust
use tinfo_plugin::{Plugin, PluginResult, Widget, WidgetBody, WidgetMode};

fn dashboard_widget(ctx: tinfo_plugin::Context, _mode: WidgetMode) -> PluginResult<Widget> {
    let city = ctx.config.string("location")?.unwrap_or_else(|| "auto".to_string());

    Ok(
        Widget::new(
            "Weather",
            WidgetBody::table(
                ["Field", "Value"],
                [["City", city.as_str()], ["Host", ctx.host.version()]],
            ),
        )
        .compact(WidgetBody::text(format!("city={city}")))
        .refresh_interval_secs(30),
    )
}

fn main() {
    Plugin::new("weather")
        .description("Weather information plugin")
        .widget(dashboard_widget)
        .dispatch();
}
```

## Widget JSON Schema

The host reads widget JSON from:

```bash
tinfo-<plugin-name> --widget
tinfo-<plugin-name> --widget --compact
```

Stable schema:

```json
{
  "title": "CPU",
  "refresh_interval_secs": 2,
  "full": {
    "type": "table",
    "headers": ["Metric", "Value"],
    "rows": [["Usage", "18%"], ["Load", "1.42"]]
  },
  "compact": {
    "type": "text",
    "content": "18%"
  }
}
```

Body variants:

`text`

```json
{
  "type": "text",
  "content": "18%"
}
```

`list`

```json
{
  "type": "list",
  "items": ["task one", "task two"]
}
```

`table`

```json
{
  "type": "table",
  "headers": ["Metric", "Value"],
  "rows": [["Usage", "18%"], ["Load", "1.42"]]
}
```

Semantics:

- `title` is required
- `full` is required
- `compact` is optional; if omitted, the host falls back to `full`
- `refresh_interval_secs` is optional and treated as a hint, not a hard contract

## Legacy Compatibility

Older plugins can still return:

```json
{
  "title": "News",
  "content": "3 unread items"
}
```

The host converts that into a text widget automatically. New plugins should use the structured schema.

## Render Model

The dashboard remains intentionally simple:

- the dashboard loop owns refresh timing
- each widget provides data, not layout
- compact and full rendering are selected by the host
- widget refresh hints are used for caching, not for independent background threads

This keeps the widget system stable, easy to document, and easy to implement in both built-in and plugin widgets.
