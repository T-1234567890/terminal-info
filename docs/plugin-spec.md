# Terminal Info Plugin Specification

This document defines the Terminal Info plugin SDK and host-plugin contract.

The host protocol is intentionally simple, but plugin authors should target the Rust SDK instead of depending on the raw protocol directly.

## Runtime model

Terminal Info plugins are standalone executables named:

```text
tinfo-<plugin-name>
```

Terminal Info routes:

```bash
tinfo <plugin-name>
```

to:

```text
tinfo-<plugin-name>
```

Managed plugins are installed in:

```text
~/.terminal-info/plugins/<plugin-name>/
```

## Plugin API version

Terminal Info currently defines:

```text
plugin_api = 1
```

Plugins should declare this in `plugin.toml` and return it from `--metadata`.

## Standard metadata command

Plugins should support:

```bash
tinfo-<plugin-name> --metadata
```

This prints JSON like:

```json
{
  "name": "weather",
  "version": "1.0.0",
  "description": "Weather information plugin",
  "author": "Plugin Author",
  "commands": ["weather"],
  "compatibility": {
    "tinfo": ">=0.9.0",
    "plugin_api": 1
  },
  "capabilities": ["network", "config"],
  "api_version": 1
}
```

The SDK generates this automatically from `PluginMetadata`.

The SDK also supports:

```bash
tinfo-<plugin-name> --manifest
tinfo-<plugin-name> --help
```

## Manifest format

Managed plugins should include:

```toml
[plugin]
name = "weather"
version = "1.0.0"
description = "Weather information plugin"
author = "Plugin Author"

[command]
name = "weather"

[compatibility]
terminal_info = ">=0.9.0"
plugin_api = 1

[requirements]
capabilities = ["network", "config"]
```

When using the SDK, `PluginManifest` is the source of truth and can generate this TOML directly.

## Capabilities

Plugins may declare capabilities to describe what they need:

- `network`
- `config`
- `cache`
- `filesystem`

These are metadata declarations for policy and tooling. They are not a sandbox.

## Host context

The `tinfo-plugin` SDK exposes:

- `ctx.system.os()`
- `ctx.system.arch()`
- `ctx.host.version()`
- `ctx.host.plugin_name()`
- `ctx.config.get("key")`
- `ctx.config.string("key")`
- `ctx.config.bool("key")`
- `ctx.config.u64("key")`
- `ctx.config.i64("key")`
- `ctx.config.f64("key")`
- `ctx.config.deserialize::<T>("key")`
- `ctx.cache_dir()`
- `ctx.plugin_dir()`
- `ctx.cache.read_string("key")`
- `ctx.cache.write_string("key", value)`
- `ctx.cache.read_json::<T>("key")`
- `ctx.cache.write_json("key", &value)`
- `ctx.fs.plugin_home()`
- `ctx.fs.plugin_data_dir()`
- `ctx.fs.config_path()`
- `ctx.network.get(url)`

Terminal Info passes the host context to plugins through environment variables when it executes them.

The SDK is responsible for reading those environment variables and presenting them as typed Rust APIs.

## Command routing

Plugins may define commands declaratively through the SDK instead of parsing `std::env::args()` directly.

Examples:

- `PluginCommand::new("status")`
- `PluginCommand::new("inspect")`

The handler receives:

- `Context`
- `CommandInput`

`CommandInput` provides helper methods such as:

- `raw()`
- `positional(index)`
- `flag("--verbose")`
- `option("--city")`

## Output helpers

The SDK provides:

- `plugin.output().section(...)`
- `plugin.output().message(...)`
- `plugin.output().kv(...)`
- `plugin.output().list(...)`
- `plugin.output().warning(...)`
- `plugin.output().error(...)`
- `plugin.output().status(...)`
- `plugin.output().progress(...)`
- `plugin.output().table(...)`
- `plugin.output().json(...)`

It also provides logging helpers:

- `plugin.log().info(...)`
- `plugin.log().warn(...)`
- `plugin.log().error(...)`

Status levels are standardized by the SDK:

- `ok`
- `info`
- `warn`
- `error`
- `running`

## Errors

The SDK provides:

- `PluginError`
- `PluginResult<T>`
- conversions from common Rust error types
- `ResultExt::context(...)`

Plugin handlers should typically return `PluginResult<()>` and use `?`.

## Testing

The SDK provides an in-process test harness so plugin authors can test plugin behavior with `cargo test` instead of shelling out to the real host.

Main testing types:

- `testing::MockHost`
- `testing::TestRunner`

This is the recommended test workflow for plugin authors.

## Compatibility

Plugins should declare:

- plugin version
- minimum Terminal Info version
- plugin API version

Terminal Info developer tooling uses this data in:

- `tinfo plugin inspect`
- `tinfo plugin test`
- `tinfo plugin publish-check`

Compatibility policy:

- `plugin_api = 1` is the current stable plugin API
- plugin authors should target the SDK surface, not undocumented environment details
- removing public SDK APIs requires a new SDK major version
- changing the host-plugin contract requires a coordinated `plugin_api` transition

## Security model

- Plugins are third-party executables
- Plugins run as the normal user
- Plugins must not require root privileges
- Installed plugins must be trusted locally before Terminal Info executes them
- Registry-managed plugins are verified with the plugin author’s Minisign public key
