# Terminal Info Plugin Specification

This document defines the Terminal Info executable plugin model.

## Overview

Terminal Info plugins are external binaries executed by Terminal Info.

Example:

```bash
tinfo docker
```

Terminal Info maps that command to:

```text
tinfo-docker
```

## Naming Convention

Plugin binary:

```text
tinfo-<plugin-name>
```

Examples:

- `tinfo-docker`
- `tinfo-github`
- `tinfo-speedtest`

Plugin names should use lowercase letters, numbers, and `-`.

## Command Routing

If a top-level Terminal Info command is not built in, Terminal Info attempts to resolve a plugin.

Search order:

1. managed plugin install directory
2. `PATH`

Managed plugin directory layout:

```text
~/.terminal-info/plugins/<plugin-name>/
```

Example:

```text
~/.terminal-info/plugins/docker/
├── plugin.toml
└── tinfo-docker
```

## Manifest Format

Each plugin directory should contain a `plugin.toml` manifest.

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

### Required Sections

`[plugin]`
- `name`
- `version`
- `description`

`[command]`
- `name`

`[compatibility]`
- `terminal_info`

## Installation Layout

Managed plugins are stored per plugin:

```text
~/.terminal-info/plugins/docker/
~/.terminal-info/plugins/github/
```

Each directory may contain:

- `plugin.toml`
- the plugin executable
- optional plugin assets

## Compatibility

The `terminal_info` field declares which Terminal Info versions the plugin expects.

Example:

```toml
[compatibility]
terminal_info = ">=0.3.0"
```

Terminal Info keeps the plugin contract intentionally small:

- plugins are executables
- arguments are passed through directly
- output is controlled by the plugin

This keeps the architecture simple and easy to extend.
