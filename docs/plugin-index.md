# Plugin Index

This repository stores the reviewed plugin registry for `tinfo`.

The registry is lightweight and decentralized:

- plugin code is not stored here
- the minimal reviewed summary index is stored here
- each plugin's detailed registry JSON is hosted in the plugin's own repository
- plugin binaries are hosted by plugin authors on GitHub

## Submission Flow

To submit a plugin:

1. Create a plugin repository
2. Publish a GitHub release for the plugin binary
3. Generate registry JSON with `tinfo plugin pack`
4. Publish the generated detailed registry JSON from the plugin repository
5. Add or update the summary entry in `plugins/index.json`
6. Open a pull request

## Metadata Schema

The registry uses two JSON layers:

- `plugins/index.json` lists plugin names and the URL of each detailed registry file
- the detailed registry file itself is hosted in the plugin repository and referenced by URL

Each detailed plugin file must be defined by JSON like this:

```json
{
  "name": "news",
  "version": "0.2.1",
  "description": "News headlines plugin",
  "author": "Example Plugin Author",
  "license": "MIT",
  "repository": "https://github.com/example/tinfo-news",
  "binary": "tinfo-news",
  "entry": "news",
  "platform": ["linux", "macos"],
  "type": "cloud",
  "requires_network": true
}
```

Fields:

- `name`
  - CLI command name
- `version`
  - exact reviewed release version
- `description`
  - longer human-readable description for detail views
- `short_description`
  - required CLI summary
  - single line
  - under 80 characters
  - plain text only
- `author`
  - plugin author or maintainer
- `license`
  - `MIT` or `Apache-2.0`
- `repository`
  - plugin GitHub repository URL
- `binary`
  - executable name
- `entry`
  - command entrypoint routed by `tinfo`
- `platform`
  - supported platforms using `linux`, `macos`, and/or `windows`
- `type`
  - `local` or `cloud`
- `requires_network`
  - whether outbound network access is required

## Validation Rules

Pull requests are validated automatically.

Checks include:

- JSON syntax validity
- required fields present
- `short_description` is one line and under 80 characters
- no duplicate plugin names
- no names conflicting with reserved built-in commands
- repository URL shape
- semver-like version format
- platform values
- supported license values

Reserved names:

- `weather`
- `ping`
- `network`
- `system`
- `time`
- `diagnostic`
- `config`
- `plugin`

## Plugin Overview

`news` fetches current news headlines through a defined remote API and exposes them through the `tinfo news` command.

- Type: cloud-based plugin
- Network requirements: requires outbound network access to its configured news API
- Executable entry: `tinfo-news`
- Security considerations: it should only call documented API endpoints, must not execute remote code, must not require `sudo`, must not install hidden background services, and must not modify system files

## Architecture

The `news` plugin uses a simple client/server model.

- CLI: the local `tinfo-news` executable runs in the user's terminal and formats output for `tinfo`
- Backend: the plugin talks to a remote news API service over HTTPS
- API interaction model: the CLI sends read-only API requests for headlines and renders the returned data locally
