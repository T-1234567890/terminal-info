# Plugins

Terminal Info plugins are external executables. The design is intentionally simple: Terminal Info routes unknown top-level commands to a matching `tinfo-<plugin-name>` binary inside the managed plugin directory.

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

## Related Documentation

- [plugin-spec.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-spec.md)
- [plugin-development.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-development.md)
- [plugin-registry.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-registry.md)
- [plugin-security.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-security.md)
