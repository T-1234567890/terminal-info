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

- `location`
- `units`
- `provider`
- `api_key`
- `active_profile`
- `profile.<name>`

`location = "auto"` means weather commands should try IP-based city detection.

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

Reset:

```bash
tinfo config reset
```

## Profiles

Profiles are named configuration blocks stored under `[profile.<name>]`.

Commands:

```bash
tinfo profile list
tinfo profile use home
```

`tinfo profile use <name>` applies the stored profile values to the active config and saves `active_profile`.

## IP-Based Location

When no explicit city is provided, `tinfo` can detect a city from:

```text
https://ipapi.co/json/
```

This is a network lookup only. It does not request GPS or OS location permissions.

## Output Modes

All commands support these global flags:

- `--plain`
- `--compact`
- `--color`

Examples:

```bash
tinfo --plain weather now
tinfo --compact diagnostic
tinfo --color
```
