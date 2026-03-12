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
- current weather summary when a default location is configured
- local network summary
- CPU usage
- memory usage

## Future Expansion

The dashboard is intentionally small and could expand later with:

- disk summary
- plugin-provided widgets
- upcoming forecast
- diagnostic status
- update status
