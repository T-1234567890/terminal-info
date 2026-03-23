#!/usr/bin/env bash

set -euo pipefail

REPO_OWNER="T-1234567890"
REPO_NAME="terminal-info"
BINARY_NAME="tinfo"
PRIMARY_INSTALL_DIR="/usr/local/bin"
FALLBACK_INSTALL_DIR="${HOME}/.local/bin"

detect_arch() {
  local arch
  arch="$(uname -m)"

  case "$arch" in
    x86_64)
      echo "x86_64"
      ;;
    arm64|aarch64)
      echo "aarch64"
      ;;
    *)
      echo "Unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac
}

detect_platform() {
  local os arch
  os="$(uname -s)"

  case "$os" in
    Linux)
      echo "unknown-linux-gnu"
      ;;
    Darwin)
      echo "apple-darwin"
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

select_install_dir() {
  if [ -d "$PRIMARY_INSTALL_DIR" ] && [ -w "$PRIMARY_INSTALL_DIR" ]; then
    echo "$PRIMARY_INSTALL_DIR"
  else
    echo "$FALLBACK_INSTALL_DIR"
  fi
}

path_contains_dir() {
  local dir="$1"
  case ":${PATH:-}:" in
    *":$dir:"*) return 0 ;;
    *) return 1 ;;
  esac
}

main() {
  require_cmd curl
  require_cmd tar
  require_cmd mkdir
  require_cmd mv
  require_cmd chmod

  local arch platform archive_name archive_url tmp_dir archive_path binary_path install_dir install_path
  tmp_dir=""
  trap 'if [ -n "${tmp_dir:-}" ]; then rm -rf "$tmp_dir"; fi' EXIT

  arch="$(detect_arch)"
  platform="$(detect_platform)"
  archive_name="${BINARY_NAME}-${arch}-${platform}.tar.gz"
  archive_url="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/${archive_name}"
  install_dir="$(select_install_dir)"
  install_path="${install_dir}/${BINARY_NAME}"

  tmp_dir="$(mktemp -d)"
  archive_path="${tmp_dir}/${archive_name}"

  mkdir -p "$install_dir"

  echo "Downloading ${BINARY_NAME} for ${arch}-${platform}..."
  curl -fsSL "$archive_url" -o "$archive_path"

  tar -xzf "$archive_path" -C "$tmp_dir"
  binary_path="${tmp_dir}/${BINARY_NAME}"

  if [ ! -f "$binary_path" ]; then
    echo "Downloaded archive did not contain ${BINARY_NAME}." >&2
    exit 1
  fi

  chmod +x "$binary_path"
  echo "Installing ${BINARY_NAME} to ${install_dir}"
  mv -f "$binary_path" "$install_path"

  echo "${BINARY_NAME} installed successfully."
  echo "Run \`${BINARY_NAME} --help\` to get started."

  if [ "$install_dir" = "$FALLBACK_INSTALL_DIR" ] && ! path_contains_dir "$FALLBACK_INSTALL_DIR"; then
    echo
    echo "${FALLBACK_INSTALL_DIR} is not in your PATH."
    echo "Add this to ~/.zshrc:"
    echo "export PATH=\"\$HOME/.local/bin:\$PATH\""
  fi
}

main "$@"
