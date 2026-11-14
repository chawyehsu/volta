#!/usr/bin/env pwsh
#Requires -Version 7.0

<#
.SYNOPSIS
    Configure the kache build cache for local builds.

.DESCRIPTION
    Dot-source this script before running tasks that should use kache:

        . ./scripts/env.ps1

    It points kache's store and runtime directory at the repository root,
    keeps CARGO_HOME on the host so the local registry is reused, and removes
    any SCCACHE* variables. The pixi-build backend force-exports
    RUSTC_WRAPPER="sccache" when any of those are visible, which would
    silently disable kache inside `pixi build`.

    The store lives outside `.pixi` so `pixi build --clean` cannot wipe it.
#>

& {
    $repoRoot = Split-Path -Parent $PSScriptRoot

    $env:KACHE_CACHE_DIR = Join-Path $repoRoot '.kache'
    $env:KACHE_RUNTIME_DIR = Join-Path $repoRoot '.kache-runtime'
    $env:KACHE_LOCAL_ONLY = '1'

    # Reuse the host Cargo cache; the pixi-build sandbox cannot see it unless
    # it is passed through `[package.build] secrets`.
    $env:CARGO_HOME = if ($IsWindows) {
        Join-Path $env:USERPROFILE '.cargo'
    }
    else {
        Join-Path $HOME '.cargo'
    }

    # `pixi-build-rust` force-exports RUSTC_WRAPPER="sccache" whenever any
    # SCCACHE* variable is visible, which would silently disable kache.
    Get-ChildItem Env: |
        Where-Object { $_.Name -like 'SCCACHE*' } |
        ForEach-Object { Remove-Item -Path "Env:$($_.Name)" }
}
