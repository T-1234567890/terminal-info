# Terminal Info Plugin Development

Terminal Info plugins are standalone executables. The plugin SDK model is intentionally small so a developer can build and publish a plugin in minutes.

## Quick Start

Generate a new plugin template:

```bash
tinfo plugin init
```

This creates:

```text
tinfo-<plugin-name>/
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

- compiles to `tinfo-<plugin-name>`
- works with `tinfo <plugin-name>`
- includes a `plugin.toml` manifest
- includes a GitHub Actions release workflow

## Local Development

```bash
cd tinfo-<plugin-name>
cargo run -- --help
cargo build --release
./target/release/tinfo-<plugin-name>
```

## Release Workflow

The template includes:

```text
.github/workflows/release.yml
```

The workflow:

- runs when a release tag like `0.1.0` is pushed
- builds release binaries
- targets:
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
  - `x86_64-unknown-linux-gnu`
  - `x86_64-pc-windows-msvc`
- uploads release assets to GitHub Releases
- can sign release assets with Minisign when repository secrets are configured

Generated asset names follow this pattern:

```text
tinfo-<plugin-name>-x86_64-apple-darwin
tinfo-<plugin-name>-aarch64-apple-darwin
tinfo-<plugin-name>-x86_64-unknown-linux-gnu
tinfo-<plugin-name>-x86_64-pc-windows-msvc.exe
```

For registry installation, publish matching signature files:

```text
tinfo-<plugin-name>-x86_64-apple-darwin.minisig
```

## Manifest

Every plugin includes:

```toml
[plugin]
name = "<plugin-name>"
version = "0.1.0"
description = "<plugin description>"

[command]
name = "<plugin-name>"

[compatibility]
terminal_info = ">=0.2.3"
```

## Publishing to the Registry

The recommended flow is:

1. Create a plugin repository such as `tinfo-<plugin-name>`
2. Push a version tag such as `0.1.0`
3. Let GitHub Actions publish the binaries
4. Add release checksums and a Minisign public key to `plugins/<plugin-name>.json`
5. Submit or update `plugins/<plugin-name>.json` in the Terminal Info repository

See [plugin-registry.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-registry.md) and [plugin-spec.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-spec.md).
