# Terminal Info CLI

[![License](https://img.shields.io/github/license/T-1234567890/terminal-info)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange)](https://www.rust-lang.org/)
[![CLI](https://img.shields.io/badge/type-CLI-blue)]()
[![Platform](https://img.shields.io/badge/platform-macOS%20Linux%20Windows-lightgrey)]()
[![Plugins](https://img.shields.io/badge/plugins-supported-brightgreen)]()

The extensible terminal information CLI. <br>
A fast Rust-powered toolbox for system information,
diagnostics, and developer utilities.

> [!IMPORTANT]
> Commit history was rewritten to remove local file paths and sensitive info.
> 
> [Notice for commit history](https://github.com/T-1234567890/terminal-info?tab=readme-ov-file#%EF%B8%8F-repository-history-notice)

## Demo

### → Experience the demo
https://tinfo.1234567890.dev

Running `tinfo`:

```text
┌──────────────────────────────────┐
│           Terminal Info          │
├──────────────────────────────────┤
│ Location: Shenzhen               │
│ Weather: Clear sky, 20.3°C       │
│ Time: 2026-03-16 xx:xx:xx        │
│ Network: xxx.xxx.x.xx            │
│ CPU: 19.3%                       │
│ Memory: 16.2 GiB / 24.0 GiB used │
└──────────────────────────────────┘
```
The dashboard is just the starting point. <br>
Terminal Info provides many additional commands and plugins
for diagnostics, networking, system information, and developer tools.

Example commands:

```bash
tinfo weather now
tinfo diagnostic network
tinfo ping
tinfo plugin search
```

## v1.0

> terminal-info has reached its first stable release.

The core toolbox, plugin system, and registry are now in place, forming the foundation of a modular terminal environment. <br>
This release focuses on stability and structure. Future updates will expand the plugin ecosystem, improve developer experience, and refine the overall workflow.

This is the beginning of terminal-info as a platform, not just a tool.

## Installation

![Latest Release](https://img.shields.io/github/v/release/T-1234567890/terminal-info)
![github downloads](https://img.shields.io/github/downloads/T-1234567890/terminal-info/total?label=github%20downloads)
[![cargo installs](https://img.shields.io/crates/d/terminal-info.svg?label=cargo%20installs)](https://crates.io/crates/terminal-info)
### Install script 
`Recommended`

Downloads and verifies the release archive with SHA-256 and Minisign before installation:

```bash
curl -fsSL -o install.sh https://github.com/T-1234567890/terminal-info/releases/latest/download/install.sh && bash install.sh
```

The installer sets up the main `tinfo` CLI, including the built-in AI managers.

Interactive setup:

```bash
tinfo install
```

This prompts:

```text
Install AI module? (Y/n)
```

Pressing `Enter` installs the AI module for the smoother default path. Direct module commands are still available:

```bash
tinfo agent
tinfo chat
```

Supported release assets include:

- macOS Intel `x86_64`
- macOS Apple Silicon `arm64`/`aarch64`
- Linux `x86_64`
- Windows `x86_64`

### Build from source
`Contributors and plugin developers`

```bash
git clone https://github.com/T-1234567890/terminal-info
cd terminal-info
cargo build --release
```

### Cargo install
`Rust users`

```bash
cargo install terminal-info
```

## 🧭 Roadmap

The public roadmap tracks what is already shipped in the dashboard, widget, and plugin platform, plus the next planned improvements.

👉 See the full roadmap here: [ROADMAP.md](./docs/roadmap.md)

## 💡 Why terminal-info?

Unlike traditional CLI tools that focus on doing one thing well,  
**terminal-info** is designed as a modular, extensible terminal toolbox.

Instead of switching between multiple utilities, you get a unified system that can adapt to your workflow.

- 🔌 Extensible via plugins  
- 🧩 Composable and customizable workflows  
- ⚡️ Fast, lightweight single-binary core  


## Features

- **A Plugin Platform**
- Dashboard view when running `tinfo`
- Weather, time, ping, network, system, and diagnostic commands
- Separate `disk` and `storage` command groups for hardware health and filesystem usage
- TOML configuration with profiles in `~/.tinfo/config.toml`
- Optional server mode for server and VPS diagnostics
- Dashboard widget ordering in `~/.tinfo/config.toml`
- Built-in and trusted plugin dashboard widgets with a shared enable/disable list in config
- Dashboard live mode, snapshot mode with `--freeze`, config-driven default freeze, and `--live` override
- Lightweight productivity commands for timers, stopwatch, tasks, notes, history, and reminders
- Interactive `tinfo config` / `tinfo configure` menu with sections for dashboard, widgets, tasks, notes, timer, and reminders, including a live widget toggle list
- Shell completions for `bash`, `zsh`, and `fish`
- Output modes for scripting and interactive use, including `--json`
- Plugin discovery, install, update, trust, verification, search, and local browser-based browsing
- A reusable `tinfo-plugin` SDK crate and plugin developer workflow
- Plugin widgets with structured JSON output rendered by the dashboard
- IP-based location detection with provider fallback and local caching

## Dashboard And Productivity

The dashboard is now a first-class feature rather than a simple overview screen. It supports built-in widgets, trusted plugin widgets, widget ordering, notes, and reminder alerts. Widget rendering stays in the core CLI, while plugins provide structured widget payloads.

Productivity tools are integrated into the same local workflow and dashboard runtime:

- `tinfo timer` opens a live timer view
- `tinfo timer start 25m` starts a countdown
- `tinfo stopwatch start` starts a separate stopwatch
- `tinfo task` opens an interactive task menu
- `tinfo note add ...` captures quick notes
- `tinfo history --limit 10` shows recent shell commands
- `tinfo remind 15m take a break` schedules a reminder and opens the live dashboard

Notes:

- reminders trigger while the dashboard is running
- deleted tasks are recoverable for 7 days from the task menu before automatic cleanup
- widget ordering and feature settings live in `~/.tinfo/config.toml`

See:

- [docs/dashboard.md](docs/dashboard.md)
- [docs/widgets.md](docs/widgets.md)
- [docs/commands.md](docs/commands.md)
- [docs/config.md](docs/config.md)

## Why not other tools?

There are already many great terminal tools.  
So why use **terminal-info**?

### System info tools

| Tool | Description | Plugin system | Extensible | Ecosystem |
|-----|-------------|--------------|-----------|-----------|
| neofetch | Classic system info tool with ASCII logos | ❌ | Limited | ❌ |
| fastfetch | Modern and faster alternative to neofetch | ❌ | Limited | ❌ |
| terminal-info | Modular terminal toolbox | ✅ | Yes | Join us |

Tools like **neofetch** or **fastfetch** are excellent for displaying system information in the terminal.  
However, they are primarily **single-purpose tools** focused on presenting system details.

`terminal-info` takes a different approach:

- It is designed as a **platform**, not just a single command.
- Features can be added via **plugins**.
- Users can extend the CLI without modifying the core.

---

### Monitoring tools

| Tool | Description | Plugin ecosystem |
|-----|-------------|----------------|
| btop / htop | Terminal system monitoring | ❌ |
| glances | Multi-metric system monitor | ❌ |
| terminal-info | Extendable toolbox with custom modules | ✅ |

Monitoring tools focus on **resource usage**, but they are still mostly fixed-feature applications.

`terminal-info` allows modules to be added for things like:

- system diagnostics
- hardware info
- network checks
- custom scripts
- developer utilities

---

### CLI platforms

Some CLI tools are successful because they provide an **ecosystem**.

Examples include:

| Tool | Purpose | Plugin model |
|-----|--------|-------------|
| kubectl | Kubernetes CLI | Plugins |
| cargo | Rust package manager | Subcommands |
| brew | Package manager | Taps |

`terminal-info` follows a similar philosophy:

- A **core CLI**
- A **plugin SDK**
- A **plugin registry**
- A plugin-based terminal toolbox

Instead of being a single-purpose utility, the goal is to become a **general-purpose terminal toolbox**.

---

### Philosophy

terminal-info is built around 5 ideas:

- **Extensibility** – anyone can write plugins
- **Modularity** – features live outside the core
- **CLI ecosystem** – tools can grow organically
- **Information** – useful information and operations
- **Fast & Light** – fast, light, but powerful enough

The goal is not to replace existing tools, but to **provide a platform where new terminal tools can live together**.

## Plugin SDK

`tinfo-plugin` is the official Rust SDK for building Terminal Info plugins.

It provides:

- typed config access
- declarative command routing
- structured output helpers
- cache, filesystem, and network helpers
- manifest generation
- in-process plugin testing

Start here:

- [docs/plugin-development.md](docs/plugin-development.md)
- [docs/sdk.md](docs/sdk.md)
- [docs/plugin-spec.md](docs/plugin-spec.md)

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
tinfo latency
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

Lightweight theming is configurable too:

```toml
[theme]
border_style = "sharp"
accent_color = "cyan"
ascii_only = false
```

See [docs/dashboard.md](docs/dashboard.md) and [docs/widgets.md](docs/widgets.md).

## Configuration

Configuration is stored in:

```text
~/.tinfo/config.toml
```

You can configure `tinfo` in three ways:

- `tinfo config` for the interactive menu
- `tinfo config ...` commands for direct scripting
- manual edits to `~/.tinfo/config.toml`

The `[theme]` section controls border style, accent color, and whether boxed output should use ASCII-only characters for limited terminals.

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
tinfo search network
tinfo profile list
tinfo profile use travel
```

See [docs/config.md](docs/config.md) and [docs/widgets.md](docs/widgets.md).

## Server Mode

Server mode is optional. It is designed for servers or VPS environments and is not recommended for normal desktop use.

Enable or disable it with:

```bash
tinfo config server status
tinfo config server enable
tinfo config server disable
```

When enabled, Terminal Info extends the normal toolbox with deeper server-oriented diagnostics. The same commands still work in normal mode, but server mode makes them broader and more server-focused:

```bash
tinfo diagnostic network
tinfo diagnostic system
tinfo diagnostic performance
tinfo diagnostic security
tinfo diagnostic leaks
tinfo diagnostic full
tinfo ping full
tinfo latency full
```

Normal mode still keeps:

```bash
tinfo diagnostic
tinfo diagnostic network
tinfo diagnostic system
tinfo diagnostic performance
tinfo diagnostic full
tinfo ping
tinfo ping full
tinfo latency
tinfo latency full
```

Server-mode-only commands are:

```bash
tinfo diagnostic security
tinfo diagnostic leaks
```

When server mode is enabled, the enhanced human-readable output prints a clear `[Server Mode Enabled]` indicator.

The enhanced server-mode diagnostics now include broader API endpoint checks, DNS resolver visibility, load average, process count, and a wider full-latency probe set.

See [docs/server-mode.md](docs/server-mode.md).

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

See [docs/completions.md](docs/completions.md).

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

See [docs/diagnostic.md](docs/diagnostic.md).

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
tinfo plugin search <query>
tinfo plugin browse
tinfo plugin browse --no-open
tinfo plugin init <name>
tinfo plugin keygen
tinfo plugin sign <file>
tinfo plugin inspect
tinfo plugin test
tinfo plugin pack
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
tinfo plugin list
tinfo plugin remove <name>
```

Registry-managed plugins are installed from the exact reviewed version pinned in the reviewed registry JSON referenced by `plugins/index.json`. Terminal Info does not install the latest plugin release automatically.
Plugins must also be trusted locally before Terminal Info will execute them.

`tinfo plugin search` groups installed plugins and registry plugins separately, ranks matches by name and description relevance, and uses the lightweight registry summary index so large registries stay responsive.
`tinfo plugin browse` starts a local browser view on `127.0.0.1` for optional visual plugin discovery without replacing the CLI workflow. The browser supports pagination, popularity sorting, stable-only or include-beta filtering, detail pages, optional plugin icons, and clear install fallback when one-click install is not supported.

Core installs and self-updates verify the official Terminal Info SHA-256 checksum and Minisign signature before replacing the binary. Plugin installs verify the plugin author's reviewed checksum and Minisign signature before installation.

Developer quick start:

```bash
tinfo plugin init hello
cd tinfo-hello
tinfo plugin inspect
tinfo plugin test
tinfo plugin keygen --output-dir ./keys
tinfo plugin pack
```

`tinfo plugin pack` now also generates `dist/registry/<plugin-name>.json`, and the generated release workflow uploads the same registry JSON as a GitHub Actions artifact for registry PRs. The generated file already uses the standard registry fields such as `repository`, `binary`, `entry`, `platform`, `type`, `requires_network`, optional `assets.icon`, `stability`, `popularity`, and install metadata.

See:

- [docs/plugin-spec.md](docs/plugin-spec.md)
- [docs/plugin-development.md](docs/plugin-development.md)
- [docs/plugin-registry.md](docs/plugin-registry.md)
- [docs/plugin-security.md](docs/plugin-security.md)

Official example plugins are included in:

```text
examples/plugins/
```

Additional stability commands:

```bash
tinfo config doctor
tinfo dashboard config
tinfo dashboard reset
tinfo completion status
tinfo completion uninstall
tinfo self-repair
tinfo reinstall
```

Profile commands:

```bash
tinfo profile list
tinfo profile show home
tinfo profile use home
tinfo profile add home
tinfo profile remove office
```

> ## ⚠️ Repository History Notice
>
> This repository previously contained full development history from early stages.
>
> In March 2026, the commit history was rewritten to remove local file paths and sensitive development artifacts.
>
>- The current codebase and documentation are unaffected  
>- The repository now reflects a clean and production-ready state  
>- Some older commits may still be accessible via GitHub cache or direct commit links, but they are no longer part of the active history  
>
>This is a standard repository maintenance process and does not impact the functionality or integrity of the project.

## 📈 Star History

[![Star History Chart](https://api.star-history.com/svg?repos=T-1234567890/terminal-info&type=date&theme=light)](https://www.star-history.com/#T-1234567890/terminal-info&Date)

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
curl -fsSL -o install.sh https://github.com/T-1234567890/terminal-info/releases/latest/download/install.sh
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
