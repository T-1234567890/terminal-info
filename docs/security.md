# Terminal Info Security

## Core Binary Signing

Terminal Info release archives are published with both an official SHA-256 checksum and an official Minisign signature.

Core release assets are published as:

- `tinfo-x86_64-unknown-linux-gnu.tar.gz`
- `tinfo-x86_64-unknown-linux-gnu.tar.gz.minisig`
- `tinfo-x86_64-apple-darwin.tar.gz`
- `tinfo-x86_64-apple-darwin.tar.gz.minisig`
- `tinfo-aarch64-apple-darwin.tar.gz`
- `tinfo-aarch64-apple-darwin.tar.gz.minisig`
- `tinfo-x86_64-pc-windows-msvc.zip`
- `tinfo-x86_64-pc-windows-msvc.zip.minisig`

The GitHub Actions release workflow builds each target on a native runner:

- Linux on `ubuntu-latest`
- macOS Intel on `macos-latest`
- macOS Apple Silicon on `macos-14`
- Windows on `windows-latest`

The install script and `tinfo update` command both download the release archive, its `.sha256` file, and its `.minisig` file. They verify the SHA-256 checksum first, then verify the Minisign signature using the public key embedded in the installer or the running Terminal Info binary from `keys/minisign.pub`.

If either verification step fails, installation or update is aborted.

## Release Signing Workflow

Core releases use the `MINISIGN_SECRET_KEY` GitHub Actions secret.

That secret must contain the full raw contents of an unencrypted Minisign secret key file, including the comment line. Generate the key without a password so GitHub Actions does not need interactive input:

```bash
minisign -G -W
```

For each release artifact, the workflow:

1. builds the target-specific binary
2. packages the archive with the exact name `tinfo-<target>.tar.gz` or `tinfo-<target>.zip`
3. writes the secret key to `minisign.key`
4. runs `minisign -S` to produce `archive.minisig`
5. writes a SHA-256 checksum file for the archive
6. uploads the archive, checksum, and signature to the GitHub release

Both checksum verification and signature verification are required before Terminal Info installs or updates a core release.

## Signing Key Rotation

To rotate the Terminal Info core signing key:

1. generate a new Minisign keypair
2. use `minisign -G -W` so the new secret key is not password protected
3. update the private key stored in the GitHub Actions secret `MINISIGN_SECRET_KEY`
4. replace the public key in `keys/minisign.pub`
5. cut a new signed Terminal Info release
6. ship a Terminal Info update that contains the new embedded public key before depending on the rotated key for future updater-only releases

Because `tinfo update` verifies signatures with the public key embedded in the currently installed binary, key rotation must be staged carefully. Existing clients must receive a release that embeds the new public key before they can verify future releases signed only by the new key.

## Plugin Signing

Plugins are third-party executables and are not signed with the Terminal Info core signing key.

Each plugin author:

- generates their own Minisign keypair
- signs their own plugin release artifacts
- submits the public key in the reviewed registry entry

Terminal Info verifies plugin downloads against the plugin-specific `pubkey` stored in `plugins/<plugin-name>.json`.

## Trust Model

- Terminal Info core updates trust the official embedded Terminal Info key.
- Plugin installs trust the plugin author's public key from the reviewed registry.
- Local execution trust is separate from signature verification. Installed plugins must also be explicitly trusted with `tinfo plugin trust <name>` before they can run.

## Checksums

SHA-256 checksums are part of the required verification flow for official core installs and updates. They complement Minisign signatures and do not replace them.
