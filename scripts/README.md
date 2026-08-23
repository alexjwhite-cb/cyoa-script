# Install Scripts

Automated installation scripts for the CYOA CLI binary. These scripts download
the latest (or a specific) release from GitHub, verify the SHA256 checksum, and
install the binary to a directory on your PATH.

## Quick Start

### Linux / macOS (bash)

```bash
curl -fsSL https://alexjwhite-cb.github.io/cyoa-script/install.sh | bash
```

### Windows (PowerShell)

```powershell
iwr https://alexjwhite-cb.github.io/cyoa-script/install.ps1 -UseBasicParsing | iex
```

## Options

### install.sh (Linux / macOS)

| Option | Description |
|--------|-------------|
| `--version <tag>` | Install a specific release (e.g. `v0.6.0`). Default: latest |
| `--install-dir <path>` | Install to a custom directory |
| `--no-checksum` | Skip SHA256 verification (not recommended) |
| `--check` | Check if a newer version is available (does not install) |
| `--dry-run` | Show what would happen without downloading |
| `--help` / `-h` | Show help |

### install.ps1 (Windows)

| Option | Description |
|--------|-------------|
| `-Version <tag>` | Install a specific release (e.g. `v0.6.0`). Default: latest |
| `-InstallDir <path>` | Install to a custom directory |
| `-NoChecksum` | Skip SHA256 verification (not recommended) |
| `-Check` | Check if a newer version is available (does not install) |
| `-DryRun` | Show what would happen without downloading |
| `-Help` | Show help |

## How It Works

1. **Detects** your OS and CPU architecture.
2. **Resolves** the correct release asset name (e.g. `cyoa-linux-x86_64`).
3. **Downloads** the binary (and the `cyoa-cli-sha256sums.txt` checksum file)
   from the latest GitHub release.
4. **Verifies** the download against the SHA256 checksums published with each
   release.
5. **Installs** the binary (renamed to `cyoa`) to a directory on your PATH:
   - Linux/macOS: `/usr/local/bin` (if writable) or `~/.local/bin`
   - Windows: `C:\Program Files\cyoa` (admin) or `%LOCALAPPDATA%\Programs\cyoa`
6. **Ensures** the install directory is on your `PATH` (adds to shell rc or
   Windows user PATH if needed).

## Version Checking

Both scripts support a `--check` / `-Check` mode that compares your installed
`cyoa version` against the latest GitHub release and tells you if an upgrade is
available — without installing anything.

```bash
# Unix
cyoa-install.sh --check

# Windows
.\install.ps1 -Check
```
