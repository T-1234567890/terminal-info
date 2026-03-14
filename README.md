# Terminal Info CLI

[![License](https://img.shields.io/github/license/T-1234567890/terminal-info)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange)](https://www.rust-lang.org/)
[![CLI](https://img.shields.io/badge/type-CLI-blue)]()
[![Platform](https://img.shields.io/badge/platform-macOS%20Linux%20Windows-lightgrey)]()
[![Plugins](https://img.shields.io/badge/plugins-supported-brightgreen)]()

`tinfo` is a lightweight Rust CLI for terminal-friendly system, network, weather, and plugin-driven information.

## Features

- Dashboard view when running `tinfo`
- Weather, time, ping, network, system, and diagnostic commands
- TOML configuration with profiles in `~/.tinfo/config.toml`
- Dashboard widget ordering in `~/.tinfo/config.toml`
- Shell completions for `bash`, `zsh`, and `fish`
- Output modes for scripting and interactive use, including `--json`
- GitHub-based plugin discovery, install, update, trust, verification, and execution

## Installation

![Latest Release](https://img.shields.io/github/v/release/T-1234567890/terminal-info)
[![Release](https://github.com/T-1234567890/terminal-info/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/T-1234567890/terminal-info/actions/workflows/release.yml)
![Downloads](https://img.shields.io/github/downloads/T-1234567890/terminal-info/total)
### Install script

You can inspect the install script before running it:

```bash
curl -sSL https://raw.githubusercontent.com/T-1234567890/terminal-info/main/install.sh | bash
```

Supported release assets include:

- macOS Intel (`x86_64`)
- macOS Apple Silicon (`arm64` / `aarch64`)
- Linux `x86_64`
- Windows `x86_64`

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
tinfo weather
tinfo weather now
tinfo weather hourly
tinfo weather alerts
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
tinfo weather home
tinfo weather forecast
tinfo weather hourly
tinfo weather alerts
tinfo diagnostic plugins
tinfo config doctor
tinfo config
tinfo config units imperial
tinfo profile list
tinfo profile use home
tinfo completion install
tinfo completion zsh
tinfo plugin search
tinfo plugin install news
tinfo plugin trust news
tinfo plugin info news
tinfo plugin verify
tinfo news tech
```

## Dashboard

Running `tinfo` with no arguments shows a simple dashboard with:

- configured location or `unknown`
- current weather when a usable location is available
- short actionable hints when weather cannot be resolved
- local time
- basic network, CPU, and memory summary
- trusted plugin widgets when plugins expose `--widget` JSON

Widget order is configurable:

```toml
[dashboard]
widgets = ["weather", "time", "network", "system", "plugins"]
```

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

Location aliases are also supported:

```toml
[locations]
home = "Shenzhen"
work = "Hong Kong"
```

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
- `--json` for machine-readable output on supported commands

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
tinfo completion install
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

## Plugin Ecosystem

Terminal Info supports community plugins using a simple executable plugin model.

Example:

```bash
tinfo docker
```

Terminal Info resolves that to:

```text
tinfo-docker
```

Managed plugins are installed into:

```text
~/.terminal-info/plugins/<plugin-name>/
```

Example:

```text
~/.terminal-info/plugins/docker/
├── plugin.toml
└── tinfo-docker
```

Plugin management commands:

```bash
tinfo plugin search
tinfo plugin init <name>
tinfo plugin keygen
tinfo plugin sign <file>
tinfo plugin install <name>
tinfo plugin trust <name>
tinfo plugin untrust <name>
tinfo plugin trusted
tinfo plugin info <name>
tinfo plugin verify
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin list
tinfo plugin remove <name>
```

Registry-managed plugins are installed from the exact reviewed version pinned in `plugins/<name>.json`. Terminal Info does not install the latest plugin release automatically.
Plugins must also be trusted locally before Terminal Info will execute them.

Core self-updates verify the official Terminal Info Minisign signature, and plugin installs verify the plugin author's Minisign signature from the reviewed registry entry. SHA-256 checksums remain an extra integrity check when present.

Developer quick start:

```bash
tinfo plugin init hello
cd tinfo-hello
tinfo plugin keygen --output-dir ./keys
cargo build --release
tinfo plugin sign dist/tinfo-hello-x86_64-apple-darwin --key ./keys/minisign.key
```

See:

- [docs/plugin-spec.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-spec.md)
- [docs/plugin-development.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-development.md)
- [docs/plugin-registry.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-registry.md)
- [docs/plugin-security.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-security.md)

## Disclaimer

While Terminal Info aims to be safe and transparent, users should understand that:

- Terminal Info executes commands locally on your machine.
- Terminal Info may perform network requests for certain features (for example weather data or IP-based location detection).
- Terminal Info supports **third-party plugins**, which are external executables developed by independent contributors.

### Third-party plugins

Third-party plugins are not developed by the Terminal Info project.
Installing a plugin may execute external code on your system.

Plugins listed in the official plugin registry will go through a basic review process.
This review does not guarantee that plugins are safe or free of malicious behavior.

Only install plugins from sources you trust and review plugin repositories before installing them.

### Installation scripts

If you install Terminal Info using the provided installation script, you may review the script before running it:

```
curl -sSL https://raw.githubusercontent.com/T-1234567890/terminal-info/main/install.sh
```

You may also download binaries directly from the GitHub Releases page.

### Privacy

Terminal Info does **not collect personal data or identifiers**.

Network requests are only used for specific features such as:

- weather information
- IP-based location detection
- plugin registry queries
- Future features

By using Terminal Info, you acknowledge that you are responsible for reviewing the software and plugins you install.

## License

This project is licensed under the **Apache 2.0** License.
