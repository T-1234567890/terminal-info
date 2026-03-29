#!/usr/bin/env bash

set -euo pipefail

REPO_OWNER="T-1234567890"
REPO_NAME="terminal-info"
BINARY_NAME="tinfo"
PRIMARY_INSTALL_DIR="/usr/local/bin"
FALLBACK_INSTALL_DIR="${HOME}/.local/bin"
MINISIGN_PUBLIC_KEY="RWRgzvl/IRChlCdww8KtvuohEfnRA++x8Ro1hql1KOvVAVItAXEsC0jN"

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
  local os
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

require_sha_tool() {
  if command -v sha256sum >/dev/null 2>&1; then
    echo "sha256sum"
    return 0
  fi

  if command -v shasum >/dev/null 2>&1; then
    echo "shasum"
    return 0
  fi

  echo "Missing required command: sha256sum or shasum" >&2
  exit 1
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

download_file() {
  local url="$1"
  local destination="$2"

  curl -fsSL "$url" -o "$destination"
}

parse_checksum() {
  local checksum_file="$1"
  local archive_name="$2"
  local checksum line parsed_checksum parsed_name

  while IFS= read -r line || [ -n "$line" ]; do
    [ -z "$line" ] && continue
    parsed_checksum="${line%%[[:space:]]*}"
    parsed_name="${line#"$parsed_checksum"}"
    parsed_name="${parsed_name#"${parsed_name%%[![:space:]]*}"}"
    parsed_name="${parsed_name#\*}"
    if [ "$parsed_name" = "$archive_name" ]; then
      checksum="$(printf '%s' "$parsed_checksum" | tr '[:upper:]' '[:lower:]')"
      if ! printf '%s' "$checksum" | grep -Eq '^[0-9a-f]{64}$'; then
        echo "Verification failed ❌" >&2
        echo "Checksum file contained an invalid SHA-256 value." >&2
        exit 1
      fi
      printf '%s' "$checksum"
      return 0
    fi
  done <"$checksum_file"

  echo "Verification failed ❌" >&2
  echo "Checksum file did not contain an entry for ${archive_name}." >&2
  exit 1
}

calculate_sha256() {
  local tool="$1"
  local file="$2"

  if [ "$tool" = "sha256sum" ]; then
    sha256sum "$file" | awk '{print $1}'
  else
    shasum -a 256 "$file" | awk '{print $1}'
  fi
}

verify_checksum() {
  local archive_path="$1"
  local checksum_path="$2"
  local archive_name="$3"
  local sha_tool expected actual

  echo "Verifying checksum..."
  sha_tool="$(require_sha_tool)"
  expected="$(parse_checksum "$checksum_path" "$archive_name")"
  actual="$(calculate_sha256 "$sha_tool" "$archive_path" | tr '[:upper:]' '[:lower:]')"

  if [ "$actual" != "$expected" ]; then
    echo "Verification failed ❌" >&2
    echo "SHA-256 checksum mismatch for ${archive_name}." >&2
    exit 1
  fi
}

verify_signature() {
  local archive_path="$1"
  local signature_path="$2"

  echo "Verifying signature..."
  if ! minisign -Vm "$archive_path" -x "$signature_path" -P "$MINISIGN_PUBLIC_KEY" >/dev/null 2>&1; then
    echo "Verification failed ❌" >&2
    echo "Minisign verification failed for ${archive_path##*/}." >&2
    exit 1
  fi
}

main() {
  require_cmd curl
  require_cmd tar
  require_cmd mkdir
  require_cmd mv
  require_cmd chmod

  if ! command -v minisign >/dev/null 2>&1; then
    echo "Missing required command: minisign" >&2
    echo "Install minisign and run the installer again." >&2
    exit 1
  fi

  local arch platform archive_name archive_url signature_url checksum_url tmp_dir archive_path
  local signature_path checksum_path binary_path install_dir install_path

  tmp_dir=""
  trap 'if [ -n "${tmp_dir:-}" ]; then rm -rf "$tmp_dir"; fi' EXIT

  arch="$(detect_arch)"
  platform="$(detect_platform)"
  archive_name="${BINARY_NAME}-${arch}-${platform}.tar.gz"
  archive_url="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/${archive_name}"
  signature_url="${archive_url}.minisig"
  checksum_url="${archive_url}.sha256"
  install_dir="$(select_install_dir)"
  install_path="${install_dir}/${BINARY_NAME}"

  tmp_dir="$(mktemp -d)"
  archive_path="${tmp_dir}/${archive_name}"
  signature_path="${archive_path}.minisig"
  checksum_path="${archive_path}.sha256"

  mkdir -p "$install_dir"

  echo "Downloading release..."
  download_file "$archive_url" "$archive_path"
  download_file "$signature_url" "$signature_path"
  download_file "$checksum_url" "$checksum_path"

  verify_checksum "$archive_path" "$checksum_path" "$archive_name"
  verify_signature "$archive_path" "$signature_path"

  echo "Verification successful ✔"

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
