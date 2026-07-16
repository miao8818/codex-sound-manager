param(
    [switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

function Invoke-CheckedNative {
    param(
        [Parameter(Mandatory)]
        [string]$Step,

        [Parameter(Mandatory)]
        [scriptblock]$Command
    )

    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Step failed with exit code $LASTEXITCODE."
    }
}

function Assert-VersionConsistency {
    param(
        [Parameter(Mandatory)]
        [string]$ProjectRoot
    )

    $tauriConfig = Get-Content -LiteralPath (Join-Path $ProjectRoot 'tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $package = Get-Content -LiteralPath (Join-Path $ProjectRoot 'package.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $packageLock = Get-Content -LiteralPath (Join-Path $ProjectRoot 'package-lock.json') -Raw -Encoding UTF8
    $cargoToml = Get-Content -LiteralPath (Join-Path $ProjectRoot 'Cargo.toml') -Raw -Encoding UTF8
    $cargoLock = Get-Content -LiteralPath (Join-Path $ProjectRoot 'Cargo.lock') -Raw -Encoding UTF8
    $cargoVersion = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    $cargoLockVersion = [regex]::Match(
        $cargoLock,
        '(?ms)\[\[package\]\]\s+name\s*=\s*"codex-sound-manager"\s+version\s*=\s*"([^"]+)"'
    ).Groups[1].Value
    $packageLockVersion = [regex]::Match($packageLock, '(?m)^  "version": "([^"]+)",').Groups[1].Value
    $packageLockRootVersion = [regex]::Match(
        $packageLock,
        '(?ms)"packages"\s*:\s*\{\s*""\s*:\s*\{.*?"version"\s*:\s*"([^"]+)"'
    ).Groups[1].Value
    $versions = @(
        [string]$tauriConfig.version,
        [string]$package.version,
        $packageLockVersion,
        $packageLockRootVersion,
        $cargoVersion,
        $cargoLockVersion
    )
    $uniqueVersions = @($versions | Where-Object { $_ } | Select-Object -Unique)
    $missingVersions = @($versions | Where-Object { [string]::IsNullOrWhiteSpace($_) })
    if ($missingVersions.Count -ne 0 -or $uniqueVersions.Count -ne 1) {
        throw "Project versions are inconsistent: $($versions -join ', ')"
    }
    Write-Host "Project version: $($uniqueVersions[0])"
}

function Assert-ReleaseExecutableNotRunning {
    param(
        [Parameter(Mandatory)]
        [string]$ProjectRoot
    )

    $releaseExecutable = Join-Path $ProjectRoot 'target\release\codex-sound-manager.exe'
    $running = Get-Process -Name 'codex-sound-manager' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $releaseExecutable }
    if ($running) {
        throw 'Codex Sound Manager is still running. Close it before building a release.'
    }
}

function Sync-TauriIcons {
    param(
        [Parameter(Mandatory)]
        [string]$ProjectRoot
    )

    $source = Join-Path $ProjectRoot 'icons\app-icon.png'
    $stamp = Join-Path $ProjectRoot 'icons\.source-sha256'
    $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant()
    $storedHash = if ([System.IO.File]::Exists($stamp)) {
        [System.IO.File]::ReadAllText($stamp, [System.Text.Encoding]::UTF8).Trim()
    }
    else {
        ''
    }
    if ($storedHash -eq $sourceHash) {
        Write-Host 'Tauri icons are already synchronized.'
        return
    }

    Invoke-CheckedNative -Step 'Tauri icon generation' -Command { npm run tauri -- icon icons/app-icon.png }
    [System.IO.File]::WriteAllText($stamp, "$sourceHash`n", [System.Text.UTF8Encoding]::new($false))
}

function New-PortablePackage {
    param(
        [Parameter(Mandatory)]
        [string]$ProjectRoot
    )

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem

    $config = Get-Content -LiteralPath (Join-Path $ProjectRoot 'tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $portableDirectory = Join-Path $ProjectRoot 'target\release\bundle\portable'
    [System.IO.Directory]::CreateDirectory($portableDirectory) | Out-Null
    $zipPath = Join-Path $portableDirectory "CodexSoundManager_$($config.version)_x64-portable.zip"
    $items = @(
        @{ Source = 'target\release\codex-sound-manager.exe'; Entry = 'codex-sound-manager.exe' },
        @{ Source = 'Run-Portable.vbs'; Entry = 'Run-Portable.vbs' },
        @{ Source = 'README.md'; Entry = 'README.md' },
        @{ Source = 'README_EN.md'; Entry = 'README_EN.md' },
        @{ Source = 'LICENSE'; Entry = 'LICENSE' },
        @{ Source = 'THIRD_PARTY_NOTICES.md'; Entry = 'THIRD_PARTY_NOTICES.md' },
        @{ Source = 'sounds\default-notification.wav'; Entry = 'sounds/default-notification.wav' },
        @{ Source = 'icons\app-icon.png'; Entry = 'icons/app-icon.png' },
        @{ Source = 'docs\images\app-screenshot.png'; Entry = 'docs/images/app-screenshot.png' },
        @{ Source = 'docs\images\community-qr.jpg'; Entry = 'docs/images/community-qr.jpg' }
    )

    $fileStream = [System.IO.File]::Open($zipPath, [System.IO.FileMode]::Create)
    try {
        $archive = [System.IO.Compression.ZipArchive]::new(
            $fileStream,
            [System.IO.Compression.ZipArchiveMode]::Create,
            $false,
            [System.Text.UTF8Encoding]::new($false)
        )
        try {
            foreach ($item in $items) {
                $source = Join-Path $ProjectRoot $item.Source
                if (-not [System.IO.File]::Exists($source)) {
                    throw "Portable package file is missing: $source"
                }
                [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                    $archive,
                    $source,
                    $item.Entry,
                    [System.IO.Compression.CompressionLevel]::Optimal
                ) | Out-Null
            }
        }
        finally {
            $archive.Dispose()
        }
    }
    finally {
        $fileStream.Dispose()
    }

    Write-Host "Portable package: $zipPath"
}

function Copy-ReleaseInstaller {
    param(
        [Parameter(Mandatory)]
        [string]$ProjectRoot
    )

    $config = Get-Content -LiteralPath (Join-Path $ProjectRoot 'tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $nsisDirectory = Join-Path $ProjectRoot 'target\release\bundle\nsis'
    $installer = Get-ChildItem -LiteralPath $nsisDirectory -File |
        Where-Object { $_.Name -like "*_$($config.version)_x64-setup.exe" } |
        Select-Object -First 1
    if (-not $installer) {
        throw "NSIS installer is missing from $nsisDirectory"
    }

    $releaseDirectory = Join-Path $ProjectRoot 'target\release\bundle\release'
    [System.IO.Directory]::CreateDirectory($releaseDirectory) | Out-Null
    $destination = Join-Path $releaseDirectory "CodexSoundManager_$($config.version)_x64-setup.exe"
    Copy-Item -LiteralPath $installer.FullName -Destination $destination -Force
    Write-Host "Release installer: $destination"
}

function Write-ReleaseChecksums {
    param(
        [Parameter(Mandatory)]
        [string]$ProjectRoot
    )

    $config = Get-Content -LiteralPath (Join-Path $ProjectRoot 'tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $installerName = "CodexSoundManager_$($config.version)_x64-setup.exe"
    $portableName = "CodexSoundManager_$($config.version)_x64-portable.zip"
    $releaseDirectory = Join-Path $ProjectRoot 'target\release\bundle\release'
    $files = @(
        (Join-Path $releaseDirectory $installerName)
        (Join-Path $ProjectRoot "target\release\bundle\portable\$portableName")
    )
    $lines = foreach ($file in $files) {
        if (-not [System.IO.File]::Exists($file)) {
            throw "Release checksum file is missing: $file"
        }
        $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $([System.IO.Path]::GetFileName($file))"
    }
    $checksumPath = Join-Path $releaseDirectory 'SHA256SUMS.txt'
    [System.IO.File]::WriteAllLines($checksumPath, $lines, [System.Text.UTF8Encoding]::new($false))
    Write-Host "Release checksums: $checksumPath"
}

Assert-VersionConsistency -ProjectRoot $root
if ($CheckOnly) {
    return
}
Assert-ReleaseExecutableNotRunning -ProjectRoot $root
Invoke-CheckedNative -Step 'Frontend dependency installation' -Command { npm ci }
& (Join-Path $PSScriptRoot 'Generate-Icons.ps1') | Out-Null
if (-not $?) {
    throw 'Icon generation failed.'
}
Sync-TauriIcons -ProjectRoot $root
Invoke-CheckedNative -Step 'Frontend checks' -Command { npm run check }
Invoke-CheckedNative -Step 'Rust tests' -Command { cargo test }
Invoke-CheckedNative -Step 'Rust lint checks' -Command { cargo clippy --all-targets -- -D warnings }
Invoke-CheckedNative -Step 'Tauri release build' -Command { npm run tauri -- build }
Copy-ReleaseInstaller -ProjectRoot $root
New-PortablePackage -ProjectRoot $root
Write-ReleaseChecksums -ProjectRoot $root
