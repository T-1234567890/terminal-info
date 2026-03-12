# Commands

This document describes the current `tinfo` command set.

## Top Level

```bash
tinfo weather
tinfo config
tinfo update
```

Top-level help groups the CLI as:

- `weather` for weather-related commands
- `config` for configuration settings
- `update` for self-updating the installed binary

## `tinfo weather now`

Show current weather.

Usage:

```bash
tinfo weather now
tinfo weather now <city>
```

Resolution order:

1. Use the provided `<city>` argument
2. Use the configured default location
3. Attempt IP-based location detection

If all methods fail, `tinfo` prints:

```text
Unable to detect location. Use `tinfo weather location <city>` to set a default location.
```

Examples:

```bash
tinfo weather now
tinfo weather now tokyo
tinfo weather now shenzhen
```

## `tinfo weather forecast`

Show a short forecast.

Usage:

```bash
tinfo weather forecast
tinfo weather forecast <city>
```

Behavior:

- Uses the provided city when given
- Otherwise uses the configured default location
- Does not currently fall back to IP lookup

Examples:

```bash
tinfo weather forecast
tinfo weather forecast london
```

## `tinfo weather location`

Show or set the default location.

Usage:

```bash
tinfo weather location
tinfo weather location <city>
```

Examples:

```bash
tinfo weather location
tinfo weather location tokyo
```

## `tinfo config`

Open the interactive configuration menu.

Usage:

```bash
tinfo config
```

Menu actions:

1. Set default location
2. Use IP location as default
3. Remove default location
4. Set units
5. Show current config
6. Exit

## `tinfo config api`

Show API provider configuration.

Usage:

```bash
tinfo config api
tinfo config api show
```

## `tinfo config api set`

Set an API provider and API key.

Usage:

```bash
tinfo config api set openweather YOUR_API_KEY
```

## `tinfo config units`

Set display units.

Usage:

```bash
tinfo config units metric
tinfo config units imperial
```

Values:

- `metric`
- `imperial`

## `tinfo update`

Download and install the latest released version of `tinfo`.

Usage:

```bash
tinfo update
```

## Help

Display top-level help:

```bash
tinfo --help
```

Display help for a subcommand:

```bash
tinfo weather --help
tinfo weather now --help
tinfo config --help
```
