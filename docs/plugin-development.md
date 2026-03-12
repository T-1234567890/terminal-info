# Plugin Development

This document explains how to build a plugin for `tinfo`.

## Naming Rule

A plugin binary must be named:

```text
tinfo-<command>
```

Example:

```text
tinfo-weather
```

If a user runs:

```bash
tinfo weather
```

`tinfo` will try to execute:

```bash
tinfo-weather
```

## Minimal Plugin Behavior

A plugin can be any executable file.

Example shell plugin:

```bash
#!/usr/bin/env bash
echo "Plugin running"
echo "Args: $*"
```

Saved as:

```text
tinfo-news
```

## Example Structure

Suggested plugin repository structure:

```text
Cargo.toml
src/main.rs
README.md
```

The compiled release binary should be named to match the plugin command.

Example:

- command: `news`
- binary: `tinfo-news`

## Release Publishing

Publish binaries in GitHub Releases so `tinfo plugin install <name>` can download them.

The installer expects either:

- a direct binary asset named like `tinfo-news`
- or an archive asset containing the plugin binary

Target-specific archive naming that includes the binary name and target triple works best.
