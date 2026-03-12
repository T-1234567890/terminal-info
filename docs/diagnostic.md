# Diagnostic

`tinfo diagnostic` runs grouped health checks and reports simple pass/fail status lines.

## Commands

```bash
tinfo diagnostic
tinfo diagnostic network
tinfo diagnostic system
tinfo diagnostic plugins
```

## Network Checks

Network diagnostics currently check:

- DNS resolution
- HTTP reachability
- TLS handshake
- latency measurement

Example output:

```text
✔ DNS OK
✔ HTTP reachable
✔ TLS handshake OK
✔ Latency 18.3 ms
```

## System Checks

System diagnostics currently check:

- disk usage
- memory usage
- CPU load

Example output:

```text
✔ Disk usage 52.1%
✔ Memory usage 41.3%
✔ CPU load 12.4%
```

## Plugin Checks

Plugin diagnostics currently check:

- plugin directory integrity
- missing plugin binaries
- plugin version mismatch against index metadata

Example output:

```text
✔ Plugin directory OK
✔ Plugin "news" metadata OK
✖ Plugin "docker" missing binary
```

## Notes

- The command is designed to work on macOS, Linux, and Windows.
- Some network checks may fail in restricted or offline environments.
- Plugin version mismatch is based on installed plugin names compared to plugin index metadata.
