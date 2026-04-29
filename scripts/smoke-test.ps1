$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
$serviceProcess = $null

Push-Location $repoRoot
try {
    Write-Host "[smoke] Building release workspace..."
    cargo build --release --workspace

    Write-Host "[smoke] Desktop host smoke test..."
    cargo run -p livewall-service -- --desktop-smoke-test

    Write-Host "[smoke] Scene package smoke test..."
    cargo run -p livewall-service -- --scene-smoke-test

    Write-Host "[smoke] Video package smoke test..."
    cargo run -p livewall-service -- --video-smoke-test

    Write-Host "[smoke] IPC round trip over named pipe..."
    $serviceProcess = Start-Process -FilePath "cargo" -ArgumentList @("run", "-p", "livewall-service", "--", "--serve-once") -NoNewWindow -PassThru
    Start-Sleep -Milliseconds 750
    cargo run -p livewall-settings -- --pipe
    Wait-Process -Id $serviceProcess.Id -Timeout 15

    Write-Host "[smoke] PASS"
}
finally {
    if ($null -ne $serviceProcess -and -not $serviceProcess.HasExited) {
        Stop-Process -Id $serviceProcess.Id
    }
    Pop-Location
}
