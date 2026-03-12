# Terminal Weather CLI

`tw` is a terminal weather CLI written in Rust. It provides current weather and short forecasts with a small command surface, plain terminal output, and a lightweight local configuration file.

By default, `tw` uses Open-Meteo, which does not require an API key. Users can optionally configure an OpenWeather API key and provider selection in their local config.

## Features

- Current weather in the terminal
- Short forecast output
- Automatic location detection by IP for `tw now`
- Manual location configuration
- Interactive configuration menu
- Config stored in `~/.tw/config.json`
- Optional API key support for OpenWeather
- Cross-platform CLI with standard stdin/stdout behavior

## Installation

### Build from source

Requirements:

- Rust toolchain
- Cargo

Clone the repository and build:

```bash
cargo build --release
```

Run directly from the project:

```bash
cargo run -- now tokyo
```

The compiled binary will be available at:

```bash
target/release/tw
```

You can also install it into Cargo's bin directory:

```bash
cargo install --path .
```

## Quick Start

Show current weather for a city:

```bash
tw now tokyo
```

Show current weather using your saved default location:

```bash
tw now
```

Show a forecast:

```bash
tw forecast london
```

Set a default location:

```bash
tw location tokyo
```

Open the interactive config menu:

```bash
tw config
```

## Commands

Core commands:

```bash
tw now
tw now <city>

tw forecast
tw forecast <city>

tw location
tw location <city>

tw config
tw config api
tw config units metric
tw config units imperial
```

See [docs/commands.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/commands.md) for the full command reference.

## Example Output

ASCII example:

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

`tw` stores user settings in:

```text
~/.tw/config.json
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

- Interactive menu via `tw config`
- Direct commands such as `tw location tokyo` or `tw config units imperial`

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
tw config api set openweather YOUR_API_KEY
```

`tw now` also supports IP-based location detection using:

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

See the Apache 2.0 license text in your repository license file if one is added separately.
