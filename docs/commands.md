# Commands

This document describes the current `tinfo` command set.

## Dashboard

```bash
tinfo
```

Shows the default dashboard.

## Weather

```bash
tinfo weather now
tinfo weather now <city>
tinfo weather forecast
tinfo weather forecast <city>
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
```

## Plugin Management

```bash
tinfo plugin list
tinfo plugin search
tinfo plugin init <name>
tinfo plugin install <name>
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin remove <name>
```

`plugin search` reads the local plugin index in `plugins/*.json`.
`plugin install` downloads the plugin's GitHub release asset and installs it into `~/.tinfo/plugins/`.
`plugin remove` deletes the installed plugin binary.

## External Plugins

Unknown top-level commands are treated as plugin candidates.

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
