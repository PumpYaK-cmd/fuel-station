$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
cargo build --release
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$gui = Get-CimInstance Win32_Process -Filter "Name = 'fuel-station.exe'" |
    Where-Object { $_.CommandLine -notmatch '\smcp(\s|$)' }
foreach ($p in $gui) {
    Stop-Process -Id $p.ProcessId -Force
    Wait-Process -Id $p.ProcessId -ErrorAction SilentlyContinue
}

# Piped so pwsh waits for the GUI-subsystem exe and sets $LASTEXITCODE.
target/release/fuel-station.exe install | Write-Output
if ($LASTEXITCODE) { exit $LASTEXITCODE }

if ($gui) {
    $app = "$env:LOCALAPPDATA\FuelStation\app\fuel-station.exe"
    $exe = if (Test-Path $app) { $app } else { "$env:LOCALAPPDATA\FuelStation\bin\fuel-station.exe" }
    # Through explorer so a run from a Claude session doesn't start the GUI inside its MSIX container.
    Start-Process explorer.exe -ArgumentList "`"$exe`""
}
