# Commands

This document describes the current `tinfo` command set.

## Dashboard

```bash
tinfo
```

Shows the default dashboard.

## Weather

```bash
tinfo weather
tinfo weather now
tinfo weather now <city>
tinfo weather forecast
tinfo weather forecast <city>
tinfo weather hourly
tinfo weather hourly <city>
tinfo weather alerts
tinfo weather alerts <city>
tinfo weather <alias>
tinfo weather location
tinfo weather location <city>
```

## Ping

```bash
tinfo ping
tinfo ping <host>
```

If no host is provided, `tinfo` checks:

- `google.com`
- `cloudflare.com`
- `github.com`

## Network

```bash
tinfo network
```

Displays:

- public IP
- local IP
- DNS
- ISP when available

## System

```bash
tinfo system
```

Displays:

- OS
- CPU
- memory
- disk usage
- uptime

## Time

```bash
tinfo time
tinfo time <city>
```

Without a city, it shows:

- Local
- Tokyo
- London
- New York

## Diagnostic

```bash
tinfo diagnostic
tinfo diagnostic network
tinfo diagnostic system
tinfo diagnostic plugins
```

Runs grouped diagnostics for:

- network checks
- system checks
- plugin checks

## Config

```bash
tinfo config
tinfo config location
tinfo config location <city>
tinfo config units
tinfo config units metric
tinfo config units imperial
tinfo config api
tinfo config api show
tinfo config api set openweather <key>
tinfo config doctor
tinfo config reset
```

Configuration is stored in:

```text
~/.tinfo/config.toml
```

## Profiles

```bash
tinfo profile list
tinfo profile use <name>
```

Profiles are defined in `~/.tinfo/config.toml` under `[profile.<name>]`.

## Completions

```bash
tinfo completion bash
tinfo completion zsh
tinfo completion fish
tinfo completion install
```

## Plugin Management

```bash
tinfo plugin list
tinfo plugin search
tinfo plugin init <name>
tinfo plugin keygen [--output-dir <dir>]
tinfo plugin sign <file> [--key <path>]
tinfo plugin install <name>
tinfo plugin trust <name>
tinfo plugin untrust <name>
tinfo plugin trusted
tinfo plugin info <name>
tinfo plugin verify
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin remove <name>
```

`plugin search` reads the reviewed registry metadata, using the local cache when available.
`plugin keygen` creates `minisign.key` and `minisign.pub` for plugin release signing.
`plugin sign` signs a plugin artifact and writes a sibling `.minisig` file.
`plugin install` downloads the plugin's pinned GitHub release asset and installs it into `~/.terminal-info/plugins/`.
`plugin remove` deletes the installed plugin directory.

## External Plugins

Unknown top-level commands are treated as plugin candidates inside the managed plugin directory.

Example:

```bash
tinfo news tech
```

Attempts to run:

```bash
tinfo-news tech
```

## Update

```bash
tinfo update
```
