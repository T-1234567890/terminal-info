# Terminal Info Plugin Specification

This document defines the Terminal Info plugin architecture.

## Architecture

Terminal Info plugins use a simple executable model.

Example:

```bash
tinfo docker
```

Terminal Info maps that command to:

```text
tinfo-docker
```

## Naming Convention

Plugin executables must use:

```text
tinfo-<plugin-name>
```

Examples:

- `tinfo-docker`
- `tinfo-github`
- `tinfo-speedtest`

## Command Routing

If a top-level command is not built in, Terminal Info searches for a matching plugin.

Search order:

1. `~/.terminal-info/plugins/<plugin-name>/tinfo-<plugin-name>`
2. `PATH`

Example install location:

```text
~/.terminal-info/plugins/docker/tinfo-docker
```

## Plugin Manifest

Each managed plugin should include:

```text
plugin.toml
```

Example:

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

## Security Model

Plugins are third-party executables.

Terminal Info plugin rules:

- plugins run as normal user processes
- plugins must not require root privileges
- plugins should be installed from trusted sources

Registry review improves safety, but it is not a full security audit.

## Installation Model

Terminal Info installs reviewed plugins into:

```text
~/.terminal-info/plugins/
```

Each plugin gets its own directory:

```text
~/.terminal-info/plugins/docker/
~/.terminal-info/plugins/github/
```

This keeps the architecture simple, predictable, and extensible.
