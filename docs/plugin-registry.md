# Terminal Info Plugin Registry

Terminal Info uses a lightweight registry model. The main repository stores plugin metadata, while plugin code lives in each plugin author's own repository.

## Registry File

The registry can be represented as a `plugins.json` file:

```json
{
  "plugins": [
    {
      "name": "docker",
      "description": "Docker tools for Terminal Info",
      "repo": "https://github.com/example/tinfo-docker"
    }
  ]
}
```

In the current repository, plugin metadata entries are stored as individual JSON files under `plugins/`.

## User Commands

Terminal Info supports:

```bash
tinfo plugin search
tinfo plugin install <name>
tinfo plugin update <name>
tinfo plugin remove <name>
tinfo plugin list
```

## Search

```bash
tinfo plugin search
```

This reads the Terminal Info registry metadata and lists available plugins.

## Install

```bash
tinfo plugin install docker
```

Terminal Info:

1. reads registry metadata
2. fetches the plugin release from the plugin repository
3. downloads a compatible release asset
4. installs the plugin into:

```text
~/.terminal-info/plugins/docker/
```

## Update

```bash
tinfo plugin update docker
```

This refreshes the installed plugin from the latest compatible release.

Update all installed plugins:

```bash
tinfo plugin upgrade-all
```

## Remove

```bash
tinfo plugin remove docker
```

This deletes the managed plugin directory for that plugin.

## Listing Installed Plugins

```bash
tinfo plugin list
```

This shows the names of plugins installed in the Terminal Info plugin directory.

## Registry Submission Flow

A typical plugin submission process is:

1. Create a public plugin repository
2. Publish release assets
3. Add plugin metadata to the Terminal Info registry
4. Open a pull request

The main Terminal Info repository does not need to host plugin source code to index a plugin.
