# Commands

This document describes the current `tw` command set.

## `tw now`

Show current weather.

Usage:

```bash
tw now
tw now <city>
```

Resolution order:

1. Use the provided `<city>` argument
2. Use the configured default location
3. Attempt IP-based location detection

If all methods fail, `tw` prints:

```text
Unable to detect location. Use `tw location <city>` to set a default location.
```

Examples:

```bash
tw now
tw now tokyo
tw now shenzhen
```

## `tw forecast`

Show a short forecast.

Usage:

```bash
tw forecast
tw forecast <city>
```

Behavior:

- Uses the provided city when given
- Otherwise uses the configured default location
- Does not currently fall back to IP lookup

Examples:

```bash
tw forecast
tw forecast london
```

## `tw location`

Show or set the default location.

Usage:

```bash
tw location
tw location <city>
```

Examples:

```bash
tw location
tw location tokyo
```

## `tw config`

Open the interactive configuration menu.

Usage:

```bash
tw config
```

Menu actions:

1. Set default location
2. Use IP location as default
3. Remove default location
4. Set units
5. Show current config
6. Exit

## `tw config api`

Show API provider configuration.

Usage:

```bash
tw config api
tw config api show
```

Examples:

```bash
tw config api
tw config api show
```

## `tw config api set`

Set an API provider and API key.

Usage:

```bash
tw config api set openweather YOUR_API_KEY
```

## `tw config units`

Set display units.

Usage:

```bash
tw config units metric
tw config units imperial
```

Values:

- `metric`
- `imperial`

## Help

Display top-level help:

```bash
tw --help
```

Display help for a subcommand:

```bash
tw now --help
tw forecast --help
tw config --help
```
