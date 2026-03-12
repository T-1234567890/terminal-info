# Terminal Info

`tinfo` is a lightweight Rust CLI for terminal-friendly system, network, weather, and plugin-driven information.

## Features

- Dashboard view when running `tinfo`
- Weather, time, ping, network, system, and diagnostic commands
- TOML configuration with profiles in `~/.tinfo/config.toml`
- Shell completions for `bash`, `zsh`, and `fish`
- Output modes for scripting and interactive use
- GitHub-based plugin discovery, install, update, and execution

## Installation

### Install script

```bash
curl -sSL https://raw.githubusercontent.com/T-1234567890/terminal-info/main/install.sh | bash
```

Supported release assets include:

- macOS Intel (`x86_64`)
- macOS Apple Silicon (`arm64` / `aarch64`)
- Linux `x86_64`

### Build from source

```bash
git clone https://github.com/T-1234567890/terminal-info
cd terminal-info
cargo build --release
```

### Cargo install

```bash
cargo install tinfo
```

## Basic Usage

```bash
tinfo
tinfo weather now
tinfo ping
tinfo network
tinfo system
tinfo time
tinfo diagnostic
```

## Example Commands

```bash
tinfo --compact
tinfo weather now tokyo
tinfo weather forecast
tinfo diagnostic plugins
tinfo config
tinfo config units imperial
tinfo profile list
tinfo profile use home
tinfo completion zsh
tinfo plugin search
tinfo plugin install news
tinfo news tech
```

## Dashboard

Running `tinfo` with no arguments shows a simple dashboard with:

- configured location or `unknown`
- current weather when a usable location is available
- local time
- basic network, CPU, and memory summary

See [docs/dashboard.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/dashboard.md).

## Configuration

Configuration is stored in:

```text
~/.tinfo/config.toml
```

You can configure `tinfo` in three ways:

- `tinfo config` for the interactive menu
- `tinfo config ...` commands for direct scripting
- manual edits to `~/.tinfo/config.toml`

Profiles let you switch quickly between named environments:

```toml
[profile.home]
location = "shenzhen"

[profile.work]
location = "tokyo"

[profile.travel]
location = "auto"
```

Commands:

```bash
tinfo profile list
tinfo profile use travel
```

See [docs/config.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/config.md).

## Output Modes

Global output flags:

- `--plain` for minimal script-friendly output
- `--compact` for shorter terminal output
- `--color` for the default interactive formatting

Examples:

```bash
tinfo --plain diagnostic
tinfo --compact weather now
tinfo --color
```

## Shell Completions

Generate completions with:

```bash
tinfo completion bash
tinfo completion zsh
tinfo completion fish
```

See [docs/completions.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/completions.md).

## Diagnostic Command

`tinfo diagnostic` groups health checks for:

- network
- system
- plugins

Examples:

```bash
tinfo diagnostic
tinfo diagnostic network
tinfo diagnostic system
tinfo diagnostic plugins
```

See [docs/diagnostic.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/diagnostic.md).

## Plugin System

Unknown top-level commands are treated as plugin candidates.

Example:

```bash
tinfo news tech
```

This attempts to run:

```bash
tinfo-news tech
```

Search order:

1. `~/.tinfo/plugins/tinfo-<command>`
2. `PATH`

Plugin management commands:

```bash
tinfo plugin search
tinfo plugin install <name>
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin list
tinfo plugin remove <name>
```

See [docs/plugins.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugins.md), [docs/plugin-development.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-development.md), and [docs/plugin-index.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-index.md).

## License

Apache 2.0
