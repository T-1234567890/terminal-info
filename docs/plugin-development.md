# Terminal Info Plugin Development

Terminal Info plugins are regular executables. A plugin developer does not need a special runtime, daemon, or embedded SDK.

## Quick Start

Generate a new plugin template:

```bash
tinfo plugin init hello
```

This creates a new repository-style directory:

```text
tinfo-hello/
├── Cargo.toml
├── README.md
├── plugin.toml
└── src/
    └── main.rs
```

Run the template locally:

```bash
cd tinfo-hello
cargo run -- --help
cargo build --release
./target/release/tinfo-hello
```

## Naming Rules

Terminal Info routes:

```bash
tinfo hello
```

to:

```text
tinfo-hello
```

Plugin binaries must use this format:

```text
tinfo-<plugin-name>
```

Examples:

- `tinfo-docker`
- `tinfo-github`
- `tinfo-speedtest`

## Minimal Plugin Behavior

A basic plugin can simply read CLI arguments and print output:

```rust
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("Hello from Terminal Info: {}", args.join(" "));
}
```

When installed, Terminal Info runs the plugin as a normal process.

## Manifest File

Every plugin should include a `plugin.toml` file:

```toml
[plugin]
name = "docker"
version = "0.1.0"
description = "Docker utilities for Terminal Info"

[command]
name = "docker"

[compatibility]
terminal_info = ">=0.3.0"
```

The manifest documents:

- the public plugin name
- the plugin version
- the Terminal Info compatibility requirement

## Publishing

A typical publishing flow is:

1. Create a GitHub repository such as `tinfo-docker`
2. Build release binaries
3. Publish GitHub Releases
4. Submit the plugin to the Terminal Info registry

See [plugin-registry.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-registry.md) and [plugin-spec.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-spec.md).
