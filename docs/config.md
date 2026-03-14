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
- `dashboard.compact_mode`
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
widgets = ["weather", "time", "network", "system", "plugins"]
refresh_interval = 1
compact_mode = false
 
[cache]
weather_ttl_secs = 60
network_ttl_secs = 30
time_ttl_secs = 10
```

## Interactive Configuration

Run:

```bash
tinfo config
```

This opens the interactive menu built with `dialoguer`.

Menu sections:

- `Location`
- `Units`
- `API Keys`
- `Server Mode`
- `Reset Config`
- `Exit`

Location submenu:

- `Set location manually`
- `Use IP location`
- `Back`

## Direct Commands

Location:

```bash
tinfo config location
tinfo config location tokyo
```

Units:

```bash
tinfo config units
tinfo config units metric
tinfo config units imperial
```

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
```

Server mode is optional. It is intended for servers or VPS environments and is not recommended for regular desktop computers.

See [server-mode.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/server-mode.md) for the full behavior and command scope.

Reset:

```bash
tinfo config reset
```

Doctor:

```bash
tinfo config doctor
```

Dashboard settings:

```bash
tinfo dashboard config
tinfo dashboard reset
```

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
