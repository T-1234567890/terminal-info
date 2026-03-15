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
tinfo latency
tinfo latency full
tinfo diagnostic
tinfo diagnostic full
tinfo diagnostic network
tinfo diagnostic performance
tinfo diagnostic security
tinfo diagnostic leaks
tinfo diagnostic system
tinfo diagnostic plugins
```

Runs grouped diagnostics for:

- network checks
- system checks
- plugin checks

Normal mode keeps `tinfo diagnostic`, `tinfo diagnostic network`, `tinfo diagnostic system`, `tinfo diagnostic performance`, `tinfo diagnostic full`, `tinfo ping`, `tinfo ping full`, `tinfo latency`, and `tinfo latency full`.

Server mode is required for:
- `tinfo diagnostic security`
- `tinfo diagnostic leaks`

## Server Mode

```bash
tinfo config server status
tinfo config server enable
tinfo config server disable
```

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
tinfo config open
tinfo config edit
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
tinfo completion powershell
tinfo completion install
tinfo completion uninstall
tinfo completion status
```

`completion install` installs completions for the detected shell.
`completion uninstall` removes the installed completion file for the detected shell.
`completion status` shows the detected shell, install path, and whether a completion file exists.

## Dashboard

```bash
tinfo dashboard config
tinfo dashboard reset
```

## Profiles

```bash
tinfo profile list
tinfo profile show <name>
tinfo profile use <name>
tinfo profile add <name>
tinfo profile remove <name>
```

`profile add` captures the current effective settings into a named reusable profile.
`profile use` activates a profile as a runtime overlay without overwriting the base config.
`profile show` displays the stored profile values.
`profile remove` deletes the named profile and clears it if it was active.

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
tinfo plugin doctor
tinfo plugin lint
tinfo plugin publish-check
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin remove <name>
```

`plugin search` reads the reviewed registry metadata, using the local cache when available.
`plugin keygen` creates `minisign.key` and `minisign.pub` for plugin release signing.
`plugin sign` signs a plugin artifact and writes a sibling `.minisig` file.
`plugin doctor` checks installed plugins for manifest, registry, path, checksum, and binary issues.
`plugin lint` validates the current plugin project files.
`plugin publish-check` validates plugin project files and release layout before publishing.
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
tinfo self-repair
tinfo reinstall
```

`self-repair` and `reinstall` force a fresh download of the latest release instead of skipping when the current version already matches.
