#!/usr/bin/env bash
#
# install.sh — Install the CYOA CLI from GitHub releases.
#
# Supports Linux (x86_64) and macOS (x86_64 + Apple Silicon).
# For Windows, use install.ps1 instead.
#
# Usage:
#   curl -fsSL https://alexjwhite-cb.github.io/cyoa-script/install.sh | bash
#   # With options:
#   curl -fsSL https://alexjwhite-cb.github.io/cyoa-script/install.sh | bash -s -- --help
#
# Options:
#   --version <tag>    Install a specific release (e.g. v0.6.0). Default: latest
#   --install-dir <path>  Install to a custom directory instead of auto-detected
#   --no-checksum      Skip SHA256 verification (not recommended)
#   --check            Check if a newer version is available (does not install)
#   --dry-run          Show what would happen without downloading or installing
#   --help, -h         Show this help message

set -euo pipefail

# ── Constants ────────────────────────────────────────────────────────────────

REPO="alexjwhite-cb/cyoa-script"
BINARY_NAME="cyoa"
CHECKSUM_FILE="cyoa-cli-sha256sums.txt"

# ── Defaults ─────────────────────────────────────────────────────────────────

VERSION=""           # empty = latest
INSTALL_DIR=""       # empty = auto-detect
SKIP_CHECKSUM=false
CHECK_ONLY=false
DRY_RUN=false

# ── Helpers ──────────────────────────────────────────────────────────────────

log()  { printf '\033[1;34m→\033[0m %s\n' "$*"; }
ok()   { printf '\033[1;32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!\033[0m %s\n' "$*" >&2; }
err()  { printf '\033[1;31m✗\033[0m %s\n' "$*" >&2; }

usage() {
  cat <<'EOF'
Install the CYOA CLI binary from GitHub releases.

Usage: install.sh [OPTIONS]

Options:
  --version <tag>       Install a specific release (e.g. v0.6.0). Default: latest
  --install-dir <path>  Install to a custom directory instead of auto-detected
  --no-checksum         Skip SHA256 verification (not recommended)
  --check               Check if a newer version is available (does not install)
  --dry-run             Show what would happen without downloading or installing
  --help, -h            Show this help message

Examples:
  install.sh                          # Install latest to auto-detected dir
  install.sh --version v0.6.0         # Install specific version
  install.sh --install-dir ~/.local/bin  # Custom install location
  install.sh --check                  # Just check for updates
  install.sh --dry-run                # Preview without installing
EOF
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { err "Required command not found: $1"; exit 1; }
}

# ── Argument parsing ─────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --install-dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    --no-checksum)
      SKIP_CHECKSUM=true
      shift
      ;;
    --check)
      CHECK_ONLY=true
      shift
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      err "Unknown option: $1"
      usage
      exit 1
      ;;
  esac
done

# ── Detect OS ────────────────────────────────────────────────────────────────

detect_os() {
  local kernel
  kernel="$(uname -s)"
  case "$kernel" in
    Linux)  echo "linux" ;;
    Darwin) echo "macos" ;;
    *)      err "Unsupported OS: $kernel"; err "Use install.ps1 on Windows."; exit 1 ;;
  esac
}

detect_arch() {
  local machine
  machine="$(uname -m)"
  case "$machine" in
    x86_64|amd64) echo "x86_64" ;;
    arm64|aarch64) echo "aarch64" ;;
    *)            err "Unsupported architecture: $machine"; exit 1 ;;
  esac
}

# ── Resolve release URL ───────────────────────────────────────────────────────

# Returns the download URL for a given asset name.
# For "latest", uses the /latest/download/ redirect URL.
# For a specific tag, uses /releases/download/TAG/ASSET.
release_url() {
  local asset="$1"
  if [[ -n "$VERSION" ]]; then
    echo "https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
  else
    echo "https://github.com/${REPO}/releases/latest/download/${asset}"
  fi
}

# Query the GitHub API for the latest release tag (e.g. "v0.6.0").
# Returns just the tag name on stdout.
get_latest_tag() {
  require_cmd curl
  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | sed -E 's/.*"tag_name": "([^"]+)".*/\1/'
}

# ── Check for updates ────────────────────────────────────────────────────────

do_check() {
  require_cmd curl
  require_cmd "$BINARY_NAME"

  local local_version latest_tag latest_version
  local_version="$($BINARY_NAME version 2>/dev/null || echo "unknown")"
  latest_tag="$(get_latest_tag)"
  # Strip leading 'v' for comparison
  latest_version="${latest_tag#v}"

  log "Installed version: ${local_version}"
  log "Latest release:   ${latest_version} (${latest_tag})"

  if [[ "$local_version" == "$latest_version" ]]; then
    ok "You are up to date!"
    return 0
  elif [[ "$local_version" == "unknown" ]]; then
    warn "Could not determine local version."
    return 1
  else
    warn "A new version is available: ${latest_version} (you have ${local_version})"
    warn "Run: curl -fsSL https://alexjwhite-cb.github.io/cyoa-script/install.sh | bash"
    return 1
  fi
}

# ── Detect install directory ──────────────────────────────────────────────────

detect_install_dir() {
  if [[ -n "$INSTALL_DIR" ]]; then
    echo "$INSTALL_DIR"
    return
  fi

  # Prefer /usr/local/bin if writable
  if [[ -w "/usr/local/bin" ]]; then
    echo "/usr/local/bin"
    return
  fi

  # Fallback to ~/.local/bin
  echo "${HOME}/.local/bin"
}

ensure_in_path() {
  local dir="$1"
  # Check if the directory is already on PATH
  case ":${PATH}:" in
    *":${dir}:"*)
      return 0
      ;;
    *)
      # Try to add to shell rc
      local shell_rc=""
      if [[ -f "${HOME}/.bashrc" ]]; then
        shell_rc="${HOME}/.bashrc"
      elif [[ -f "${HOME}/.zshrc" ]]; then
        shell_rc="${HOME}/.zshrc"
      elif [[ -f "${HOME}/.profile" ]]; then
        shell_rc="${HOME}/.profile"
      fi

      if [[ -n "$shell_rc" ]] && ! grep -qF "$dir" "$shell_rc" 2>/dev/null; then
        echo "" >> "$shell_rc"
        echo "# Added by cyoa install script" >> "$shell_rc"
        echo "export PATH=\"\$PATH:${dir}\"" >> "$shell_rc"
        warn "Added ${dir} to PATH in ${shell_rc}"
      fi
      warn "Note: ${dir} is not on your current PATH."
      warn "Run: export PATH=\"\$PATH:${dir}\""
      ;;
  esac
}

# ── Verify checksum ──────────────────────────────────────────────────────────

# Returns 0 if verification passes, exits with error otherwise.
verify_checksum() {
  local download_dir="$1"  # directory containing the downloaded binary
  local binary="$2"        # the binary filename

  require_cmd curl

  local checksum_url sum_file expected_hash actual_hash
  sum_file="cyoa-cli-sha256sums.txt"

  log "Downloading checksums..."
  if ! curl -fsSL "$(release_url "$sum_file")" -o "${download_dir}/${sum_file}"; then
    warn "Could not download checksum file. Skipping verification."
    return 0
  fi

  # Extract the expected hash for our binary
  expected_hash="$(grep -E "[[:space:]]${binary}$" "${download_dir}/${sum_file}" | awk '{print $1}')"
  if [[ -z "$expected_hash" ]]; then
    warn "Binary not found in checksum file. Skipping verification."
    return 0
  fi

  # Compute actual hash
  if command -v shasum >/dev/null 2>&1; then
    actual_hash="$(shasum -a 256 "${download_dir}/${binary}" | awk '{print $1}')"
  elif command -v sha256sum >/dev/null 2>&1; then
    actual_hash="$(sha256sum "${download_dir}/${binary}" | awk '{print $1}')"
  else
    warn "No SHA256 tool available (shasum/sha256sum). Skipping verification."
    return 0
  fi

  if [[ "$actual_hash" != "$expected_hash" ]]; then
    err "Checksum verification failed!"
    err "Expected: ${expected_hash}"
    err "Actual:   ${actual_hash}"
    return 1
  fi

  ok "Checksum verified."
}

# ── Download and install ─────────────────────────────────────────────────────

do_install() {
  require_cmd curl

  local os arch asset
  os="$(detect_os)"
  arch="$(detect_arch)"

  # Map os+arch to release asset name
  case "${os}-${arch}" in
    linux-x86_64)  asset="cyoa-linux-x86_64" ;;
    linux-aarch64) asset="cyoa-linux-aarch64" ;;
    macos-x86_64)  asset="cyoa-macos-x86_64" ;;
    macos-aarch64) asset="cyoa-macos-aarch64" ;;
    *)             err "No pre-built binary for ${os}-${arch}. Build from source."; exit 1 ;;
  esac

  local dest_dir download_dir dest_file
  dest_dir="$(detect_install_dir)"
  download_dir="$(mktemp -d)"
  dest_file="${dest_dir}/${BINARY_NAME}"

  # Resolve version
  local version_tag
  if [[ -n "$VERSION" ]]; then
    version_tag="$VERSION"
  else
    version_tag="$(get_latest_tag)"
  fi

  log "Detected: ${os} ${arch}"
  log "Latest release: ${version_tag}"
  log "Asset: ${asset}"
  log "Install dir: ${dest_dir}"
  log "Binary: ${dest_file}"

  if [[ "$DRY_RUN" == "true" ]]; then
    log "[dry-run] Would download: $(release_url "$asset")"
    log "[dry-run] Would install to: ${dest_file}"
    log "[dry-run] Would make executable: chmod +x ${dest_file}"
    return 0
  fi

  # Create install directory if needed
  mkdir -p "$dest_dir"

  # Download binary
  log "Downloading ${asset}..."
  if ! curl -fSL "$(release_url "$asset")" -o "${download_dir}/${asset}"; then
    err "Download failed. Exiting."
    rm -rf "$download_dir"
    exit 1
  fi
  ok "Downloaded ${asset} ($(wc -c < "${download_dir}/${asset}" | tr -d ' ') bytes)"

  # Verify checksum
  if [[ "$SKIP_CHECKSUM" == "false" ]]; then
    verify_checksum "$download_dir" "$asset"
  else
    warn "Skipping checksum verification (--no-checksum)"
  fi

  # Install
  log "Installing to ${dest_file}..."
  cp "${download_dir}/${asset}" "$dest_file"
  chmod +x "$dest_file"
  rm -rf "$download_dir"

  ok "Installed ${BINARY_NAME} to ${dest_file}"

  # Verify it runs
  if "$dest_file" version >/dev/null 2>&1; then
    ok "Verified: $(${dest_file} version)"
  else
    warn "Binary installed but could not verify version. Try running it manually."
  fi

  # Ensure install dir is on PATH
  ensure_in_path "$dest_dir"
}

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
  if [[ "$CHECK_ONLY" == "true" ]]; then
    do_check
    exit $?
  fi

  do_install
}

main "$@"
