# Architecture

`tinfo` is a small Rust CLI with a deliberately narrow architecture. The current implementation centers on weather features, but the command tree now leaves room for broader terminal information commands over time.

The code is organized around three responsibilities: command parsing, configuration, and weather data retrieval.

## Module Layout

- `src/main.rs`
  - Defines the `clap` command structure
  - Organizes commands under the `weather` group
  - Dispatches subcommands
  - Owns interactive terminal prompts for `tinfo config`
  - Formats user-facing terminal output
- `src/config.rs`
  - Resolves the config file path
  - Creates `~/.tinfo` and `config.json` when missing
  - Loads and saves JSON configuration through `serde`
  - Stores provider selection, optional API key, units, and default location
- `src/weather.rs`
  - Builds the shared `reqwest` client
  - Resolves cities via geocoding
  - Fetches current weather and forecast data
  - Selects between Open-Meteo and OpenWeather
  - Handles IP-based location lookup via `ipapi.co`

## Command Flow

At startup:

1. `clap` parses the command line.
2. The config is loaded from `~/.tinfo/config.json` or a compatible legacy config path.
3. The requested subcommand is executed.

Examples:

- `tinfo weather now tokyo`
  - Uses the explicit `tokyo` argument
  - Fetches current weather from the active provider
- `tinfo weather now`
  - Uses the saved default location if present
  - Otherwise attempts IP-based city detection
  - If detection fails, prints a clear fallback message
- `tinfo config`
  - Enters the interactive configuration menu
  - Saves changes immediately after each action

## Command Hierarchy

The top-level command tree is intentionally simple:

- `tinfo weather`
  - `now`
  - `forecast`
  - `location`
- `tinfo config`
- `tinfo update`

This structure isolates weather-specific behavior under a dedicated command group while keeping global configuration and update workflows at the top level.

## Provider Selection

Provider selection is configuration-driven:

- If no provider is configured, `tinfo` uses Open-Meteo
- If `provider = "openweather"` and `api_key` is present, `tinfo` uses OpenWeather

This keeps the default experience keyless while allowing users to opt into an API-key-based provider.

## Output Design

The CLI output is intentionally plain:

- Simple boxed headers for weather and forecast sections
- Readable labels for temperature, wind, and humidity
- Minimal terminal dependencies

The project avoids complex terminal UI frameworks in favor of predictable cross-platform behavior.

## Error Handling

The project follows a direct CLI error model:

- Network failures return short human-readable errors
- Missing config files are created automatically
- Invalid or unavailable location detection fails safely
- User-facing failures are printed to stderr where appropriate

## Design Goals

The current architecture optimizes for:

- Small code size
- Clear command behavior
- Easy local configuration
- Minimal external setup
- Straightforward future extension
