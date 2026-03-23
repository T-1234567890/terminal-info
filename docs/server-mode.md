# Server Mode

Server mode is an optional extension for Terminal Info.

It is designed for:

- servers
- VPS environments
- long-running developer hosts

It is not recommended for regular desktop computers.

## What Server Mode Does

Server mode does not replace the normal CLI behavior.

Normal mode still keeps the standard toolbox commands such as:

```bash
tinfo diagnostic
tinfo diagnostic network
tinfo diagnostic system
tinfo diagnostic performance
tinfo diagnostic full
tinfo ping
tinfo ping full
tinfo latency
tinfo latency full
```

When server mode is enabled, these commands become more server-oriented:

- broader endpoint coverage for full ping and latency tests
- extra API reachability checks in network diagnostics
- extra DNS resolver, uptime, swap, load-average, and process-count checks
- deeper full diagnostics

Server mode also unlocks server-only checks:

```bash
tinfo diagnostic security
tinfo diagnostic leaks
```

## Enable And Disable

Server mode is disabled by default.

Use:

```bash
tinfo config server status
tinfo config server enable
tinfo config server disable
```

When enabling server mode, Terminal Info shows a warning and asks for confirmation:

```text
Server mode is intended for server or VPS environments and may not be suitable for regular desktop computers.
```

## Config File

Server mode is stored in the existing config file:

```toml
server_mode = true
```

The config file location is:

```text
~/.tinfo/config.toml
```

## Interactive Configuration

You can also manage server mode through the interactive config menu:

```bash
tinfo config
```

Menu path:

- `Server Mode`
- `Enable`
- `Disable`
- `Status`

## Enhanced Commands

When server mode is enabled, these commands are enhanced:

- `tinfo diagnostic`
- `tinfo diagnostic network`
- `tinfo diagnostic system`
- `tinfo diagnostic performance`
- `tinfo diagnostic full`
- `tinfo ping`
- `tinfo ping full`
- `tinfo latency`
- `tinfo latency full`

The biggest differences are:

- `tinfo diagnostic network`
  - adds common API endpoint connectivity checks
  - adds DNS resolver visibility when available
- `tinfo diagnostic system`
  - adds uptime, swap usage, load average, and process count
- `tinfo diagnostic performance`
  - focuses on CPU, memory, disk, swap, uptime, process pressure, load average, and running process count
- `tinfo diagnostic full`
  - combines the broader network, system, config, cache, registry, load, DNS, and latency checks
- `tinfo ping full` and `tinfo latency full`
  - use a broader server-oriented endpoint set including additional DNS providers, CDNs, and cloud vendors

## Server-Only Commands

These commands require server mode:

```bash
tinfo diagnostic security
tinfo diagnostic leaks
```

If server mode is disabled, Terminal Info shows:

```text
This feature requires server mode.
Enable it with: tinfo config server enable
```

## Output Indicator

When server mode is active and an enhanced diagnostic path is used, human-readable output includes:

```text
[Server Mode Enabled]
```

This makes it clear that Terminal Info is using the server-oriented diagnostic behavior.

## Privacy And Local Checks

Server security and leak checks are designed to run locally.

Examples:

- config secret detection
- environment variable secret detection
- local config path checks

These checks should not require sending secret data to external services.

## Related Docs

- [config.md](config.md)
- [diagnostic.md](diagnostic.md)
- [commands.md](commands.md)
