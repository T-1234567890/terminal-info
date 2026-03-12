# Plugins

`tinfo` supports a decentralized plugin ecosystem.

The main repository does not host plugin code. It only hosts a plugin index in `plugins/*.json`. Plugin authors publish and maintain their own GitHub repositories and releases.

## Plugin Discovery

Plugin metadata lives in:

```text
plugins/
```

Each plugin is described by a JSON file.

Example:

```json
{
  "name": "news",
  "description": "News headlines plugin",
  "repo": "https://github.com/example/tinfo-news",
  "binary": "tinfo-news",
  "version": "latest"
}
```

Users can discover available plugins with:

```bash
tinfo plugin search
```

## Plugin Installation

Install a plugin from the index:

```bash
tinfo plugin install news
```

This flow:

1. reads `plugins/news.json`
2. fetches the plugin's GitHub release metadata
3. downloads the release asset
4. installs the binary into `~/.tinfo/plugins/`

Installed plugins can be listed with:

```bash
tinfo plugin list
```

Removed with:

```bash
tinfo plugin remove news
```

## Plugin Commands

The built-in plugin management commands are:

```bash
tinfo plugin search
tinfo plugin install <name>
tinfo plugin remove <name>
tinfo plugin list
```

## Plugin Execution

If `tinfo` receives an unknown top-level command, it attempts to execute a plugin named:

```text
tinfo-<command>
```

Example:

```bash
tinfo news tech
```

Attempts to execute:

```bash
tinfo-news tech
```

Search order:

1. `~/.tinfo/plugins/tinfo-<command>`
2. `PATH`
