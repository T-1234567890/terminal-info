#!/usr/bin/env python3

import json
import sys
from pathlib import Path

RESERVED = {
    "weather",
    "ping",
    "network",
    "system",
    "time",
    "diagnostic",
    "config",
    "profile",
    "completion",
    "plugin",
    "update",
}

REQUIRED = {"name", "description", "repo", "version"}


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    sys.exit(1)


def main() -> None:
    plugin_dir = Path("plugins")
    if not plugin_dir.exists():
        fail("plugins/ directory does not exist.")

    files = sorted(plugin_dir.glob("*.json"))
    if not files:
        fail("No plugin metadata files found in plugins/.")

    names = {}
    for path in files:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            fail(f"{path}: invalid JSON: {exc}")

        missing = REQUIRED.difference(data.keys())
        if missing:
            fail(f"{path}: missing required fields: {', '.join(sorted(missing))}")

        name = data["name"]
        if not isinstance(name, str) or not name.strip():
            fail(f"{path}: plugin name must be a non-empty string")

        if name in RESERVED:
            fail(f"{path}: plugin name '{name}' conflicts with a reserved built-in command")

        repo = data["repo"]
        if not isinstance(repo, str) or not repo.startswith("https://github.com/"):
            fail(f"{path}: repo must be a GitHub repository URL")

        version = data["version"]
        if not isinstance(version, str) or not version.strip():
            fail(f"{path}: version must be a non-empty string")

        if version == "latest":
            fail(f"{path}: version must pin an exact reviewed release, not 'latest'")

        if name in names:
            fail(f"{path}: duplicate plugin name '{name}' also defined in {names[name]}")
        names[name] = path

    print(f"Validated {len(files)} plugin metadata files.")


if __name__ == "__main__":
    main()
