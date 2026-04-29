$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Copy-Binary {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [Parameter(Mandatory = $true)]
        [string]$Destination
    )

    $candidates = @(
        (Join-Path "target/release" "$Name.exe"),
        (Join-Path "target/release" $Name)
    )

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            Copy-Item -Path $candidate -Destination $Destination -Force
            return
        }
    }

    throw "release binary not found for $Name"
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")

Push-Location $repoRoot
try {
    Write-Host "[package] Building release binaries..."
    cargo build --release -p livewall-service -p livewall-settings

    $distRoot = Join-Path $repoRoot "dist"
    $stagingRoot = Join-Path $distRoot "livewall-v1-preview"
    $archivePath = Join-Path $distRoot "livewall-v1-preview.zip"

    if (Test-Path $stagingRoot) {
        Remove-Item -Path $stagingRoot -Recurse -Force
    }
    if (Test-Path $archivePath) {
        Remove-Item -Path $archivePath -Force
    }

    New-Item -Path $stagingRoot -ItemType Directory | Out-Null
    New-Item -Path (Join-Path $stagingRoot "bin") -ItemType Directory | Out-Null

    Copy-Binary -Name "livewall-service" -Destination (Join-Path $stagingRoot "bin")
    Copy-Binary -Name "livewall-settings" -Destination (Join-Path $stagingRoot "bin")

    Copy-Item -Path "wallpapers/samples" -Destination (Join-Path $stagingRoot "wallpapers") -Recurse -Force
    Copy-Item -Path "README.md" -Destination $stagingRoot -Force
    Copy-Item -Path "docs/architecture.md" -Destination (Join-Path $stagingRoot "architecture.md") -Force

    Compress-Archive -Path (Join-Path $stagingRoot "*") -DestinationPath $archivePath -Force

    Write-Host "[package] Created $archivePath"
}
finally {
    Pop-Location
}
