<#
.SYNOPSIS
    Install the CYOA CLI binary from GitHub releases on Windows.

.DESCRIPTION
    Downloads the appropriate cyoa CLI binary for Windows (x86_64) from the
    latest (or specified) GitHub release, verifies its SHA256 checksum, and
    installs it to a directory on PATH.

    For Linux or macOS, use install.sh instead.

.PARAMETER Version
    Install a specific release tag (e.g. "v0.6.0"). Default: latest release.

.PARAMETER InstallDir
    Install to a custom directory instead of the auto-detected default.
    Default: $env:LOCALAPPDATA\Programs\cyoa (user) or C:\Program Files\cyoa (admin).

.PARAMETER NoChecksum
    Skip SHA256 checksum verification. Not recommended.

.PARAMETER Check
    Check if a newer version is available without installing.

.PARAMETER DryRun
    Show what would happen without downloading or installing.

.PARAMETER Help
    Show help message.

.EXAMPLE
    PS> .\install.ps1
    Installs the latest release to the default directory.

.EXAMPLE
    PS> .\install.ps1 -Version v0.6.0
    Installs a specific version.

.EXAMPLE
    PS> .\install.ps1 -InstallDir "C:\tools\cyoa"
    Installs to a custom directory.

.EXAMPLE
    PS> .\install.ps1 -Check
    Only checks for updates.

.NOTES
    Requires PowerShell 5.1+ (ships with Windows 10+) or PowerShell 7.
    Requires Invoke-WebRequest (available by default).
#>

[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [switch]$NoChecksum,
    [switch]$Check,
    [switch]$DryRun,
    [switch]$Help
)

# ── Constants ────────────────────────────────────────────────────────────────

$Repo = "alexjwhite-cb/cyoa-script"
$BinaryName = "cyoa"
$ChecksumFile = "cyoa-cli-sha256sums.txt"

# ── Helpers ──────────────────────────────────────────────────────────────────

function Write-Step($msg) {
    Write-Host "→ $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "✓ $msg" -ForegroundColor Green
}

function Write-Warn($msg) {
    Write-Host "! $msg" -ForegroundColor Yellow
}

function Write-Err($msg) {
    Write-Host "✗ $msg" -ForegroundColor Red
}

# ── Argument handling ────────────────────────────────────────────────────────

if ($Help) {
    Get-Help $MyInvocation.MyCommand.Path -Detailed
    exit 0
}

# ── Detect architecture ──────────────────────────────────────────────────────

function Get-Arch {
    if ([Environment]::Is64BitOperatingSystem) {
        return "x86_64"
    }
    Write-Err "Only 64-bit Windows is supported."
    exit 1
}

function Get-LatestTag {
    $url = "https://api.github.com/repos/$Repo/releases/latest"
    $response = Invoke-RestMethod -Uri $url -UseBasicParsing
    return $response.tag_name
}

function Get-ReleaseUrl([string]$asset) {
    if ($Version) {
        return "https://github.com/$Repo/releases/download/$Version/$asset"
    } else {
        return "https://github.com/$Repo/releases/latest/download/$asset"
    }
}

# ── Check for updates ────────────────────────────────────────────────────────

function Do-Check {
    # Ensure cyoa is on PATH
    $localExe = Get-Command $BinaryName -ErrorAction SilentlyContinue
    if (-not $localExe) {
        Write-Warn "cyoa is not installed or not on PATH."
        Write-Host "  Install it first with: .\install.ps1"
        exit 1
    }

    $localVersion = & $localExe version 2>$null
    if (-not $localVersion) {
        # Fall back to --version (clap built-in)
        $localVersion = & $localExe --version 2>$null | ForEach-Object {
            $_ -replace 'cyoa\s+', ''
        }
    }

    Write-Step "Installed version: $localVersion"

    $latestTag = Get-LatestTag
    $latestVersion = $latestTag.TrimStart('v')
    Write-Step "Latest release:   $latestVersion ($latestTag)"

    if ($localVersion -eq $latestVersion) {
        Write-Ok "You are up to date!"
        exit 0
    } elseif ($localVersion) {
        Write-Warn "A new version is available: $latestVersion (you have $localVersion)"
        Write-Warn "Run: iwr https://alexjwhite-cb.github.io/cyoa-script/install.ps1 | iex"
        exit 1
    } else {
        Write-Warn "Could not determine local version."
        exit 1
    }
}

# ── Detect install directory ─────────────────────────────────────────────────

function Get-InstallDir {
    if ($InstallDir) {
        return $InstallDir
    }

    # If running as admin, prefer C:\Program Files\cyoa
    $isAdmin = ([Security.Principal.WindowsPrincipal] `
        [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(`
        [Security.Principal.WindowsBuiltInRole]::Administrator)

    if ($isAdmin) {
        $dir = "C:\Program Files\cyoa"
    } else {
        $dir = "$env:LOCALAPPDATA\Programs\cyoa"
    }

    return $dir
}

function Ensure-InPath([string]$dir) {
    $currentPath = [System.Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -split ";" -contains $dir) {
        return
    }

    # Add to user PATH
    $newPath = if ($currentPath) { "$currentPath;$dir" } else { $dir }
    [System.Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    Write-Warn "Added $dir to user PATH. Restart your terminal to apply."

    # Also check if it's on the system PATH
    $sysPath = [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    if ($sysPath -and ($sysPath -split ";" -contains $dir)) {
        return
    }
}

# ── Verify checksum ──────────────────────────────────────────────────────────

function Verify-Checksum([string]$DownloadDir, [string]$binaryFile) {
    Write-Step "Downloading checksums..."

    $sumPath = Join-Path $DownloadDir $ChecksumFile
    $sumUrl = Get-ReleaseUrl $ChecksumFile

    try {
        Invoke-WebRequest -Uri $sumUrl -OutFile $sumPath -UseBasicParsing
    } catch {
        Write-Warn "Could not download checksum file. Skipping verification."
        return $true
    }

    $lines = Get-Content $sumPath
    $expectedLine = $lines | Where-Object { $_ -match "[\s]+$([System.IO.Path]::GetFileName($binaryFile))$" }
    if (-not $expectedLine) {
        Write-Warn "Binary not found in checksum file. Skipping verification."
        return $true
    }

    $expectedHash = ($expectedLine -split '\s+')[0].ToLower()
    $actualHash = (Get-FileHash -Path $binaryFile -Algorithm SHA256).Hash.ToLower()

    if ($actualHash -ne $expectedHash) {
        Write-Err "Checksum verification failed!"
        Write-Err "Expected: $expectedHash"
        Write-Err "Actual:   $actualHash"
        return $false
    }

    Write-Ok "Checksum verified."
    return $true
}

# ── Main ─────────────────────────────────────────────────────────────────────

if ($Check) {
    Do-Check
    exit $LASTEXITCODE
}

# Detect
$os = "windows"
$arch = Get-Arch

# Map os+arch to release asset name
switch ("$os-$arch") {
    "windows-x86_64" { $asset = "cyoa-windows-x86_64.exe" }
    default {
        Write-Err "No pre-built binary for $os-$arch."
        exit 1
    }
}

$destDir = Get-InstallDir
$downloadDir = [System.IO.Path]::GetTempPath() + [System.Guid]::NewGuid().ToString()
New-Item -ItemType Directory -Path $downloadDir -Force | Out-Null
$destFile = Join-Path $destDir "$BinaryName.exe"

# Resolve version
if ($Version) {
    $versionTag = $Version
} else {
    $versionTag = Get-LatestTag
}

Write-Step "Detected: $os $arch"
Write-Step "Latest release: $versionTag"
Write-Step "Asset: $asset"
Write-Step "Install dir: $destDir"
Write-Step "Binary: $destFile"

if ($DryRun) {
    Write-Step "[dry-run] Would download: $(Get-ReleaseUrl $asset)"
    Write-Step "[dry-run] Would install to: $destFile"
    Write-Step "[dry-run] Would verify checksum"
    exit 0
}

# Download
Write-Step "Downloading $asset..."
$assetPath = Join-Path $downloadDir $asset
try {
    Invoke-WebRequest -Uri $(Get-ReleaseUrl $asset) -OutFile $assetPath -UseBasicParsing
    $size = (Get-Item $assetPath).Length
    Write-Ok "Downloaded $asset ($size bytes)"
} catch {
    Write-Err "Download failed: $($_.Exception.Message)"
    Remove-Item $downloadDir -Recurse -Force -ErrorAction SilentlyContinue
    exit 1
}

# Verify checksum
if (-not $NoChecksum) {
    $ok = Verify-Checksum $downloadDir $assetPath
    if (-not $ok) {
        Remove-Item $downloadDir -Recurse -Force -ErrorAction SilentlyContinue
        exit 1
    }
} else {
    Write-Warn "Skipping checksum verification (-NoChecksum)"
}

# Install
Write-Step "Installing to $destFile..."
if (-not (Test-Path $destDir)) {
    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
}
Copy-Item $assetPath $destFile -Force
Remove-Item $downloadDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Ok "Installed $BinaryName to $destFile"

# Ensure install dir is on PATH
Ensure-InPath $destDir

# Verify it runs
try {
    $version = & $destFile version 2>$null
    if ($version) {
        Write-Ok "Verified: $version"
    } else {
        $version = & $destFile --version 2>$null | ForEach-Object { $_ -replace "cyoa\s+", "" }
        if ($version) {
            Write-Ok "Verified: $version"
        } else {
            Write-Warn "Binary installed but could not verify version."
        }
    }
} catch {
    Write-Warn "Binary installed but could not verify version."
}
