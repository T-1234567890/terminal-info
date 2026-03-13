# Terminal Info Plugin Specification

This document defines the Terminal Info plugin architecture.

## Architecture

Terminal Info plugins use a simple executable model.

Example:

```bash
tinfo <plugin-name>
```

Terminal Info maps that command to:

```text
tinfo-<plugin-name>
```

## Naming Convention

Plugin executables must use:

```text
tinfo-<plugin-name>
```

Examples:

- `tinfo-<plugin-name>`
- `tinfo-<another-plugin>`
- `tinfo-<tool-plugin>`

## Command Routing

If a top-level command is not built in, Terminal Info searches for a matching plugin.

Search order:

1. `~/.terminal-info/plugins/<plugin-name>/tinfo-<plugin-name>`

Example install location:

```text
~/.terminal-info/plugins/<plugin-name>/tinfo-<plugin-name>
```

## Plugin Manifest

Each managed plugin should include:

```text
plugin.toml
```

Example:

```toml
[plugin]
name = "<plugin-name>"
version = "0.1.0"
description = "<plugin description>"

[command]
name = "<plugin-name>"

[compatibility]
terminal_info = ">=0.3.0"
```

## Security Model

Plugins are third-party executables.

Terminal Info plugin rules:

- plugins run as normal user processes
- plugins must not require root privileges
- plugins should be installed from trusted sources
- plugins must be trusted locally before Terminal Info will execute them
- plugin downloads are verified with the plugin author's Minisign signature from the reviewed registry
- checksums may be used as an additional integrity check when present

Registry review improves safety, but it is not a full security audit.

## Installation Model

Terminal Info installs reviewed plugins into:

```text
~/.terminal-info/plugins/
```

Each plugin gets its own directory:

```text
~/.terminal-info/plugins/<plugin-name>/
~/.terminal-info/plugins/<another-plugin>/
```

This keeps the architecture simple, predictable, and extensible.
