# Diagnostic

`tinfo diagnostic` runs grouped health checks and reports structured status lines with severity and actionable fixes.

Server mode is optional and intended for servers or VPS environments. Normal mode keeps the standard developer diagnostics. Server mode only extends them with deeper checks.

For a full overview, see [server-mode.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/server-mode.md).

Enable server mode with:

```bash
tinfo config server enable
```

## Commands

```bash
tinfo diagnostic
tinfo diagnostic full
tinfo diagnostic network
tinfo diagnostic performance
tinfo diagnostic security
tinfo diagnostic leaks
tinfo diagnostic system
tinfo diagnostic plugins
```

## Quick vs Full

- `tinfo diagnostic` is the fast default path for everyday checks.
- `tinfo diagnostic network` and `tinfo diagnostic system` run in normal mode and add extra checks automatically when server mode is enabled.
- `tinfo diagnostic performance` and `tinfo diagnostic full` run in normal mode and become deeper when server mode is enabled.
- `tinfo diagnostic security` and `tinfo diagnostic leaks` require server mode.

Full mode adds checks such as:

- weather API connectivity
- plugin registry access
- cache presence/integrity
- local config secret exposure checks
- environment secret exposure checks
- expanded latency probes across more global endpoints

## Network Checks

Quick `tinfo diagnostic` checks:

- DNS resolution
- HTTP reachability
- TLS handshake

Normal `tinfo latency` uses the basic endpoints:

- Cloudflare
- Google
- GitHub

Server mode enhances `tinfo diagnostic network` with:

- DNS resolution
- HTTP reachability
- TLS handshake
- GitHub API reachability
- weather API reachability
- IP geolocation API reachability

Expanded latency testing is available through:

```bash
tinfo ping full
tinfo latency full
```

Full latency mode probes a broader set of endpoints, including CDN providers, DNS providers, and major global services.

When server mode is enabled, the same `full` commands use an even broader server-oriented endpoint set that includes additional cloud providers, CDN networks, and public DNS services.

`tinfo ping full` and `tinfo latency full` work in normal mode and use the expanded endpoint set. When server mode is enabled, the output is clearly labeled as server mode.

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

Server mode enhances system diagnostics with:

- swap usage
- system uptime

Performance diagnostics are available in both modes through:

```bash
tinfo diagnostic performance
```

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

## Security And Leaks

Server mode adds:

```bash
tinfo diagnostic security
tinfo diagnostic leaks
```

These checks run locally and focus on:

- plaintext secrets in config
- exposed environment variables
- common server-side secret handling risks

## Notes

- The command is designed to work on macOS, Linux, and Windows.
- Some network checks may fail in restricted or offline environments.
- Plugin version mismatch is based on installed plugin names compared to plugin index metadata.
- `tinfo config doctor` includes migration, cache, plugin directory, and weather configuration status.
- `--json` returns structured objects for both quick and full diagnostic and latency modes.
- Human-readable server diagnostics print `[Server Mode Enabled]` so the mode is obvious in CLI output.
