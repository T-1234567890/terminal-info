# Plugins

Terminal Info plugins are external executables. Terminal Info routes unknown top-level commands to a matching `tinfo-<plugin-name>` binary inside the managed plugin directory.

Example:

```bash
tinfo github issues
```

Terminal Info attempts to run:

```text
tinfo-github issues
```

## Managed Plugin Layout

Managed plugins are installed into:

```text
~/.terminal-info/plugins/<plugin-name>/
```

Example:

```text
~/.terminal-info/plugins/docker/
├── plugin.toml
└── tinfo-docker
```

## Trust Model

Installed plugins are not allowed to execute until the user trusts them explicitly.

Commands:

```bash
tinfo plugin trust <name>
tinfo plugin untrust <name>
tinfo plugin trusted
```

Trusted plugin names are stored in:

```text
~/.terminal-info/trusted_plugins.json
```

## Plugin Commands

```bash
tinfo plugin search
tinfo plugin search <query>
tinfo plugin browse
tinfo plugin browse --no-open
tinfo plugin init <name>
tinfo plugin install <name>
tinfo plugin trust <name>
tinfo plugin untrust <name>
tinfo plugin trusted
tinfo plugin info <name>
tinfo plugin verify
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin remove <name>
tinfo plugin list
```

Registry-managed installs always use the exact version pinned in the Terminal Info plugin registry.
Plugin downloads verify a checksum from the registry and a Minisign signature from the plugin release.

`plugin search` groups results into installed plugins and registry plugins. Registry results come from the cached summary index, can be sorted by popularity or name, label beta plugins explicitly, and when a search term is provided, matches are ranked so exact and prefix name matches appear before looser description hits. The CLI search output is intentionally capped so large registries stay readable and points users to `tinfo plugin browse` for the full catalog.

`plugin browse` starts an optional localhost browser UI for discovery and inspection. It uses the same reviewed registry data as the CLI, supports pagination, can filter beta plugins, can show optional icons or screenshots from registry metadata, and does not bypass the trust model. If a plugin does not support one-click installation, the browser shows the fallback install command instead of a broken install button.

## Related Documentation

- [plugin-spec.md](plugin-spec.md)
- [plugin-development.md](plugin-development.md)
- [plugin-registry.md](plugin-registry.md)
- [plugin-security.md](plugin-security.md)
