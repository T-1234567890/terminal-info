#!/usr/bin/env python3

import json
import re
import sys
from pathlib import Path
from urllib.parse import urlparse

ALLOWED_PLATFORMS = {"linux", "macos", "windows"}
NAME_RE = re.compile(r"^[a-z0-9_-]+$")
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
SHA256_RE = re.compile(r"^[0-9a-fA-F]{64}$")
MAX_SHORT_DESCRIPTION = 79


def validate_schema(data):
    errors = []

    if not isinstance(data, dict):
        return ["root JSON value must be an object"]

    required_fields = {
        "name",
        "version",
        "description",
        "short_description",
        "author",
        "binary",
        "platform",
        "entry",
    }

    for field in sorted(required_fields):
        if field not in data:
            errors.append(f"missing required field: {field}")

    name = data.get("name")
    if "name" in data:
        if not isinstance(name, str) or not name.strip():
            errors.append("name must be a non-empty string")
        else:
            if " " in name:
                errors.append("name must not contain spaces")
            if not NAME_RE.fullmatch(name):
                errors.append("name must use only [a-z0-9-_]")

    version = data.get("version")
    if "version" in data:
        if not isinstance(version, str) or not VERSION_RE.fullmatch(version):
            errors.append("version must match semver-like pattern 1.0.0")

    description = data.get("description")
    if "description" in data and not isinstance(description, str):
        errors.append("description must be a string")

    short_description = data.get("short_description")
    if "short_description" in data:
        if not isinstance(short_description, str) or not short_description.strip():
            errors.append("short_description must be a non-empty string")
        else:
            if "\n" in short_description or "\r" in short_description:
                errors.append("short_description must be a single line")
            if len(short_description.strip()) > MAX_SHORT_DESCRIPTION:
                errors.append("short_description must be under 80 characters")

    author = data.get("author")
    if "author" in data and not isinstance(author, str):
        errors.append("author must be a string")

    binary = data.get("binary")
    if "binary" in data:
        if not isinstance(binary, str) or not binary.strip():
            errors.append("binary must be a non-empty string")

    entry = data.get("entry")
    if "entry" in data:
        if not isinstance(entry, str) or not entry.strip():
            errors.append("entry must be a non-empty string")

    platform = data.get("platform")
    if "platform" in data:
        if not isinstance(platform, list):
            errors.append("platform must be an array of strings")
        elif not platform:
            errors.append("platform array must not be empty")
        else:
            for value in platform:
                if not isinstance(value, str):
                    errors.append("platform entries must be strings")
                    continue
                if value not in ALLOWED_PLATFORMS:
                    errors.append(
                        f"platform '{value}' is invalid; expected subset of linux, macos, windows"
                    )

    homepage = data.get("homepage")
    if homepage is not None and not _is_valid_url(homepage):
        errors.append("homepage must be a valid http:// or https:// URL")

    repository = data.get("repository")
    if repository is not None and not _is_valid_url(repository):
        errors.append("repository must be a valid http:// or https:// URL")

    sha256 = data.get("sha256")
    if sha256 is not None:
        if not isinstance(sha256, str) or not SHA256_RE.fullmatch(sha256):
            errors.append("sha256 must be exactly 64 hexadecimal characters")

    signature = data.get("signature")
    if signature is not None and not isinstance(signature, str):
        errors.append("signature must be a string")

    return errors


def validate_file(path):
    print(f"Validating plugin: {path}")

    try:
        contents = path.read_text(encoding="utf-8")
    except OSError as exc:
        return False, f"unable to read file: {exc}"

    try:
        data = json.loads(contents)
    except json.JSONDecodeError as exc:
        return False, f"invalid JSON: {exc}"

    errors = validate_schema(data)
    if errors:
        return False, "; ".join(errors)

    return True, None


def _is_valid_url(value):
    if not isinstance(value, str) or not value.strip():
        return False

    parsed = urlparse(value)
    return parsed.scheme in {"http", "https"} and bool(parsed.netloc)


def _discover_files(argv):
    if len(argv) > 2:
        raise ValueError("usage: python3 scripts/validate_plugins.py [plugins/file.json]")

    if len(argv) == 2:
        return [Path(argv[1])]

    plugin_dir = Path("plugins")
    if not plugin_dir.exists():
        raise ValueError("plugins/ directory does not exist")

    files = sorted(path for path in plugin_dir.glob("*.json") if path.name != "index.json")
    if not files:
        raise ValueError("no plugin metadata files found in plugins/")
    return files


def main():
    try:
        files = _discover_files(sys.argv)
    except ValueError as exc:
        print(f"✗ Failed: {exc}", file=sys.stderr)
        return 1

    total = 0
    failed = 0

    for path in files:
        total += 1
        ok, error = validate_file(path)
        if ok:
            print("✓ Passed")
        else:
            failed += 1
            print(f"✗ Failed: {error}")

    print(f"{total} plugins validated, {failed} failed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
