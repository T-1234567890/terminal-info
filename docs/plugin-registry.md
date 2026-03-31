# Terminal Info Plugin Registry

Terminal Info maintains a reviewed plugin registry in the repository `plugins/` directory.

The registry uses two layers:

1. `plugins/index.json`
2. one detailed JSON file per plugin

This keeps the first fetch small, supports pagination in the browser, and avoids fetching every plugin definition just to render a list.

## Layer 1: index.json

`index.json` is the summary layer for plugin discovery. It should stay lightweight even when the registry grows large.

Required fields:

- `version`
- `plugins[].name`
- `plugins[].registry`

Recommended summary fields for browser and search:

- `plugins[].version`
- `plugins[].description`
- `plugins[].short_description`
- `plugins[].author`
- `plugins[].repository`
- `plugins[].homepage`
- `plugins[].assets.icon`
- `plugins[].stability`
- `plugins[].popularity`
- `plugins[].install`

The registry now supports:

- stable and beta plugin classification
- summary-first discovery for large registries
- browser paging and sorting
- install fallback commands when one-click install is not supported

Example:

```json
{
  "version": 1,
  "plugins": [
    {
      "name": "news",
      "registry": "https://raw.githubusercontent.com/T-1234567890/terminal-info/main/plugins/news.json",
      "version": "0.2.1",
      "description": "Fetches current news headlines from reviewed remote sources.",
      "short_description": "Current news headlines in the terminal",
      "author": "Example Plugin Author",
      "repository": "https://github.com/example/tinfo-news",
      "homepage": "https://github.com/example/tinfo-news",
      "assets": {
        "icon": "assets/icon.png"
      },
      "stability": "stable",
      "popularity": 128,
      "install": {
        "supported": true
      }
    }
  ]
}
```

Summary entries should contain enough metadata for `tinfo plugin search` and `tinfo plugin browse` to render a useful catalog without downloading every per-plugin file up front.

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
  "assets": {
    "icon": "assets/icon.png"
  },
  "stability": "stable",
  "popularity": 128,
  "install": {
    "supported": true,
    "command": "tinfo plugin install weather"
  },
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
- `assets.icon`: optional icon under `assets/`, usually `assets/icon.png`
- `screenshots`: optional preview asset URLs for the local browser UI
- `stability`: `stable` or `beta`, defaults to `stable`
- `popularity`: numeric sort key used by the browser and search
- `install`: optional install metadata:
  - `supported`: whether one-click install is supported on the current registry path
  - `command`: fallback CLI command shown when one-click install is disabled

## Stability

Plugins may include:

```json
"stability": "stable"
```

or:

```json
"stability": "beta"
```

Rules:

- the default is `stable`
- beta plugins should be used only when the registry explicitly marks them as `beta`
- `tinfo plugin browse` shows a visible beta badge
- `tinfo plugin search` shows `beta` inline in the result flags
- the browser defaults to stable-only results and can include beta when requested

If `stability` is omitted, Terminal Info treats the plugin as stable.

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

## Asset Rules

Plugin assets are optional but standardized:

- store icons under `assets/`
- preferred file name: `assets/icon.png`
- optional SVG: `assets/icon.svg`
- asset paths must be relative
- browser UI falls back to a generated default icon when an icon is missing

`assets.icon` must:

- stay under `assets/`
- not contain `..`
- not be an absolute path
- end in `.png` or `.svg`

## Fetch Flow

Terminal Info:

1. fetches `index.json`
2. caches the index locally for a short time
3. renders search and browser list views from the cached index summary data
4. fetches a per-plugin registry JSON only when a plugin needs to be inspected, installed, updated, verified, or opened in the detail view
4. caches each per-plugin registry JSON separately

This keeps the list request small and avoids repeatedly downloading every plugin definition.

## Pagination And Sorting

The local browser and JSON search view support paging over registry plugins:

- default limit: `50`
- page parameter: `page=<n>`
- sort modes:
  - `popularity`
  - `name`
- beta filtering:
  - stable only by default
  - include beta with `beta=1`

CLI search uses the same summary metadata but is intentionally shorter:

- it shows a bounded first page of registry results instead of dumping the full registry
- it still includes installed plugins separately
- it recommends `tinfo plugin browse` when users need the full catalog

This keeps `tinfo plugin search` readable even when the registry grows into hundreds or thousands of entries.

## Search And Browse Behavior

`tinfo plugin search`:

- reads only the summary registry layer
- ranks matches by name and description relevance
- shows stability in the result flags
- caps registry output by default
- points users to `tinfo plugin browse` for the full catalog

`tinfo plugin browse`:

- reads the same summary metadata for the list view
- supports pagination and sorting
- marks beta plugins visually
- shows icons when `assets.icon` exists
- loads the full per-plugin JSON only for detail and install flows

The browser currently applies paging in the frontend view using the cached summary index. If the registry grows further, the same summary structure can be split into multiple index files later without changing the per-plugin metadata schema.

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

If `install.supported` is `false`, the browser disables one-click install and shows the fallback command instead. The CLI also refuses misleading automatic installation and prints the configured install command.

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
- summary metadata consistency between `index.json` and per-plugin JSON
- optional asset paths and naming
- stability classification
- install fallback metadata
- plugin API version
- signing key presence
- release asset and checksum shape

Registry review verifies packaging, metadata, and signing information. It does not replace source review or operator trust decisions.
