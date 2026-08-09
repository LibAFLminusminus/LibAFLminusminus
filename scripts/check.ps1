#!/usr/bin/env pwsh

$ErrorActionPreference = "Stop"

$LibaflmmDir = Split-Path -Parent $PSScriptRoot
Set-Location $LibaflmmDir

$env:RUST_BACKTRACE = "full"

$ClippyCmd = @("clippy", "--no-deps", "--tests", "--examples", "--benches")
$DocCmd = @("doc", "--no-deps")

$RustcFlags = @()
if ($env:RUSTC_FLAGS) {
    $RustcFlags = @($env:RUSTC_FLAGS -split "\s+" | Where-Object { $_ })
}

function Invoke-Cargo {
    param (
        [string]$Dir,
        [string[]]$CargoArgs
    )
    Write-Host "cargo $($CargoArgs -join ' ')"
    Push-Location $Dir

    try {
        cargo @CargoArgs

        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }
    finally {
        Pop-Location
    }
}

function Invoke-Clippy {
    param (
        [string]$Dir,
        [string[]]$Features
    )
    Write-Host "Running Clippy on $Dir"
    Invoke-Cargo -Dir $Dir -CargoArgs ($ClippyCmd + $Features + "--" + $RustcFlags)
}

function Invoke-Doc {
    param (
        [string]$Dir,
        [string[]]$Features
    )
    Write-Host "Building docs for $Dir"
    Invoke-Cargo -Dir $Dir -CargoArgs ($DocCmd + $Features)
}

$AllProjects = @(
    "crates/libaflmm",
    "crates/libaflmm_bolts",
    "crates/libaflmm_cc",
    "crates/libaflmm_frida"
)

$NoAllFeatures = @(
    "crates/libaflmm_qemu"
)

if ($args.Count -eq 0) {
    $Projects = $AllProjects
}
else {
    $Projects = $args[0] -split ","
}

foreach ($project in $Projects) {
    $project = $project.Trim()
    if ($NoAllFeatures -contains $project) {
        $features = @("--features=clippy")
    }
    else {
        $features = @("--all-features")
    }
    if (Test-Path -LiteralPath $project -PathType Container) {
        Invoke-Clippy -Dir $project -Features $features
        Invoke-Doc -Dir $project -Features $features
    }
    else {
        Write-Host "Warning: Directory $project does not exist. Skipping."
    }
}

$WorkspaceArgs = @("--workspace", "--exclude", "generics_reorder")

Invoke-Cargo -Dir $LibaflmmDir -CargoArgs ($ClippyCmd + $WorkspaceArgs + "--" + $RustcFlags)
Invoke-Cargo -Dir $LibaflmmDir -CargoArgs ($DocCmd + $WorkspaceArgs)

Write-Host "Good to go"
