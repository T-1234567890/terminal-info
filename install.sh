#!/usr/bin/env bash

set -euo pipefail

REPO_OWNER="T-1234567890"
REPO_NAME="terminal-info"
BINARY_NAME="tinfo"
INSTALL_PATH="/usr/local/bin/${BINARY_NAME}"

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64)
          echo "x86_64-unknown-linux-gnu"
          ;;
        *)
          echo "Unsupported architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64)
          echo "x86_64-apple-darwin"
          ;;
        arm64|aarch64)
          echo "aarch64-apple-darwin"
          ;;
        *)
          echo "Unsupported architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "Unsupported operating system: $os" >&2
      exit 1
      ;;
  esac
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

install_binary() {
  local source="$1"

  if [ -w "$(dirname "$INSTALL_PATH")" ]; then
    install -m 0755 "$source" "$INSTALL_PATH"
  else
    sudo install -m 0755 "$source" "$INSTALL_PATH"
  fi
}

main() {
  require_cmd curl
  require_cmd tar
  require_cmd install

  local target archive_url tmp_dir archive_path binary_path
  tmp_dir=""
  trap 'if [ -n "${tmp_dir:-}" ]; then rm -rf "$tmp_dir"; fi' EXIT

  target="$(detect_target)"
  archive_url="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/${BINARY_NAME}-${target}.tar.gz"
  tmp_dir="$(mktemp -d)"
  archive_path="${tmp_dir}/${BINARY_NAME}-${target}.tar.gz"

  echo "Downloading ${BINARY_NAME} for ${target}..."
  curl -fsSL "$archive_url" -o "$archive_path"

  tar -xzf "$archive_path" -C "$tmp_dir"
  binary_path="${tmp_dir}/${BINARY_NAME}"

  if [ ! -f "$binary_path" ]; then
    echo "Downloaded archive did not contain ${BINARY_NAME}." >&2
    exit 1
  fi

  install_binary "$binary_path"
  chmod +x "$INSTALL_PATH"

  echo "Installed ${BINARY_NAME} to ${INSTALL_PATH}"
  echo "Run '${BINARY_NAME} --help' to get started."
}

main "$@"
