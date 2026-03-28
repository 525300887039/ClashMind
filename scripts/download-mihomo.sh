#!/usr/bin/env bash
# Download mihomo binary for local development
# Usage: bash scripts/download-mihomo.sh [version]
# Example: bash scripts/download-mihomo.sh v1.19.10

set -euo pipefail

VERSION="${1:-}"
BINARIES_DIR="src-tauri/binaries"

# Detect platform and set asset/sidecar names
detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64)  ASSET="mihomo-linux-amd64";    SIDECAR="mihomo-x86_64-unknown-linux-gnu";    EXT=".gz" ;;
        aarch64) ASSET="mihomo-linux-arm64";     SIDECAR="mihomo-aarch64-unknown-linux-gnu";   EXT=".gz" ;;
        *)       echo "Unsupported Linux arch: $arch"; exit 1 ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64)  ASSET="mihomo-darwin-amd64";    SIDECAR="mihomo-x86_64-apple-darwin";         EXT=".gz" ;;
        arm64)   ASSET="mihomo-darwin-arm64";     SIDECAR="mihomo-aarch64-apple-darwin";        EXT=".gz" ;;
        *)       echo "Unsupported macOS arch: $arch"; exit 1 ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      case "$arch" in
        x86_64)  ASSET="mihomo-windows-amd64";   SIDECAR="mihomo-x86_64-pc-windows-msvc.exe";  EXT=".zip" ;;
        aarch64) ASSET="mihomo-windows-arm64";    SIDECAR="mihomo-aarch64-pc-windows-msvc.exe"; EXT=".zip" ;;
        *)       echo "Unsupported Windows arch: $arch"; exit 1 ;;
      esac
      ;;
    *)
      echo "Unsupported OS: $os"; exit 1 ;;
  esac
}

# Resolve latest version from GitHub API
resolve_version() {
  if [ -z "$VERSION" ]; then
    echo "Resolving latest mihomo version..."
    VERSION=$(curl -sL https://api.github.com/repos/MetaCubeX/mihomo/releases/latest \
      | grep '"tag_name"' | head -1 | cut -d'"' -f4)
    if [ -z "$VERSION" ]; then
      echo "Failed to resolve latest version. Please specify a version manually."
      exit 1
    fi
  fi
  echo "Using mihomo version: $VERSION"
}

# Download and extract
download() {
  local url="https://github.com/MetaCubeX/mihomo/releases/download/${VERSION}/${ASSET}-${VERSION}${EXT}"
  local archive="mihomo-archive${EXT}"

  mkdir -p "$BINARIES_DIR"

  echo "Downloading: $url"
  curl -fSL -o "$archive" "$url"

  if [ "$EXT" = ".gz" ]; then
    gunzip -c "$archive" > "${BINARIES_DIR}/${SIDECAR}"
    chmod +x "${BINARIES_DIR}/${SIDECAR}"
  else
    # Windows .zip
    local tmpdir="mihomo-tmp"
    rm -rf "$tmpdir"
    unzip -o "$archive" -d "$tmpdir"
    mv "$tmpdir"/mihomo*.exe "${BINARIES_DIR}/${SIDECAR}"
    rm -rf "$tmpdir"
  fi

  rm -f "$archive"
  echo "Done! Binary saved to: ${BINARIES_DIR}/${SIDECAR}"
  ls -la "${BINARIES_DIR}/"
}

detect_platform
resolve_version
download
