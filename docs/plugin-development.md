# Terminal Info Plugin Development

Terminal Info plugins are standalone executables. The plugin SDK model is intentionally small so a developer can build and publish a plugin in minutes.

## Quick Start

Generate a new plugin template:

```bash
tinfo plugin init <name>
```

This creates:

```text
tinfo-<name>/
├── .github/
│   └── workflows/
│       └── release.yml
├── Cargo.toml
├── README.md
├── plugin.toml
└── src/
    └── main.rs
```

## What the Template Does

The generated plugin:

- compiles to `tinfo-<name>`
- works with `tinfo <name>`
- includes a `plugin.toml` manifest
- includes a GitHub Actions release workflow

## Local Development

```bash
cd tinfo-<name>
cargo run -- --help
cargo build --release
./target/release/tinfo-docker
```

## Release Workflow

The template includes:

```text
.github/workflows/release.yml
```

The workflow:

- runs when a `v*` tag is pushed
- builds release binaries
- targets:
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
  - `x86_64-unknown-linux-gnu`
- uploads release assets to GitHub Releases

Generated asset names follow this pattern:

```text
tinfo-<name>-x86_64-apple-darwin
tinfo-<name>-aarch64-apple-darwin
tinfo-<name>-x86_64-unknown-linux-gnu
```

## Manifest

Every plugin includes:

```toml
[plugin]
name = "<name>"
version = "x.y.z"
description = "<description>"

[command]
name = "docker"

[compatibility]
terminal_info = ">=x.y.z"
```

## Publishing to the Registry

The recommended flow is:

1. Create a plugin repository such as `tinfo-docker`
2. Push a version tag such as `v0.1.0`
3. Let GitHub Actions publish the binaries
4. Submit or update `plugins/docker.json` in the Terminal Info repository

See [plugin-registry.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-registry.md) and [plugin-spec.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-spec.md).
