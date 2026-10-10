$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
cargo build --release
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages[0].version
$msi = "target/fuel-station-$version.msi"
wix build installer/fuel-station.wxs -arch x64 -d Version=$version -d Exe=target/release/fuel-station.exe -o $msi
if ($LASTEXITCODE) { exit $LASTEXITCODE }
Write-Output "built $msi"
