# Terminal Info Plugin Registry

Terminal Info keeps a reviewed plugin registry in the repository `plugins/` directory.

## Registry entry format

Each plugin uses one file:

```text
plugins/<plugin-name>.json
```

Example:

```json
{
  "name": "weather",
  "description": "Weather information plugin",
  "repo": "https://github.com/example/tinfo-weather",
  "version": "1.0.0",
  "author": "Plugin Author",
  "plugin_api": 1,
  "capabilities": ["network", "config"],
  "pubkey": "RW...",
  "checksums": {
    "x86_64-unknown-linux-gnu": "<sha256>",
    "x86_64-apple-darwin": "<sha256>",
    "aarch64-apple-darwin": "<sha256>",
    "x86_64-pc-windows-msvc": "<sha256>"
  }
}
```

## Why exact versions are pinned

Terminal Info does not install the latest plugin release automatically.

The registry review flow is:

1. plugin author publishes a release
2. plugin author updates the registry entry
3. maintainer reviews the new version, metadata, and signing key
4. users install or update the reviewed version

## Installation flow

When a user runs:

```bash
tinfo plugin install <plugin-name>
```

Terminal Info:

1. fetches the plugin registry
2. reads the exact reviewed version
3. downloads that exact GitHub release tag
4. downloads the matching `.minisig` file
5. verifies the bundle with the registry `pubkey`
6. verifies the checksum for the active target
7. installs the plugin into `~/.terminal-info/plugins/<plugin-name>/`

## Developer workflow

Before submitting a registry pull request:

```bash
tinfo plugin inspect
tinfo plugin test
tinfo plugin pack
tinfo plugin publish-check
```

Then:

1. publish the signed release assets
2. update `plugins/<plugin-name>.json`
3. submit a pull request

## Review expectations

Registry review should check:

- plugin name conflicts
- built-in command conflicts
- repository legitimacy
- manifest and metadata shape
- plugin API version
- signing key presence
- release asset and checksum shape

This review improves safety, but it is not a full security audit.
