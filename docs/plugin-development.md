# Write a Terminal Info Plugin in 5 Minutes

Terminal Info plugins are standalone executables, and the standard developer workflow uses the `tinfo-plugin` SDK crate and the built-in plugin tooling.

Conceptually, the plugin system works a lot like a lightweight `brew tap`. A plugin author ships a separate executable such as `tinfo-weather`, publishes signed release assets on GitHub, and then adds a reviewed registry entry that tells Terminal Info where to find that exact version. When a user runs `tinfo plugin install weather`, Terminal Info looks up the pinned registry metadata, downloads that release asset, verifies its checksum and Minisign signature, installs it under `~/.terminal-info/plugins/weather/`, and then routes `tinfo weather` to that plugin executable. The host and plugin communicate through a small stable contract, but plugin authors are expected to target the Rust SDK instead of dealing with the raw protocol directly.

## 1. Scaffold a plugin

```bash
tinfo plugin init weather
cd tinfo-weather
```

The generated project includes:

```text
tinfo-weather/
├── .github/workflows/release.yml
├── Cargo.toml
├── README.md
├── plugin.toml
├── src/main.rs
└── tests/smoke.rs
```

The template already:

- depends on the `tinfo-plugin` SDK crate
- implements the standard `--metadata` command automatically through the SDK
- implements the standard `--manifest` command automatically through the SDK
- declares `plugin_api = 1`
- includes capabilities in `plugin.toml`
- shows typed config access
- shows declarative command routing
- includes a smoke test using the SDK test harness
- builds signed cross-platform release bundles
- includes a release workflow that generates registry JSON automatically

## 2. Run locally

```bash
cargo run
```

Inspect the plugin API metadata:

```bash
cargo run -- --metadata
cargo run -- --manifest
```

The generated plugin uses the SDK's declarative command model:

- `status`
- `inspect`

The SDK also handles these flags automatically:

- `--metadata`
- `--manifest`
- `--help`

See [sdk.md](sdk.md) for the full API guide.

## 3. Test with host simulation

From inside the plugin project:

```bash
cargo test
tinfo plugin inspect
tinfo plugin test
```

These commands:

- run the in-process SDK smoke test
- validate `plugin.toml`
- run the local plugin metadata command
- simulate Terminal Info host environment variables
- preview the plugin output

## 4. Build a signed release bundle

Generate a plugin signing key once:

```bash
tinfo plugin keygen --output-dir ./keys
```

Build and pack the plugin:

```bash
tinfo plugin pack
```

The generated `plugin.toml` includes a `[release]` section. Set the repository URL before publishing and keep `keys/minisign.pub` in the repo so the workflow can generate registry JSON automatically.

```toml
[release]
repository = "https://github.com/OWNER/tinfo-weather"
pubkey_path = "keys/minisign.pub"
short_description = "Current weather and forecast"
stability = "stable"
popularity = 128

[release.assets]
icon = "assets/icon.png"

[release.install]
supported = true
```

Local packing creates artifacts such as:

```text
dist/weather-v0.1.0.tar.gz
dist/weather-v0.1.0.tar.gz.sha256
dist/weather-v0.1.0.tar.gz.minisig
dist/registry/weather.json
```

`dist/registry/<plugin-name>.json` is the ready-to-submit registry JSON for the current build output.
That JSON already follows the reviewed registry schema, including `repository`, `binary`, `entry`, `platform`, `type`, `requires_network`, optional `assets.icon`, `stability`, `popularity`, and install metadata, so plugin authors should submit the generated file instead of hand-editing registry metadata.

If you want to sign a file manually:

```bash
tinfo plugin sign dist/weather-v0.1.0.tar.gz --key ./keys/minisign.key
```

## 5. Validate before publishing

```bash
tinfo plugin lint
tinfo plugin publish-check
```

Use these before pushing a release tag.

## 6. Publish

1. Push a release tag such as `0.1.0`
2. Let GitHub Actions build the release assets
3. Let GitHub Actions run `tinfo plugin pack --from-dist` and upload the generated registry JSON artifact
4. Confirm the bundles, checksums, `.minisig` files, and registry JSON artifact exist
5. Add or update `plugins/<plugin-name>.json` in the Terminal Info registry using the generated JSON
6. Submit a pull request for review

## SDK Example

```rust
use serde::Serialize;
use tinfo_plugin::{
    Capability, CommandInput, Plugin, PluginCommand, PluginResult, StatusLevel,
};

#[derive(Serialize)]
struct InspectView {
    host_version: String,
    location: Option<String>,
}

fn status(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    let location = ctx.config.string("location")?.unwrap_or_else(|| "auto".to_string());
    ctx.output().status(StatusLevel::Ok, format!("location: {location}"));
    Ok(())
}

fn inspect(ctx: tinfo_plugin::Context, _args: CommandInput) -> PluginResult<()> {
    ctx.output().json(&InspectView {
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
        .command(
            PluginCommand::new("status")
                .description("Show plugin status")
                .handler(status),
        )
        .command(
            PluginCommand::new("inspect")
                .description("Emit machine-readable state")
                .handler(inspect),
        )
        .default_handler(status)
        .dispatch();
}
```

## SDK Surface

The SDK provides:

- `Plugin` for metadata, routing, and dispatch
- `PluginCommand` and `CommandInput` for command handling
- typed config access through `ctx.config`
- structured output through `ctx.output()`
- logging through `ctx.log()`
- cache, filesystem, and network helpers through `ctx.cache`, `ctx.fs`, and `ctx.network`
- `PluginManifest` and compatibility types for manifest generation
- `testing::TestRunner` and `testing::MockHost` for plugin tests

## Official examples

See the official example plugins in:

```text
examples/plugins/
```

Included examples:

- `daily-brief`
- `git-summary`
- `docker-status`
- `remote-health`
