# Install check for draco-rs: build, decode, encode, and run roundtrip test.
# Ref equivalent: _ref/draco/src/draco/tools/install_test/test.py (builds C++ lib, links test app).
# Usage: .\scripts\install_check.ps1
# Run from workspace root (usd-rs) so paths to crate test data work.

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$crateRoot = Split-Path -Parent $scriptDir
$testDir = Join-Path $crateRoot "test"
$drcIn = Join-Path $testDir "cube_att.obj.edgebreaker.cl4.2.2.drc"
$objOut = Join-Path $testDir "install_check_out.obj"
$drcOut = Join-Path $testDir "install_check_out.drc"

# Run from workspace root so "cargo run -p draco-cli" and paths resolve
$workspaceRoot = Split-Path -Parent (Split-Path -Parent $crateRoot)
Push-Location $workspaceRoot

Write-Host "[1] cargo build --release -p draco-rs -p draco-cli"
cargo build --release -p draco-rs -p draco-cli
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }

if (Test-Path $drcIn) {
    Write-Host "[2] CLI decode: $drcIn -> $objOut"
    cargo run --release -p draco-cli -- decoder -i $drcIn -o $objOut
    if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
    Write-Host "[3] CLI encode: $objOut -> $drcOut"
    cargo run --release -p draco-cli -- encoder -i $objOut -o $drcOut
    if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
    Remove-Item -ErrorAction SilentlyContinue $objOut, $drcOut
} else {
    Write-Host "[2] Skip CLI decode/encode (test file not found: $drcIn)"
}

Write-Host "[4] cargo test -p draco-rs io_roundtrip_draco_mesh"
cargo test -p draco-rs -- io_roundtrip_draco_mesh --nocapture
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }

Write-Host "Install check passed."
Pop-Location
