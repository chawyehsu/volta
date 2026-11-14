#!/usr/bin/env pwsh
#Requires -Version 7.0

<#
.SYNOPSIS
    Build Volta and package release artifacts.

.DESCRIPTION
    Builds Volta with pixi for the requested platform and packages the
    resulting binaries into the release archives expected by the release
    workflow. This script is used by .github/workflows/build.yml but can be
    run locally on any supported platform.

    Extraction of the conda package produced by `pixi build` is delegated to
    `rattler-build package extract`.

    The build temporarily mutates `pixi.toml` (cargo profile override, macOS
    SDK path); the original manifest is always restored afterwards.

.PARAMETER Platform
    The build target to produce. One of: linux, linux-aarch64, macos, macos-cross,
    windows. Defaults to the platform matching the current host.

    `macos` builds both macOS architectures natively on a macOS host (for local
    use); `macos-cross` cross-builds them from a Linux host (used by CI, which
    is faster than spinning up macOS runners).

.PARAMETER BuildType
    The artifact type: "release" or "nightly". Nightly builds use the "ci"
    cargo profile. Defaults to "release".

.PARAMETER OutputDir
    Directory the final packaged artifacts are written to. Defaults to "out".
    Intermediate files (conda packages, staging trees, WiX objects) are written
    to ".pixi/output" instead.
#>
[CmdletBinding()]
param(
    [ValidateSet('linux', 'linux-aarch64', 'macos', 'macos-cross', 'windows')]
    [string] $Platform,

    [ValidateSet('release', 'nightly')]
    [string] $BuildType = 'release',

    [string] $OutputDir = 'out'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# The script reads/mutates pixi.toml and Cargo.toml relative to the repo root,
# and [System.IO.Path]::GetFullPath does not track PowerShell's location, so
# anchor to the script directory and resolve the output dir once.
Set-Location -LiteralPath $PSScriptRoot
$OutputDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDir)
# Intermediate build outputs (conda packages, staging trees, WiX objects) live
# under .pixi so that $OutputDir contains only uploadable release artifacts.
$PackageDir = Join-Path $PSScriptRoot '.pixi/output'

# Point kache's store and runtime directory at the repo root and keep
# CARGO_HOME on the host. The pixi-build sandbox receives these through
# `[package.build] secrets`.
. (Join-Path $PSScriptRoot 'scripts/env.ps1')

function Invoke-Checked {
    param(
        [Parameter(Mandatory)][string] $Command,
        [string[]] $Arguments = @()
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Command $($Arguments -join ' ')"
    }
}

function Invoke-Pixi {
    param([string[]] $Arguments = @())

    Invoke-Checked -Command 'pixi' -Arguments $Arguments
}

function Invoke-RattlerBuild {
    param([string[]] $Arguments = @())

    Invoke-Pixi -Arguments (@('run', '-e', 'build', 'rattler-build') + $Arguments)
}

function Get-DefaultPlatform {
    if ($IsWindows) { return 'windows' }
    if ($IsMacOS) { return 'macos' }
    if ($IsLinux -and [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq 'Arm64') {
        return 'linux-aarch64'
    }
    if ($IsLinux) { return 'linux' }
    throw 'Unsupported host platform.'
}

function Get-VoltaVersion {
    $match = Select-String -Path 'Cargo.toml' -Pattern '^version\s*=\s*"(.*)"' | Select-Object -First 1
    if (-not $match) {
        throw 'Unable to determine Volta version from Cargo.toml.'
    }
    return $match.Matches.Groups[1].Value
}

function Update-PixiManifest {
    param(
        [Parameter(Mandatory)][scriptblock] $Transform
    )

    $content = Get-Content -Path 'pixi.toml' -Raw
    Set-Content -Path 'pixi.toml' -Value (& $Transform $content) -NoNewline
}

function Set-CargoProfile {
    # The default profile is "release", which is what pixi.toml already
    # specifies. Non-release builds swap it for the "ci" profile.
    if ($BuildType -eq 'release') {
        return
    }

    Update-PixiManifest { param($content) $content -replace '"--profile", "release"', '"--profile", "ci"' }
}

function Set-MacOSSDKPath {
    param([Parameter(Mandatory)][string] $SdkRoot)

    Update-PixiManifest { param($content) $content.Replace('__MACOS_SDK_ROOT__', $SdkRoot) }
}

function Install-MacOSSDK {
    # Cross-compiling from Linux: conda-forge cannot ship the macOS SDK, so fetch
    # it explicitly. Keep in sync with c_stdlib_version in pixi.toml.
    $sdkVersion = '11.0'
    $sdkDir = Join-Path (Get-Location) '.pixi/macos-sdk'
    $archive = Join-Path $sdkDir "MacOSX${sdkVersion}.sdk.tar.xz"

    New-Item -ItemType Directory -Path $sdkDir -Force | Out-Null
    Invoke-Checked -Command 'curl' -Arguments @(
        '-fsSL', '--retry', '3', '--retry-delay', '5', '-o', $archive,
        "https://github.com/phracker/MacOSX-SDKs/releases/download/11.3/MacOSX${sdkVersion}.sdk.tar.xz"
    )

    $expected = 'd3feee3ef9c6016b526e1901013f264467bb927865a03422a9cb925991cc9783'
    $actual = (Get-FileHash -Path $archive -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) {
        throw "SHA256 mismatch for macOS SDK: expected $expected, got $actual"
    }

    Invoke-Checked -Command 'tar' -Arguments @('-xf', $archive, '-C', $sdkDir)
    Remove-Item -Path $archive -Force

    $sdkRoot = Join-Path $sdkDir "MacOSX${sdkVersion}.sdk"
    Set-MacOSSDKPath -SdkRoot $sdkRoot
}

function Install-NativeMacOSSDK {
    # Native builds use the SDK bundled with the host Xcode toolchain.
    if (-not $IsMacOS) {
        throw "The 'macos' platform requires a macOS host; use 'macos-cross' to cross-build from Linux."
    }

    $sdkRoot = @(& xcrun --show-sdk-path 2>$null) | Select-Object -First 1
    if ($LASTEXITCODE -ne 0 -or -not $sdkRoot) {
        throw 'Unable to determine the macOS SDK path via xcrun.'
    }

    Set-MacOSSDKPath -SdkRoot $sdkRoot.Trim()
}

function Build-CondaPackage {
    param([Parameter(Mandatory)][string] $TargetPlatform)

    if ($Platform -eq 'macos-cross') {
        Invoke-Pixi -Arguments @('run', 'build', '--build-platform', 'linux-64', '--target-platform', $TargetPlatform)
    }
    else {
        Invoke-Pixi -Arguments @('run', 'build', '--target-platform', $TargetPlatform)
    }

    $package = Get-ChildItem -Path . -Filter 'volta*.conda' -File | Select-Object -First 1
    if (-not $package) {
        throw "No conda package produced for $TargetPlatform."
    }

    # Normalize the package name so callers get a predictable path.
    $dest = Join-Path $PackageDir "volta-$TargetPlatform.conda"
    Move-Item -Path $package.FullName -Destination $dest -Force
    return $dest
}

function Expand-CondaPackage {
    param(
        [Parameter(Mandatory)][string] $PackageFile,
        [Parameter(Mandatory)][string] $Destination
    )

    if (Test-Path -Path $Destination) {
        Remove-Item -Path $Destination -Recurse -Force
    }
    Invoke-RattlerBuild -Arguments @('package', 'extract', $PackageFile, '--dest', $Destination)
}

function New-UnixArchive {
    param(
        [Parameter(Mandatory)][string] $PackageFile,
        [Parameter(Mandatory)][string] $ArtifactName
    )

    $staging = Join-Path $PackageDir "staging-$ArtifactName"
    Expand-CondaPackage -PackageFile $PackageFile -Destination $staging

    $binDir = Join-Path $staging 'bin'
    $archive = Join-Path $OutputDir $ArtifactName
    Push-Location $binDir
    try {
        $files = Get-ChildItem -Filter 'volta*' -File | ForEach-Object { $_.Name }
        Invoke-Checked -Command 'tar' -Arguments (@('-zcvf', $archive) + $files)
    }
    finally {
        Pop-Location
    }

    Write-LinuxAbiReport -Path (Join-Path $binDir 'volta')
    Remove-Item -Path $staging -Recurse -Force
}

function Write-LinuxAbiReport {
    param([Parameter(Mandatory)][string] $Path)

    # Only the Linux targets produce ELF binaries; skip for the macOS targets
    # that are cross-built on a Linux host.
    if ($Platform -ne 'linux' -and $Platform -ne 'linux-aarch64') {
        return
    }

    & ldd -v $Path

    $glibc = & objdump -T $Path | Select-String 'GLIBC' |
        ForEach-Object { if ($_.Line -match 'GLIBC_([.0-9]+)') { $Matches[1] } } |
        Sort-Object -Unique
    Write-Host ($glibc -join "`n")
}

function New-WindowsArtifacts {
    param(
        [Parameter(Mandatory)][string] $PackageFile,
        [Parameter(Mandatory)][string] $Arch,
        [Parameter(Mandatory)][string] $Version
    )

    $zipName = "volta-$Arch-pc-windows-msvc.zip"
    $msiName = "volta-$Arch-pc-windows-msvc.msi"

    $staging = Join-Path $PackageDir "staging-$Arch"
    Expand-CondaPackage -PackageFile $PackageFile -Destination $staging

    $binDir = Join-Path $staging 'bin'
    Get-ChildItem -Path $binDir -Filter 'volta*.exe' -File |
        Compress-Archive -DestinationPath (Join-Path $OutputDir $zipName) -Force

    $wixObject = Join-Path $PackageDir "volta-$Arch.wixobj"
    Invoke-Checked -Command 'candle' -Arguments @(
        'wix/main.wxs', "-dVersion=$Version", "-dCargoTargetBinDir=$binDir", '-o', $wixObject
    )
    Invoke-Checked -Command 'light' -Arguments @(
        $wixObject, '-spdb', '-ext', 'WixUIExtension', '-ext', 'WixUtilExtension', '-o', (Join-Path $OutputDir $msiName)
    )

    Remove-Item -Path $staging -Recurse -Force
}

function Get-WindowsBuildDirectory {
    param([Parameter(Mandatory)][string] $RepoRoot)

    # The short-path junction works around MAX_PATH limits on GitHub Actions
    # Windows runners. Only use it in CI: local checkouts build in place, so a
    # stale C:\w from another checkout can never be picked up.
    if ($env:GITHUB_ACTIONS -ne 'true') {
        return $RepoRoot
    }

    $junction = 'C:\w'
    $repoRoot = (Get-Item -LiteralPath $RepoRoot).FullName.TrimEnd('\', '/')

    if (Test-Path -LiteralPath $junction) {
        $item = Get-Item -LiteralPath $junction -Force
        $targetProp = $item.PSObject.Properties['Target']
        $target = if ($targetProp) { @($targetProp.Value) | Select-Object -First 1 } else { $item.LinkTarget }
        if (-not $target) {
            throw "The path '$junction' already exists but is not a junction to '$repoRoot'. Remove it and retry."
        }
        $target = [System.IO.Path]::GetFullPath($target).TrimEnd('\', '/')
        if ($target -ine $repoRoot) {
            throw "The existing junction '$junction' points to '$target', not '$repoRoot'. Remove it and retry."
        }
    }
    else {
        New-Item -ItemType Junction -Path $junction -Target $repoRoot | Out-Null
    }

    return $junction
}

function New-Checksums {
    $patterns = if ($Platform -eq 'windows') { @('volta-*.zip', 'volta-*.msi') } else { @('volta-*.tar.gz') }

    foreach ($pattern in $patterns) {
        Get-ChildItem -Path $OutputDir -Filter $pattern -File | ForEach-Object {
            $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
            "$hash  $($_.Name)" | Set-Content -Path "$($_.FullName).sha256"
        }
    }
}

# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------

$Platform = if ($Platform) { $Platform } else { Get-DefaultPlatform }

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
New-Item -ItemType Directory -Path $PackageDir -Force | Out-Null
$version = Get-VoltaVersion

# The build mutates pixi.toml (cargo profile override, macOS SDK path); always
# restore the original manifest, even on failure, so local runs leave no diff.
$manifest = Get-Content -Path 'pixi.toml' -Raw
try {
    Set-CargoProfile

    switch ($Platform) {
        'linux' {
            $package = Build-CondaPackage -TargetPlatform 'linux-64'
            New-UnixArchive -PackageFile $package -ArtifactName 'volta-x86_64-unknown-linux-gnu.tar.gz'
        }
        'linux-aarch64' {
            $package = Build-CondaPackage -TargetPlatform 'linux-aarch64'
            New-UnixArchive -PackageFile $package -ArtifactName 'volta-aarch64-unknown-linux-gnu.tar.gz'
        }
        'macos' {
            Install-NativeMacOSSDK

            $arm = Build-CondaPackage -TargetPlatform 'osx-arm64'
            New-UnixArchive -PackageFile $arm -ArtifactName 'volta-aarch64-apple-darwin.tar.gz'

            $x64 = Build-CondaPackage -TargetPlatform 'osx-64'
            New-UnixArchive -PackageFile $x64 -ArtifactName 'volta-x86_64-apple-darwin.tar.gz'
        }
        'macos-cross' {
            Install-MacOSSDK

            $arm = Build-CondaPackage -TargetPlatform 'osx-arm64'
            New-UnixArchive -PackageFile $arm -ArtifactName 'volta-aarch64-apple-darwin.tar.gz'

            $x64 = Build-CondaPackage -TargetPlatform 'osx-64'
            New-UnixArchive -PackageFile $x64 -ArtifactName 'volta-x86_64-apple-darwin.tar.gz'
        }
        'windows' {
            $workingDir = Get-WindowsBuildDirectory -RepoRoot $PWD.Path
            Push-Location -LiteralPath $workingDir
            try {
                # Prime the test environment before the first pixi build; the
                # Windows build fails otherwise.
                Invoke-Pixi -Arguments @('run', '-e', 'test', 'cargo', '--version')

                $x64 = Build-CondaPackage -TargetPlatform 'win-64'
                New-WindowsArtifacts -PackageFile $x64 -Arch 'x86_64' -Version $version

                $arm = Build-CondaPackage -TargetPlatform 'win-arm64'
                New-WindowsArtifacts -PackageFile $arm -Arch 'aarch64' -Version $version
            }
            finally {
                Pop-Location
            }
        }
    }
}
finally {
    Set-Content -Path 'pixi.toml' -Value $manifest -NoNewline
}

New-Checksums

Write-Host "Build and packaging completed for $Platform ($BuildType)."
