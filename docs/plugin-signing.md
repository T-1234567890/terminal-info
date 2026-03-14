# Terminal Info Plugin Signing

## Overview

Terminal Info plugins are signed by plugin authors, not by the Terminal Info project.

This keeps the plugin ecosystem decentralized while allowing Terminal Info to verify downloaded plugin artifacts before installation.

## Developer Workflow

1. Generate a Minisign keypair:

```bash
tinfo plugin keygen --output-dir ./keys
```

This writes `minisign.key` and `minisign.pub` into the selected directory.

You can also generate keys with Minisign directly:

```bash
minisign -G
```

2. Build the plugin release assets.

3. Sign each asset:

```bash
tinfo plugin sign dist/tinfo-<plugin-name>-x86_64-apple-darwin --key ./keys/minisign.key
```

Or use Minisign directly:

```bash
minisign -S -s minisign.key -m dist/tinfo-<plugin-name>-x86_64-apple-darwin
```

4. Upload both files to the plugin GitHub release:

- `tinfo-<plugin-name>-x86_64-apple-darwin`
- `tinfo-<plugin-name>-x86_64-apple-darwin.minisig`

5. Submit a registry pull request that includes:

- plugin name
- repository URL
- reviewed version
- `pubkey`
- checksums

## Registry Example

```json
{
  "name": "<plugin-name>",
  "description": "<plugin description>",
  "repo": "https://github.com/example/tinfo-<plugin-name>",
  "version": "0.1.0",
  "pubkey": "RWRgzvl/IRChlCdww8KtvuohEfnRA++x8Ro1hql1KOvVAVItAXEsC0jN",
  "checksums": {
    "x86_64-unknown-linux-gnu": "<sha256>",
    "x86_64-apple-darwin": "<sha256>",
    "aarch64-apple-darwin": "<sha256>"
  }
}
```

## Install Verification

When a user runs `tinfo plugin install <plugin-name>`, Terminal Info:

1. reads the plugin metadata from the reviewed registry
2. downloads the exact version pinned in the registry
3. downloads the matching `.minisig` file
4. verifies the signature using `pubkey`
5. verifies the checksum when one is provided
6. installs the plugin only if verification succeeds

If signature verification fails, the install is aborted.
