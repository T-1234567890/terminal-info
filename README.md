# Terminal Info CLI

[![Release](https://img.shields.io/github/v/release/T-1234567890/terminal-info)](https://github.com/T-1234567890/terminal-info/releases)
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
- Shell completions for `bash`, `zsh`, and `fish`
- Output modes for scripting and interactive use
- GitHub-based plugin discovery, install, update, and execution

## Installation

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
tinfo plugin install <name>
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin list
tinfo plugin remove <name>
```

Developer quick start:

```bash
tinfo plugin init hello
cd tinfo-hello
cargo build --release
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
