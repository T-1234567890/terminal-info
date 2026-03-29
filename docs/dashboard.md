# Dashboard

When `tinfo` is run with no arguments, it shows a simple dashboard instead of command help.

## Current Sections

The dashboard displays responsive widget boxes for:

- `Location`
- `Weather`
- `Time`
- `Network`
- `CPU`
- `Memory`
- `Timer`, `Tasks`, `Notes`, `History`, and `Reminder` when local productivity state exists
- `Plugins` when trusted plugins expose widgets

Example:

```text
+---------------+
| Terminal Info |
+---------------+
Location: Shenzhen
Weather: Partly cloudy, 27.0°C
Time: 2026-03-12 22:10:00
Network: 192.168.1.8
CPU: 8.3%
Memory: 7.2 GiB / 16.0 GiB used
```

## Layout Behavior

The dashboard now supports three layout modes in `~/.tinfo/config.toml`:

```toml
[dashboard]
layout = "auto"
columns = 2
```

- `vertical` stacks widget boxes top to bottom
- `horizontal` renders them in multiple columns
- `auto` switches layouts based on terminal width

When the terminal is narrow, the dashboard falls back to a single column. On wider terminals it will tile widgets side by side and truncate long lines to avoid overflow.

## How It Works

The dashboard is implemented in:

- `src/dashboard.rs`

`main.rs` calls `show_dashboard()` when no command is provided.

The dashboard uses lightweight helper logic to gather:

- local time
- current weather summary when a location or IP detection can resolve a city
- local network summary
- CPU usage
- memory usage

Weather failures are shown with short, actionable hints instead of a generic unavailable state.

## Widget Order

Dashboard widgets can be ordered in the config file:

```toml
[dashboard]
widgets = ["weather", "time", "network", "system", "timer", "tasks", "notes", "history", "reminders", "plugins"]
```

Supported widget names are:

- `weather`
- `time`
- `network`
- `system`
- `timer`
- `tasks`
- `notes`
- `history`
- `reminders`
- `plugins`

Plugin widgets do not render terminal UI directly. They return structured JSON through `--widget`, and the core dashboard renders that payload in compact or full mode.

For the full widget configuration and plugin widget API reference, see [widgets.md](widgets.md).

Example widget payload:

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

The dashboard still accepts the legacy `{ "title": "...", "content": "..." }` widget shape for older plugins.

## Productivity Widgets

Productivity widget state is stored in:

```text
~/.tinfo/data/
```

Commands:

```bash
tinfo timer
tinfo timer start 25m
tinfo timer stop
tinfo stopwatch start
tinfo stopwatch stop
tinfo task add finish README
tinfo task list
tinfo task done 1
tinfo note add remember to rotate keys
tinfo note list
tinfo history --limit 10
tinfo remind 15m stand up
tinfo dashboard notes show
tinfo dashboard notes set remember to rotate keys
tinfo dashboard notes clear
```

The widgets are intentionally small and non-blocking. The dashboard reads local JSON state and shell history on short intervals without pausing the rest of the render loop.

The dashboard also acts as the reminder scheduler. `tinfo remind ...` writes reminder data to `~/.tinfo/data/reminders.json`, prints `Note: reminders trigger while the dashboard is running.`, and opens the live dashboard. While the dashboard is running it checks for due reminders, marks them as triggered, rings the terminal bell, and shows a temporary alert row.

For tasks, deleted items are soft-deleted into a recoverable area instead of disappearing immediately. They can be restored from `tinfo task` for 7 days, and expired deleted tasks are purged automatically the next time the CLI loads the task store.

Timer widgets can also be tuned through:

```toml
[timer]
hide_when_complete = true
mode = "compact"
```

- `hide_when_complete = true` shows `completed` briefly before removing the finished countdown
- `mode = "compact"` shortens timer and stopwatch text inside the dashboard

## Freeze Mode

Dashboard freeze mode captures one snapshot and reuses it instead of auto-refreshing.

You can enable it:

- temporarily with `tinfo --freeze`
- override it with `tinfo --live`
- persistently with `dashboard.freeze = true` in `~/.tinfo/config.toml`

When freeze mode is enabled, the live dashboard loop renders once and exits with a static snapshot.

Priority order:

1. `--freeze`
2. `--live`
3. `dashboard.freeze`
4. normal live mode

Examples:

- `tinfo dashboard` uses the configured default
- `tinfo dashboard --freeze` always renders a snapshot
- `tinfo dashboard --live` always renders live updates

## Theme Behavior

Dashboard rendering follows the `[theme]` section in `~/.tinfo/config.toml`.

- `border_style = "sharp"` uses square corners
- `border_style = "rounded"` uses rounded corners when Unicode is enabled
- `ascii_only = true` forces ASCII borders for terminals with limited Unicode support
- `accent_color` colors dashboard borders in `--color` mode and is ignored in plain, compact, and JSON output

## Future Expansion

The dashboard is intentionally small and could expand later with:

- disk summary
- upcoming forecast
- diagnostic status
- update status
