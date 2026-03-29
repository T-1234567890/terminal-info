# Configuration

`tinfo` stores user configuration in:

```text
~/.tinfo/config.toml
```

The `~/.tinfo` directory is created automatically when needed.

## Config Format

Minimal example:

```toml
location = "shenzhen"
units = "metric"
```

With profiles:

```toml
location = "shenzhen"
units = "metric"
active_profile = "home"

[profile.home]
location = "shenzhen"
units = "metric"

[profile.work]
location = "tokyo"
units = "imperial"

[profile.travel]
location = "auto"
```

Supported top-level fields:

- `config_version`
- `server_mode`
- `location`
- `units`
- `provider`
- `api_key`
- `active_profile`
- `profile.<name>`
- `locations.<alias>`
- `dashboard.widgets`
- `dashboard.refresh_interval`
- `dashboard.layout`
- `dashboard.columns`
- `dashboard.compact_mode`
- `dashboard.freeze`
- `tasks.show_completed`
- `tasks.sort_order`
- `tasks.max_display`
- `tasks.auto_remove_completed`
- `notes.max_stored`
- `notes.show_in_widget`
- `timer.default_duration`
- `timer.auto_start`
- `timer.show_in_widget`
- `timer.hide_when_complete`
- `timer.mode`
- `reminders.default_duration`
- `reminders.enable_notifications`
- `reminders.sound_alert`
- `reminders.visual_alert`
- `theme.border_style`
- `theme.accent_color`
- `theme.ascii_only`
- `cache.weather_ttl_secs`
- `cache.network_ttl_secs`
- `cache.time_ttl_secs`

`location = "auto"` means weather commands should try IP-based city detection.

Location aliases:

```toml
[locations]
home = "Shenzhen"
work = "Hong Kong"
```

Dashboard widget order:

```toml
[dashboard]
widgets = ["weather", "time", "network", "system", "timer", "tasks", "notes", "history", "reminders", "plugins"]
refresh_interval = 1
layout = "auto"
columns = 2
compact_mode = false
freeze = false

[tasks]
show_completed = true
sort_order = "created"
max_display = 5
auto_remove_completed = false

[notes]
max_stored = 50
show_in_widget = true

[timer]
default_duration = "25m"
auto_start = false
show_in_widget = true
hide_when_complete = true
mode = "full"

[reminders]
default_duration = "15m"
enable_notifications = true
sound_alert = true
visual_alert = true

[theme]
border_style = "sharp"
accent_color = "cyan"
ascii_only = false
 
[cache]
weather_ttl_secs = 60
network_ttl_secs = 30
time_ttl_secs = 10
```

## Interactive Configuration

Run:

```bash
tinfo config
tinfo configure
```

This opens the interactive menu built with `dialoguer`.

Menu sections:

- `Guided Setup`
- `Location`
- `Dashboard`
- `Widgets`
- `Tasks`
- `Notes`
- `Timer`
- `Reminders`
- `Default Output`
- `Theme`
- `Shell Completions`
- `Units`
- `API Keys`
- `Server Mode`
- `Advanced and More Config`
- `Reset Config`
- `Exit`

Location submenu:

- `Set location manually`
- `Use IP location`
- `Back`

## Direct Commands

Location:

```bash
tinfo config setup
tinfo config location
tinfo config location tokyo
```

Units:

```bash
tinfo config units
tinfo config units metric
tinfo config units imperial
```

Default output:

```bash
tinfo config output
tinfo config output color
tinfo config output compact
tinfo config output plain
```

Theme settings:

```bash
tinfo config theme
tinfo config theme border sharp
tinfo config theme border rounded
tinfo config theme accent auto
tinfo config theme accent cyan
tinfo config theme unicode on
tinfo config theme unicode off
```

`theme.border_style` controls the box corners used by the dashboard and boxed reports.
`theme.accent_color` applies only in color mode.
`theme.ascii_only = true` forces `+`, `-`, and `|` borders and ASCII status markers for older or limited terminals.

Dashboard layout settings:

- `layout = "auto"` is the default and switches to multiple columns on wider terminals
- `layout = "vertical"` keeps the classic stacked layout
- `layout = "horizontal"` tiles widgets side by side
- `columns` lets you override the preferred column count for `horizontal` or `auto`
- `dashboard.widgets` stores the enabled widget list; widgets not listed are disabled
- `tinfo config` now includes a widget picker that shows built-in and trusted plugin widgets together
- in the widget picker, `Enter` toggles a widget and `q` saves and exits

Timer widget settings:

- `hide_when_complete = true` shows `completed` briefly and then removes the countdown row
- `mode = "compact"` shortens timer and stopwatch text inside the dashboard widget

API settings:

```bash
tinfo config api
tinfo config api show
tinfo config api set openweather YOUR_API_KEY
```

Server mode:

```bash
tinfo config server status
tinfo config server enable
tinfo config server disable
tinfo config open
tinfo config edit
```

Server mode is optional. It is intended for servers or VPS environments and is not recommended for regular desktop computers.

See [server-mode.md](server-mode.md) for the full behavior and command scope.

Reset:

```bash
tinfo config reset
```

Open with the system default app:

```bash
tinfo config open
```

`tinfo config open` opens the TOML config file directly with the operating system default app.

Edit in terminal editor:

```bash
tinfo config edit
```

`tinfo config edit` opens the TOML config file using:

1. `$EDITOR`
2. `nano`
3. `vim`

Doctor:

```bash
tinfo config doctor
```

After the interactive `tinfo config` menu exits, Terminal Info also prints a short advanced-configuration hint with the config file path plus `tinfo config open` and `tinfo config edit`.

Dashboard settings:

```bash
tinfo dashboard config
tinfo dashboard reset
tinfo dashboard notes show
tinfo dashboard notes set remember to check disk health
tinfo dashboard notes clear
```

`dashboard.freeze = true` makes the dashboard render a single static snapshot by default.
Use `tinfo --live` to override that setting for one run, or `tinfo --freeze` to force snapshot mode explicitly.
See [widgets.md](widgets.md) for widget ordering, notes, and plugin widget behavior.

Fast widget commands:

```bash
tinfo config widgets
tinfo config widgets show
tinfo config widgets add timer
tinfo config widgets remove network
tinfo config widgets set weather time system timer tasks notes history reminders plugins
tinfo config widgets reset
```

Productivity tool state is stored separately in:

```text
~/.tinfo/data/
```

This directory contains small JSON files for timers, tasks, notes, and reminders.

Productivity settings:

- `Tasks` controls completed-task visibility, task sort order, widget limits, and auto-removal
- `Notes` controls note retention and widget visibility
- `Timer` controls default duration, auto-start, and widget visibility
- `Reminders` controls default reminder duration plus notification, sound, and visual alert behavior

Task menu behavior:

- `tinfo task` uses the same prompt-and-select interaction style as `tinfo config`
- the menu uses `Exit` as the close action
- `Deleted tasks` shows recoverable tasks still within the 7-day retention window
- expired deleted tasks are purged automatically when the task store is opened

## Profiles

Profiles are named configuration blocks stored under `[profile.<name>]`.

Commands:

```bash
tinfo profile list
tinfo profile show home
tinfo profile use home
tinfo profile add home
tinfo profile remove work
```

`tinfo profile use <name>` activates the stored profile values as runtime overrides and saves `active_profile`.
`tinfo profile add <name>` saves the current effective config values into a named profile.
`tinfo profile show <name>` prints the stored values for that profile.
`tinfo profile remove <name>` deletes the named profile.

Example home/office setup:

```toml
location = "Shenzhen"

[dashboard]
widgets = ["weather", "time", "network", "system", "plugins"]
refresh_interval = 1
compact_mode = false

[profile.home]
location = "Shenzhen"
units = "metric"

[profile.home.dashboard]
widgets = ["weather", "time", "plugins"]
refresh_interval = 1
compact_mode = false

[profile.office]
location = "Tokyo"
units = "metric"

[profile.office.dashboard]
widgets = ["weather", "time", "network"]
refresh_interval = 2
compact_mode = true
```

When no profile is active, Terminal Info uses the top-level config values.
When a profile is active, profile values override the top-level config for supported fields while leaving the base config intact.

## Migration

On startup, Terminal Info automatically checks for:

- legacy `~/.tw/config.json`
- older config schema versions
- legacy plugin paths under `~/.tinfo/plugins`

When a migration changes files, Terminal Info writes a backup first.

## IP-Based Location

When no explicit city is provided, `tinfo` can detect a city from:

```text
https://ipapi.co/json/
https://ipinfo.io/json
https://ipwho.is/
```

This is a network lookup only. It does not request GPS or OS location permissions.

Detected IP location is cached locally for 6 hours to reduce rate limits and improve dashboard reliability.

## Output Modes

All commands support these global flags:

- `--plain`
- `--compact`
- `--color`
- `--json`

Examples:

```bash
tinfo --plain weather now
tinfo --compact diagnostic
tinfo --color
```
