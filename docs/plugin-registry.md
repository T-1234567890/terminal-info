# Terminal Info Plugin Registry

Terminal Info maintains a reviewed plugin registry in the repository `plugins/` directory.

## Registry Layout

Each plugin uses one metadata file:

```text
plugins/<plugin-name>.json
```

Example:

```json
{
  "name": "docker",
  "description": "Docker utilities for Terminal Info",
  "repo": "https://github.com/example/tinfo-docker",
  "version": "0.2.1"
}
```

The registry pins an exact plugin version. Terminal Info does not install the latest release automatically for registry-managed plugins.

## Why Exact Versions

This workflow is intentional:

1. plugin developer publishes a release
2. developer submits a pull request updating the registry version
3. maintainer reviews the change
4. users install or update that reviewed version

This makes plugin installation safer and more predictable.

## Review Process

Registry pull requests should verify:

- plugin name conflicts
- built-in command conflicts
- repository legitimacy
- basic code inspection

This review is not a full security audit.

Maintainers review metadata and obvious risks, but users should still evaluate third-party plugins themselves before installation.

## User Commands

```bash
tinfo plugin search
tinfo plugin install <name>
tinfo plugin update <name>
tinfo plugin upgrade-all
tinfo plugin remove <name>
tinfo plugin list
```

## Install Behavior

When a user runs:

```bash
tinfo plugin install docker
```

Terminal Info:

1. downloads the plugin registry
2. reads `plugins/docker.json`
3. reads the exact version from the registry
4. downloads that exact GitHub Release tag
5. installs the plugin into:

```text
~/.terminal-info/plugins/docker/
```

## Updating Plugins

Updating follows the same reviewed path:

```bash
tinfo plugin update docker
```

The installed version changes only when the registry version changes after review.

Update all installed plugins:

```bash
tinfo plugin upgrade-all
```
