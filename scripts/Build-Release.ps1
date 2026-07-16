$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root

& (Join-Path $PSScriptRoot 'Generate-Icons.ps1') | Out-Null
if (-not $?) {
    throw 'Icon generation failed.'
}

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

$env:PYTHONIOENCODING = 'utf-8'
Invoke-CheckedNative -Step 'Default sound generation' -Command { python (Join-Path $PSScriptRoot 'generate_default_sound.py') | Out-Null }
Invoke-CheckedNative -Step 'Frontend dependency installation' -Command { npm ci }
Invoke-CheckedNative -Step 'Frontend checks' -Command { npm run check }
Invoke-CheckedNative -Step 'Rust tests' -Command { cargo test }
Invoke-CheckedNative -Step 'Rust lint checks' -Command { cargo clippy --all-targets -- -D warnings }
Invoke-CheckedNative -Step 'Tauri release build' -Command { npm run tauri -- build }
Copy-ReleaseInstaller -ProjectRoot $root
New-PortablePackage -ProjectRoot $root
