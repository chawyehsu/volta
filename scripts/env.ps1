#!/usr/bin/env pwsh
#Requires -Version 7.0

<#
.SYNOPSIS
    Configure kache for `pixi build`.

.DESCRIPTION
    Dot-source this script before `pixi build`, or run `build.ps1`, which
    sources it:

        . ./scripts/env.ps1
        pixi run build

    `pixi build` runs in an isolated build prefix whose HOME points at the
    throwaway build directory, so kache's default store would be discarded
    with the build. This script exports kache's per-user store and runtime
    directory so the build can pass them into the sandbox through
    `[package.build] secrets`. It also keeps CARGO_HOME on the host and
    removes any SCCACHE* variables, which would make the pixi-build backend
    switch the wrapper back to sccache.

    Cargo-backed tasks such as `pixi run test` are not sandboxed and do not
    need this script. They use kache only when RUSTC_WRAPPER is configured,
    for example in `~/.cargo/config.toml`.
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
