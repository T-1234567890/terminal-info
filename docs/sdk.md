# Terminal Info SDK Guide

`tinfo-plugin` is the official Rust SDK for writing Terminal Info plugins.

The SDK is designed so plugin authors work with stable Rust APIs instead of the raw host protocol.

## What the SDK Provides

- typed configuration access
- declarative command routing
- structured output helpers
- capability-aware cache, filesystem, and network helpers
- SDK-owned manifest generation
- in-process testing without launching the real host

## Basic Plugin

```rust
use serde::Serialize;
use tinfo_plugin::{
    Capability, CommandInput, Plugin, PluginCommand, PluginResult, StatusLevel, Table,
};

#[derive(Serialize)]
struct InspectView {
    plugin: &'static str,
    host_version: String,
    location: Option<String>,
}

fn status(ctx: tinfo_plugin::Context, args: CommandInput) -> PluginResult<()> {
    let location = args
        .option("--city")
        .map(str::to_string)
        .or(ctx.config.string("location")?)
        .unwrap_or_else(|| "auto".to_string());

    ctx.cache.write_string("last-city", &location)?;

    ctx.output().section("Status");
    ctx.output().status(StatusLevel::Ok, "plugin is ready");
    ctx.output().kv("Location", &location);
    ctx.output().table(
        Table::new(["Field", "Value"])
            .row(["OS", ctx.system.os()])
            .row(["Arch", ctx.system.arch()]),
    );
    Ok(())
}

fn inspect(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    ctx.output().json(&InspectView {
        plugin: "weather",
        host_version: ctx.host.version(),
        location: ctx.config.string("location")?,
    })?;
    Ok(())
}

fn main() {
    Plugin::new("weather")
        .description("Weather information plugin")
        .author("Plugin Author")
        .capability(Capability::Config)
        .capability(Capability::Cache)
        .command(
            PluginCommand::new("status")
                .description("Show plugin status")
                .handler(status),
        )
        .command(
            PluginCommand::new("inspect")
                .description("Emit a JSON inspection view")
                .handler(inspect),
        )
        .default_handler(status)
        .dispatch();
}
```

## Typed Config API

Use typed accessors instead of parsing raw JSON by hand.

Available helpers:

- `ctx.config.get("path")`
- `ctx.config.string("path")`
- `ctx.config.bool("path")`
- `ctx.config.u64("path")`
- `ctx.config.i64("path")`
- `ctx.config.f64("path")`
- `ctx.config.deserialize::<T>("path")`

Examples:

```rust
let city = ctx.config.string("location")?;
let enabled = ctx.config.bool("features.summary")?.unwrap_or(false);
let ttl = ctx.config.u64("cache.ttl_secs")?.unwrap_or(60);
```

```rust
#[derive(serde::Deserialize)]
struct Credentials {
    token: String,
}

let creds = ctx.config.deserialize::<Credentials>("plugin.auth")?;
```

## Errors

Use `PluginResult<T>` and `?`.

The SDK converts common errors automatically:

- `std::io::Error`
- `serde_json::Error`
- `toml::de::Error`
- `toml::ser::Error`
- `reqwest::Error`
- integer and float parse errors

Add extra context with:

```rust
use tinfo_plugin::ResultExt;

let body = std::fs::read_to_string("data.txt").context("read plugin input")?;
```

## Command Routing

Plugins can define commands declaratively.

```rust
Plugin::new("docker")
    .command(
        PluginCommand::new("status")
            .description("Show container status")
            .handler(status),
    )
    .command(
        PluginCommand::new("inspect")
            .description("Emit machine-readable state")
            .handler(inspect),
    )
    .default_handler(status)
    .dispatch();
```

`CommandInput` helpers:

- `raw()`
- `is_empty()`
- `len()`
- `positional(index)`
- `flag("--verbose")`
- `option("--city")`

Built-in SDK flags:

- `--metadata`
- `--manifest`
- `--help`

## Output

Use the shared output primitives instead of inventing ad-hoc formatting.

Available helpers:

- `section`
- `message`
- `kv`
- `list`
- `warning`
- `error`
- `status`
- `progress`
- `table`
- `json`

Status levels:

- `StatusLevel::Ok`
- `StatusLevel::Info`
- `StatusLevel::Warn`
- `StatusLevel::Error`
- `StatusLevel::Running`

## Capability Helpers

The SDK exposes helpers around common plugin capabilities.

### Cache

```rust
ctx.cache.write_string("last-run", "ok")?;
let previous = ctx.cache.read_string("last-run")?;
ctx.cache.write_json("snapshot.json", &snapshot)?;
let cached = ctx.cache.read_json::<Snapshot>("snapshot.json")?;
```

### Filesystem

```rust
let home = ctx.fs.plugin_home();
let data_dir = ctx.fs.plugin_data_dir()?;
let config_path = ctx.fs.config_path();
```

### Network

```rust
#[derive(serde::Deserialize)]
struct ApiResponse {
    status: String,
}

let body = ctx.network.get("https://example.com/health").send_text()?;
let response = ctx
    .network
    .get("https://example.com/api")
    .query("city", "tokyo")
    .send_json::<ApiResponse>()?;
```

## Widgets

Plugins can expose dashboard widgets without rendering terminal UI directly.

Use the widget builder on `Plugin` and return a structured widget payload:

```rust
use tinfo_plugin::{Plugin, PluginResult, Widget, WidgetBody, WidgetMode};

fn dashboard_widget(ctx: tinfo_plugin::Context, mode: WidgetMode) -> PluginResult<Widget> {
    let city = ctx.config.string("location")?.unwrap_or_else(|| "auto".to_string());
    let compact = WidgetBody::text(format!("city={city}"));
    let full = WidgetBody::table(
        ["Field", "Value"],
        [["City", city.as_str()], ["Host", ctx.host.version()]],
    );

    let widget = Widget::new("Weather", full)
        .compact(compact)
        .refresh_interval_secs(30);

    match mode {
        WidgetMode::Compact | WidgetMode::Full => Ok(widget),
    }
}

Plugin::new("weather")
    .description("Weather information plugin")
    .widget(dashboard_widget)
    .dispatch();
```

The SDK handles:

- `--widget`
- `--widget --compact`

See [widgets.md](widgets.md) for the full widget JSON schema and dashboard-side behavior.

Widget JSON schema:

```json
{
  "title": "Weather",
  "refresh_interval_secs": 30,
  "full": {
    "type": "table",
    "headers": ["Field", "Value"],
    "rows": [["City", "tokyo"], ["Host", "1.0.7"]]
  },
  "compact": {
    "type": "text",
    "content": "city=tokyo"
  }
}
```

## Manifest Model

The SDK owns the plugin manifest model.

You can generate or validate manifests with Rust types instead of manually keeping code and TOML in sync.

Main types:

- `PluginMetadata`
- `PluginManifest`
- `CompatibilityPolicy`
- `Capability`

Built-in support:

- `Plugin::manifest()`
- `cargo run -- --manifest`

## Testing

Use the in-process test harness for normal Rust tests.

```rust
use serde_json::json;
use tinfo_plugin::testing::{MockHost, TestRunner};

#[test]
fn status_reads_location() {
    let plugin = build_plugin();
    let run = TestRunner::new(plugin)
        .host(MockHost::default().config_json(json!({ "location": "tokyo" })))
        .args(["status"])
        .run()
        .unwrap();

    assert!(run.stdout.contains("tokyo"));
}
```

## Compatibility Policy

`plugin_api = 1` is the current stable plugin API.

The SDK is the compatibility layer between plugins and the host. Plugin authors should target the SDK, not the raw environment variables or manifest details directly.

Stable surface:

- metadata emitted by `--metadata`
- manifest shape generated by the SDK
- typed `Context` APIs
- command routing and output helpers

Breaking changes require either:

- a new SDK major version, or
- a new `plugin_api` level with coordinated host support

## Module Layout

The SDK is organized into these modules:

- `command`
- `config`
- `context`
- `error`
- `manifest`
- `output`
- `testing`
