# Terminal Info Plugin Security

Terminal Info plugins are third-party software. Users should treat them like any other executable they install on their machine.

## Security Model

Terminal Info uses a simple executable plugin model:

- plugins run as local user processes
- plugins do not receive automatic root or administrator privileges
- Terminal Info does not elevate plugin permissions

## Managed Plugin Location

Managed plugins are installed under:

```text
~/.terminal-info/plugins/
```

Example:

```text
~/.terminal-info/plugins/docker/tinfo-docker
```

This keeps community plugins in a predictable user-owned location.

## Trust Guidance

Before installing a plugin, users should:

- review the plugin repository
- review the release source and publisher
- understand what the plugin does

Only install Terminal Info plugins from trusted sources.

## No Automatic Root Access

Terminal Info plugins must never require root privileges to run.

If a plugin asks users to run it with `sudo` or elevated privileges, that should be treated as a strong warning sign unless the user fully understands the risk.

## PATH and Local Development

Terminal Info can also resolve plugin executables on `PATH` for development and manual workflows.

For normal use, Terminal Info installs and runs managed plugins from `~/.terminal-info/plugins/`. PATH-based resolution is supported for local development and manual testing, but the managed plugin directory is the stable installation path.
