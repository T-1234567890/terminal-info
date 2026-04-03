# Commands

This document describes the `tinfo` command set.

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
tinfo diagnostic --markdown-out ./diagnostic.md
tinfo diagnostic full
tinfo diagnostic --markdown-out ./diagnostic-full.md full
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

## Disk

```bash
tinfo disk
tinfo disk health
tinfo disk smart
tinfo disk temperature
tinfo disk reliability
```

`disk` focuses on hardware health and reliability signals such as disk model, type, interface, capacity, SMART status, temperature, media errors, wear level, power-on hours, and health interpretation.

## Storage

```bash
tinfo storage
tinfo storage usage
tinfo storage largest
tinfo storage analyze
tinfo storage optimize
```

`storage` focuses on filesystem usage, filesystem type, used/free space, large directories, large files, and cleanup suggestions such as caches, logs, temporary files, and build artifacts.

## Server Mode

```bash
tinfo config server status
tinfo config server enable
tinfo config server disable
```

## Config

```bash
tinfo config
tinfo config setup
tinfo config location
tinfo config location <city>
tinfo config units
tinfo config units metric
tinfo config units imperial
tinfo config output
tinfo config output color
tinfo config output compact
tinfo config output plain
tinfo config widgets
tinfo config widgets show
tinfo config widgets add notes
tinfo config widgets remove network
tinfo config widgets set weather time system notes plugins
tinfo config widgets reset
tinfo config theme
tinfo config theme border sharp
tinfo config theme border rounded
tinfo config theme accent auto
tinfo config theme accent cyan
tinfo config theme unicode on
tinfo config theme unicode off
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

`config widgets` provides quick dashboard widget changes without editing TOML manually.
The interactive `tinfo config` menu also includes a widget picker that shows built-in and trusted plugin widgets together, lets you toggle them with `Enter`, and saves with `q`.

## Search

```bash
tinfo search <query>
```

`search` looks across built-in commands, installed plugins, and the plugin registry cache and returns the most relevant matches first.

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
tinfo configure
tinfo dashboard
tinfo dashboard --freeze
tinfo dashboard --live
tinfo dashboard config
tinfo dashboard reset
tinfo dashboard notes show
tinfo dashboard notes set remember to review plugins
tinfo dashboard notes clear
```

`dashboard` renders the dashboard itself when no dashboard subcommand is provided.
`--freeze` forces snapshot mode.
`--live` forces live mode even when `dashboard.freeze = true` in config.

## Productivity

```bash
tinfo timer
tinfo timer start
tinfo timer start 25m
tinfo timer stop
tinfo stopwatch start
tinfo stopwatch stop
tinfo task
tinfo task add finish README
tinfo task list
tinfo task done 1
tinfo task delete 1
tinfo note add remember to rotate keys
tinfo note list
tinfo history --limit 10
tinfo remind
tinfo remind 15m
tinfo remind 14:30 stand up
tinfo remind 30m stand up
```

These commands store lightweight local state in:

```text
~/.tinfo/data/
```

- `timer` opens a live timer dashboard by default, starts a countdown with `start [duration]`, and uses the configured default duration when omitted
- `stopwatch` manages the stopwatch separately with `start` and `stop`
- `task` opens an interactive menu by default and also supports `add`, `list`, `done`, and `delete`
- the interactive task menu includes:
  - current task list
  - `List all tasks`
  - `Deleted tasks`
  - `Add task`
  - `Delete task`
  - `Exit`
- selecting a task or an item in `List all tasks` toggles its done state in one selection
- deleted tasks move into a recoverable deleted-task area instead of being removed permanently
- deleted tasks can be recovered for 7 days
- when `tinfo` loads the task store on or after the seventh day, expired deleted tasks are removed automatically
- `note` appends quick notes for later review and for the dashboard notes widget
- `history` shows recent shell history lines from the detected history file
- `remind` schedules a local reminder for a delay like `15m`, `1h30m`, or `45s`, or for a clock time like `14:30`; if omitted it uses the configured default duration
- reminders are written to `~/.tinfo/data/reminders.json`, then `tinfo` opens the live dashboard automatically
- after scheduling, `tinfo` prints: `Note: reminders trigger while the dashboard is running.`

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
tinfo plugin search <query>
tinfo plugin browse
tinfo plugin browse --no-open
tinfo plugin init <name>
tinfo plugin keygen [--output-dir <dir>]
tinfo plugin sign <file> [--key <path>]
tinfo plugin inspect
tinfo plugin test
tinfo plugin pack
tinfo plugin pack --from-dist
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

`plugin search` reads the reviewed registry summary index, using the local cache when available, and groups results into installed and registry sections. It marks beta plugins in the output, shows a bounded first page instead of dumping the entire registry, and points users to `tinfo plugin browse` for the full catalog.
`plugin browse` starts a localhost browser UI for plugin discovery and inspection, with paging, popularity sorting, stable/beta filtering, optional icons, and plugin detail views.
`plugin keygen` creates `minisign.key` and `minisign.pub` for plugin release signing.
`plugin sign` signs a plugin artifact and writes a sibling `.minisig` file.
`plugin inspect` shows local plugin metadata and compatibility information for the current project.
`plugin test` validates the current plugin project, runs `--metadata`, and previews local plugin output with simulated host values.
`plugin pack` builds a release binary, bundles it with `plugin.toml`, writes a checksum, signs the bundle, and generates `dist/registry/<plugin-name>.json`.
`plugin pack --from-dist` skips the local build and generates registry JSON from previously downloaded workflow artifacts in `dist/`.
`plugin doctor` checks installed plugins for manifest, registry, path, checksum, and binary issues.
`plugin lint` validates the current plugin project files.
`plugin publish-check` validates plugin project files and release layout before publishing.
`plugin install` downloads the plugin's pinned GitHub release asset and installs it into `~/.terminal-info/plugins/`. If the registry marks automatic install as unsupported for that plugin, Terminal Info prints the fallback install command instead of starting a broken install flow.
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

## AI

```bash
tinfo agent
tinfo chat
```

`agent` opens the built-in AI agent manager.
`chat` opens the built-in AI chat manager.

See also: `docs/chat.md`
