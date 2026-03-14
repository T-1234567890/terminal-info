# Architecture

`tinfo` is a small Rust CLI with a deliberately narrow architecture. The current implementation centers on weather features, but the command tree now leaves room for broader terminal information commands over time.

The code is organized around three responsibilities: command parsing, configuration, and weather data retrieval.
It also now includes a small dashboard module and an external plugin runner.

## Module Layout

- `src/main.rs`
  - Defines the `clap` command structure
  - Organizes commands under the `weather` group
  - Dispatches subcommands
  - Owns interactive terminal prompts for `tinfo config`
  - Calls the dashboard when no command is provided
  - Routes unknown top-level commands to the plugin system
  - Formats user-facing terminal output
- `src/config.rs`
  - Resolves the config file path
  - Creates `~/.tinfo` and `config.toml` when missing
  - Loads and saves TOML configuration through `serde`
  - Stores provider selection, optional API key, units, default location, profiles, dashboard settings, cache settings, and server mode
- `src/weather.rs`
  - Builds the shared `reqwest` client
  - Resolves cities via geocoding
  - Fetches current weather and forecast data
  - Selects between Open-Meteo and OpenWeather
  - Handles IP-based location lookup via multiple fallback providers with local caching
- `src/dashboard.rs`
  - Renders the default startup dashboard
  - Displays location, local time, and a short weather summary when available
- `src/builtins.rs`
  - Implements system, network, ping, latency, and diagnostic commands
  - Extends diagnostics when server mode is enabled
- `src/plugin.rs`
  - Resolves managed plugin binaries
  - Executes plugins for unknown top-level commands

## Command Flow

At startup:

1. `clap` parses the command line.
2. The config is loaded from `~/.tinfo/config.json` or a compatible legacy config path.
3. Server mode settings are applied from the existing config when diagnostic and latency commands run.
4. The requested subcommand is executed, the dashboard is shown, or a plugin is launched.

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
- `tinfo`
  - Shows the dashboard
- `tinfo news`
  - Attempts to execute a `tinfo-news` plugin

## Command Hierarchy

The top-level command tree is intentionally simple:

- `tinfo weather`
  - `now`
  - `forecast`
  - `location`
- `tinfo config`
- `tinfo update`

Unknown top-level commands are treated as plugin candidates.

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
