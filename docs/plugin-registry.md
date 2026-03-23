# Terminal Info Plugin Registry

Terminal Info maintains a reviewed plugin registry in the repository `plugins/` directory.

The registry uses two layers:

1. `plugins/index.json`
2. one detailed JSON file per plugin

This keeps the first fetch small and avoids GitHub API rate limits.

## Layer 1: index.json

`index.json` is intentionally minimal. It is used only to list plugin names and find the URL of each detailed registry file.

It contains only:

- `version`
- `plugins[].name`
- `plugins[].registry`

Example:

```json
{
  "version": 1,
  "plugins": [
    {
      "name": "news",
      "registry": "https://raw.githubusercontent.com/T-1234567890/terminal-info/main/plugins/news.json"
    }
  ]
}
```

## Layer 2: per-plugin registry JSON

Each plugin has one detailed registry file:

```text
plugins/<plugin-name>.json
```

Example:

```json
{
  "name": "weather",
  "version": "1.0.0",
  "description": "Weather information plugin",
  "author": "Plugin Author",
  "license": "MIT",
  "short_description": "Current weather and forecast",
  "repository": "https://github.com/example/tinfo-weather",
  "homepage": "https://example.com/tinfo-weather",
  "binary": "tinfo-weather",
  "entry": "weather",
  "platform": ["linux", "macos", "windows"],
  "type": "cloud",
  "requires_network": true,
  "icon": "https://example.com/assets/icon.png",
  "screenshots": [
    "https://example.com/assets/preview-1.png"
  ],
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

Optional discovery fields:

- `short_description`: compact summary used in search and browser cards
- `homepage`: preferred user-facing project URL
- `icon`: square logo or icon URL
- `screenshots`: optional preview asset URLs for the local browser UI

Required plugin metadata fields:

- `name`: plugin command name
- `version`: exact reviewed release version
- `description`: full human-readable description
- `author`: plugin author or maintainer
- `license`: `MIT` or `Apache-2.0`
- `repository`: GitHub repository URL used to derive release asset URLs
- `binary`: executable name inside the release archive, usually `tinfo-<plugin-name>`
- `entry`: command entrypoint routed by `tinfo`
- `platform`: supported platform list using `linux`, `macos`, and/or `windows`
- `type`: `local` or `cloud`
- `requires_network`: whether the plugin requires outbound network access
- `plugin_api`: host compatibility version
- `capabilities`: declared SDK capabilities
- `pubkey`: Minisign public key used for verification
- `checksums`: per-target SHA-256 checksums

## Fetch Flow

Terminal Info:

1. fetches `index.json` from `raw.githubusercontent.com`
2. caches the index locally for a short time
3. fetches a per-plugin registry JSON only when a plugin needs to be searched in detail, inspected, installed, updated, or rendered in the browser UI
4. caches each per-plugin registry JSON separately

This keeps the list request small and avoids repeatedly downloading every plugin definition.

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

1. fetches `index.json`
2. fetches the target plugin registry JSON
3. reads the exact reviewed version
4. downloads that exact GitHub release asset URL directly
5. downloads the matching `.minisig` file
6. verifies the bundle with the registry `pubkey`
7. verifies the checksum for the active target
8. installs the plugin into `~/.terminal-info/plugins/<plugin-name>/`

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
2. generate or download the registry JSON produced by `tinfo plugin pack`
3. update `plugins/<plugin-name>.json`
4. submit a pull request

## Review expectations

Registry review should check:

- plugin name conflicts
- built-in command conflicts
- repository legitimacy
- manifest and metadata shape
- plugin API version
- signing key presence
- release asset and checksum shape

Registry review verifies packaging, metadata, and signing information. It does not replace source review or operator trust decisions.
