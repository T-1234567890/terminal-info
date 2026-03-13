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
  "name": "<plugin-name>",
  "description": "<plugin description>",
  "repo": "https://github.com/example/tinfo-<plugin-name>",
  "version": "0.2.1",
  "pubkey": "RW...",
  "checksums": {
    "x86_64-unknown-linux-gnu": "<sha256>",
    "x86_64-pc-windows-msvc": "<sha256>"
  }
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

Registry metadata should include:

- exact reviewed version
- plugin author Minisign public key in `pubkey`
- per-target SHA-256 checksums

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
tinfo plugin install <plugin-name>
```

Terminal Info:

1. downloads the plugin registry
2. reads `plugins/<plugin-name>.json`
3. reads the exact version from the registry
4. downloads that exact GitHub Release tag
5. verifies the Minisign signature using the registry `pubkey`
6. verifies the checksum when a registry checksum is present
7. installs the plugin into:

```text
~/.terminal-info/plugins/<plugin-name>/
```

## Updating Plugins

Updating follows the same reviewed path:

```bash
tinfo plugin update <plugin-name>
```

The installed version changes only when the registry version changes after review.

Update all installed plugins:

```bash
tinfo plugin upgrade-all
```

## Signing Policy

- Terminal Info core releases are signed with the official Terminal Info Minisign key.
- Plugins must not reuse the Terminal Info core signing key.
- Each plugin author signs their own release artifacts.
- The reviewed registry stores the plugin author's public key in `pubkey`.
- `tinfo plugin install` and `tinfo plugin update` verify plugin artifacts against that plugin-specific key before installation.
