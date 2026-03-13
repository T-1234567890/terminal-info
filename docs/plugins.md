# Plugins

Terminal Info plugins are external executables. The design is intentionally simple: Terminal Info routes unknown top-level commands to a matching `tinfo-<plugin-name>` binary.

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

## Plugin Commands

```bash
tinfo plugin search
tinfo plugin init <name>
tinfo plugin install <name>
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin remove <name>
tinfo plugin list
```

Registry-managed installs always use the exact version pinned in the Terminal Info plugin registry.

## Related Documentation

- [plugin-spec.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-spec.md)
- [plugin-development.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-development.md)
- [plugin-registry.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-registry.md)
- [plugin-security.md](/Users/2111832868qq.com/PycharmProjects/Learning/Terminal%20Weather/docs/plugin-security.md)
