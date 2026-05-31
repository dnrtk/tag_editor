<#
.SYNOPSIS
    Build the Windows x64 release binary (desktop GUI) and the headless Docker
    image (server-only, for the Raspberry Pi / OMV NAS).

.DESCRIPTION
    Step 1: `cargo build --release`            -> target\release\tag_editor.exe
    Step 2: `docker buildx build ... --load`   -> the ARM server image
    Step 3 (optional, -Save): `docker save`    -> a .tar to copy to the NAS

.PARAMETER Platform
    Docker target platform: "linux/arm64" (64-bit Pi/OMV) or "linux/arm/v7"
    (32-bit). Check the NAS with `uname -m` (aarch64 / armv7l).

.PARAMETER Tag
    Docker image tag. Default: tag-editor:latest

.PARAMETER Save
    Also export the built image to a .tar for transferring to the NAS.

.PARAMETER SkipRust
    Skip the Windows x64 release build.

.PARAMETER SkipDocker
    Skip the Docker image build (and save).

.EXAMPLE
    .\build.ps1
.EXAMPLE
    .\build.ps1 -Platform linux/arm/v7 -Save
#>
[CmdletBinding()]
param(
    [ValidateSet("linux/arm64", "linux/arm/v7")]
    [string]$Platform = "linux/arm64",
    [string]$Tag = "tag-editor:latest",
    [switch]$Save,
    [switch]$SkipRust,
    [switch]$SkipDocker
)

# Stop on cmdlet errors; native exe failures are checked explicitly via $LASTEXITCODE.
$ErrorActionPreference = "Stop"
# Run from the repo root (this script's folder) so cargo and `docker build .` resolve.
Set-Location -LiteralPath $PSScriptRoot

function Write-Section($message) {
    Write-Host ""
    Write-Host "==> $message" -ForegroundColor Cyan
}

function Invoke-Step($description, [scriptblock]$action) {
    & $action
    if ($LASTEXITCODE -ne 0) {
        throw "$description failed (exit code $LASTEXITCODE)"
    }
}

$started = Get-Date

try {
    # --- 1. Windows x64 release build (desktop GUI, default features) -----------
    if (-not $SkipRust) {
        Write-Section "cargo build --release  (Windows x64, GUI)"
        Invoke-Step "cargo build --release" { cargo build --release }
        Write-Host "    -> target\release\tag_editor.exe"
    }

    # --- 2. Docker image (headless server build, target ARM) -------------------
    if (-not $SkipDocker) {
        Write-Section "docker buildx build --platform $Platform -t $Tag"
        Invoke-Step "docker buildx build" {
            docker buildx build --platform $Platform -t $Tag --load .
        }

        if ($Save) {
            $tarName = "tag-editor-" + ($Platform -replace "[/]", "-") + ".tar"
            Write-Section "docker save -> $tarName"
            Invoke-Step "docker save" { docker save $Tag -o $tarName }
            Write-Host "    -> $tarName"
            Write-Host "    Copy it to the NAS, then load with:  docker load -i $tarName"
        }
    }

    $elapsed = (Get-Date) - $started
    Write-Section ("Done in {0:N0}s." -f $elapsed.TotalSeconds)
}
catch {
    Write-Host ""
    Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
    # Deterministic non-zero exit so build.bat / CI can detect the failure.
    exit 1
}
