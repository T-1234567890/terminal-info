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
- `plugins`

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
