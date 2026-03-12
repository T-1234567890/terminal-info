# Plugin Index

This repository stores the plugin index for `tinfo`.

The index is lightweight and decentralized:

- plugin code is not stored here
- plugin metadata is stored here
- plugin binaries are hosted by plugin authors on GitHub

## Submission Flow

To submit a plugin:

1. Create a plugin repository
2. Publish a GitHub release for the plugin binary
3. Add a metadata JSON file to `plugins/`
4. Open a pull request

## Metadata Schema

Each plugin must be defined by a JSON file:

```json
{
  "name": "news",
  "description": "News headlines plugin",
  "repo": "https://github.com/example/tinfo-news",
  "binary": "tinfo-news",
  "version": "latest"
}
```

Fields:

- `name`
  - CLI command name
- `description`
  - short human-readable description
- `repo`
  - plugin GitHub repository URL
- `binary`
  - executable name
- `version`
  - release version or `latest`

## Validation Rules

Pull requests are validated automatically.

Checks include:

- JSON syntax validity
- required fields present
- no duplicate plugin names
- no names conflicting with reserved built-in commands

Reserved names:

- `weather`
- `ping`
- `network`
- `system`
- `time`
- `doctor`
- `config`
- `plugin`
