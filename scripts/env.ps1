#!/usr/bin/env pwsh
#Requires -Version 7.0

<#
.SYNOPSIS
    Configure the kache build cache for local builds.

.DESCRIPTION
    Dot-source this script before running tasks that should use kache:

        . ./scripts/env.ps1

    It points kache's store and runtime directory at kache's per-user cache
    directory, so every project on the machine reuses the same compiled
    dependencies, keeps CARGO_HOME on the host so the local registry is
    reused, and removes any SCCACHE* variables. The pixi-build backend
    force-exports RUSTC_WRAPPER="sccache" when any of those are visible,
    which would silently disable kache inside `pixi build`.

    The values still have to be exported: `pixi build` always runs isolated,
    and an unset KACHE_CACHE_DIR would put the store inside the throwaway
    build directory. Preset values win, so CI keeps its workspace-scoped
    store.
#>

& {
    # Same locations kache uses by default.
    $defaultStore = if ($IsWindows) {
        Join-Path $env:LOCALAPPDATA 'kache'
    }
    elseif ($IsMacOS) {
        Join-Path $HOME 'Library/Caches/kache'
    }
    else {
        Join-Path ($env:XDG_CACHE_HOME ? $env:XDG_CACHE_HOME : (Join-Path $HOME '.cache')) 'kache'
    }

    if (-not $env:KACHE_CACHE_DIR) {
        $env:KACHE_CACHE_DIR = $defaultStore
    }
    if (-not $env:KACHE_RUNTIME_DIR) {
        $env:KACHE_RUNTIME_DIR = $env:KACHE_CACHE_DIR
    }

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
