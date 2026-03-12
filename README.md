# Terminal Info CLI

`tinfo` is a terminal-first Rust CLI for weather and related local terminal information workflows. The current implementation focuses on weather lookups with a small command surface, plain output, and lightweight local configuration.

By default, `tinfo` uses Open-Meteo, which does not require an API key. Users can optionally configure an OpenWeather API key and provider selection in their local config.

## Features

- Current weather in the terminal
- Short forecast output
- Automatic location detection by IP for `tinfo weather now`
- Manual location configuration
- Interactive configuration menu
- Config stored in `~/.tinfo/config.json`
- Optional API key support for OpenWeather
- Cross-platform CLI with standard stdin/stdout behavior

## Installation

### Option 1 - Install script

Install the latest release binary:

```bash
curl -sSL https://raw.githubusercontent.com/T-1234567890/terminal-info/main/install.sh | bash
```

### Option 2 - Build from source

Requirements:

- Rust toolchain
- Cargo

Clone and build:

```bash
git clone https://github.com/T-1234567890/terminal-info
cd terminal-info
cargo build --release
```

The compiled binary will be available at:

```bash
target/release/tinfo
```

### Option 3 - Cargo install

```bash
cargo install tinfo
```

## Updating

Update to the latest GitHub release:

```bash
tinfo update
```

## Quick Start

Show current weather for a city:

```bash
tinfo weather now tokyo
```

Show current weather using your saved default location:

```bash
tinfo weather now
```

Show a forecast:

```bash
tinfo weather forecast london
```

Set a default location:

```bash
tinfo weather location tokyo
```

Open the interactive config menu:

```bash
tinfo config
```

## Commands

Core commands:

```bash
tinfo weather now
tinfo weather now <city>

tinfo weather forecast
tinfo weather forecast <city>

tinfo weather location
tinfo weather location <city>

tinfo config
tinfo config api
tinfo config units metric
tinfo config units imperial

tinfo update
```

See [docs/commands.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/commands.md) for the full command reference.

## Example Output

ASCII example:

```text
╭────────────────────╮
│  ☀ Tokyo Weather   │
├────────────────────┤
│ Temp: 27°C         │
│ Wind: 3 m/s        │
╰────────────────────╯
```

Current implementation output is intentionally simple and terminal-friendly, for example:

```text
+-----------------------------+
| Tokyo, Tokyo, Japan Weather |
+-----------------------------+
  Partly cloudy
  Temperature: 8.4°C
  Wind: 1.6 m/s
  Humidity: 47%
```

## Configuration

`tinfo` stores user settings in:

```text
~/.tinfo/config.json
```

The config directory and file are created automatically when needed.

Example config:

```json
{
  "location": "Tokyo",
  "units": "metric"
}
```

When an API provider and key are configured, the file may also include:

```json
{
  "provider": "openweather",
  "api_key": "your-api-key",
  "units": "metric",
  "location": "Tokyo"
}
```

Configuration can be managed in two ways:

- Interactive menu via `tinfo config`
- Direct commands such as `tinfo weather location tokyo` or `tinfo config units imperial`

See [docs/config.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/config.md) for details.

## API Providers

### Open-Meteo

- Default provider
- No API key required
- Used automatically when no custom provider is configured

### OpenWeather

- Optional provider
- Requires an API key
- Can be configured with:

```bash
tinfo config api set openweather YOUR_API_KEY
```

`tinfo weather now` also supports IP-based location detection using:

- `https://ipapi.co/json/`

This is used as a fallback when no city argument is provided and no default location is stored.

See [docs/api.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/api.md) for provider details.

## Architecture

The CLI is intentionally small and split into a few modules:

- `src/main.rs` defines the `clap` command tree and user interaction
- `src/config.rs` handles config loading and saving
- `src/weather.rs` handles provider selection, HTTP requests, and response parsing

See [docs/architecture.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/architecture.md) for a more detailed overview.

## Acknowledgements

The project idea was inspired by [terminal-weather](https://github.com/Vincent4486/terminal-weather) by Vincent4486. This project acknowledges that inspiration respectfully and does not imply code reuse.

## License

This project is licensed under the Apache License 2.0.
