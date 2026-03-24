# Dashboard

When `tinfo` is run with no arguments, it shows a simple dashboard instead of command help.

## Current Sections

The dashboard currently displays:

- `Location`
- `Weather`
- `Time`
- `Network`
- `CPU`
- `Memory`
- `Notes` when local dashboard notes exist
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
widgets = ["weather", "time", "network", "system", "plugins"]
```

Supported widget names are:

- `weather`
- `time`
- `network`
- `system`
- `notes`
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

## Notes Widget

The built-in `notes` widget reads from:

```text
~/.tinfo/dashboard-notes.txt
```

Manage it with:

```bash
tinfo dashboard notes show
tinfo dashboard notes set remember to rotate keys
tinfo dashboard notes clear
```

The widget is intentionally small and non-blocking. The dashboard reads the file and refreshes it on a short interval without pausing the rest of the render loop.

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
