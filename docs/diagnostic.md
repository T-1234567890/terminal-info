# Diagnostic

`tinfo diagnostic` runs grouped health checks and reports structured status lines with severity and actionable fixes.

## Commands

```bash
tinfo diagnostic
tinfo diagnostic full
tinfo diagnostic network
tinfo diagnostic system
tinfo diagnostic plugins
```

## Quick vs Full

- `tinfo diagnostic` is the fast default path for everyday checks.
- `tinfo diagnostic full` runs a broader and slower diagnostic pass.

Full mode adds checks such as:

- weather API connectivity
- plugin registry access
- cache presence/integrity

## Network Checks

Network diagnostics currently check:

- DNS resolution
- HTTP reachability
- TLS handshake

Expanded latency testing is available through:

```bash
tinfo ping full
tinfo latency full
```

Full latency mode probes a broader set of endpoints, including CDN providers, DNS providers, and major global services.

Example output:

```text
PASS: DNS resolution (2.1 ms)
FIX: none
FAIL: HTTP reachability (unreachable)
FIX: verify outbound HTTP access
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
- `tinfo config doctor` includes migration, cache, plugin directory, and weather configuration status.
- `--json` returns structured objects for both quick and full diagnostic and latency modes.
