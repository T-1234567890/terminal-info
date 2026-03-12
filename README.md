# Terminal Info

`tinfo` is a small Rust CLI for useful terminal information. It combines weather, time, network, system diagnostics, and a lightweight plugin ecosystem built around a decentralized GitHub-based index.

## Features

- Dashboard shown when running `tinfo`
- Built-in weather, network, system, time, and doctor commands
- TOML configuration at `~/.tinfo/config.toml`
- External plugin execution for unknown top-level commands
- GitHub-based plugin installation from a local plugin index
- Plugin validation workflow for index pull requests

## Installation

### Install script

```bash
curl -sSL https://raw.githubusercontent.com/T-1234567890/terminal-info/main/install.sh | bash
```

The install script supports:

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
tinfo doctor
```

## Example Commands

```bash
tinfo
tinfo weather now tokyo
tinfo ping github.com
tinfo time london
tinfo plugin search
tinfo plugin install news
tinfo plugin list
tinfo news tech
```

## Dashboard Feature

Running `tinfo` with no arguments shows a dashboard with:

- location
- weather
- time
- network
- CPU
- memory

See [docs/dashboard.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/dashboard.md).

## Plugin Ecosystem

`tinfo` uses a decentralized plugin model:

- this repository only stores plugin metadata files in `plugins/`
- plugin authors host their own plugin repositories on GitHub
- plugin installs fetch binaries from plugin release assets

Plugin metadata example:

```json
{
  "name": "news",
  "description": "News headlines plugin",
  "repo": "https://github.com/example/tinfo-news",
  "binary": "tinfo-news",
  "version": "latest"
}
```

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

## Plugin Installation

Search available plugins from the index:

```bash
tinfo plugin search
```

Install a plugin from its GitHub release:

```bash
tinfo plugin install news
```

List installed plugins:

```bash
tinfo plugin list
```

Remove a plugin:

```bash
tinfo plugin remove news
```

## Plugin Development

Plugins are standalone executables named:

```text
tinfo-<command>
```

Example:

```text
tinfo-news
```

See [docs/plugin-development.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-development.md).

## Plugin Submission

To submit a plugin to the index:

1. Create a plugin repository
2. Publish a GitHub release
3. Add a metadata JSON file to `plugins/`
4. Open a pull request

See [docs/plugin-index.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-index.md).

## Documentation

- [docs/commands.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/commands.md)
- [docs/plugins.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugins.md)
- [docs/plugin-development.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-development.md)
- [docs/plugin-index.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-index.md)

## License

Apache 2.0
