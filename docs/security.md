# Terminal Info Security

## Core Binary Signing

Terminal Info release archives are signed with the official Terminal Info Minisign key.

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

The `tinfo update` command downloads the release archive and its `.minisig` file, then verifies the signature using the public key embedded in the Terminal Info binary from `keys/minisign.pub`.

If the signature does not verify, the update is aborted.

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
5. uploads both the archive and the signature to the GitHub release

The `.sha256` file is optional extra metadata. Signature verification is the required trust check.

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

SHA-256 checksums are optional extra integrity metadata for releases and plugins. They do not replace Minisign signatures.
