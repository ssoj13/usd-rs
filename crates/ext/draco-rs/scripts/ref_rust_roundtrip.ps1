# Ref <-> Rust Draco roundtrip using vcpkg C++ encoder/decoder.
# Run from repo root or crates/draco-rs. Uses VCPKG_ROOT or default C:\vcpkg.
# Usage: .\scripts\ref_rust_roundtrip.ps1 [obj_file]
# Example: .\scripts\ref_rust_roundtrip.ps1 test/cube_att.obj

param(
    [string]$InputObj = "test/cube_att.obj"
)

$ErrorActionPreference = "Stop"
$vcpkg = $env:VCPKG_ROOT
if (-not $vcpkg) { $vcpkg = "C:\vcpkg" }
$triplet = "x64-windows"
$toolsDir = Join-Path $vcpkg "installed\$triplet\tools\draco"
$refEncoder = Join-Path $toolsDir "draco_encoder.exe"
$refDecoder = Join-Path $toolsDir "draco_decoder.exe"

foreach ($exe in @($refEncoder, $refDecoder)) {
    if (-not (Test-Path $exe)) {
        Write-Error "vcpkg Draco tool not found: $exe (VCPKG_ROOT=$vcpkg)"
    }
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$crateRoot = Split-Path -Parent $scriptDir
Push-Location $crateRoot

$baseName = [System.IO.Path]::GetFileNameWithoutExtension($InputObj)
$refOutDrc = "test\${baseName}_ref_out.drc"
$refDecodedObj = "test\${baseName}_ref_decoded.obj"
$rustOutDrc = "test\${baseName}_rust_out.drc"
$rustDecodedObj = "test\${baseName}_rust_decoded.obj"

Write-Host "Input: $InputObj"
Write-Host "Ref encoder: $refEncoder"
Write-Host "Ref decoder: $refDecoder"
Write-Host ""

# 1) Ref encode OBJ -> .drc
Write-Host "[1] Ref encode: $InputObj -> $refOutDrc"
& $refEncoder -i $InputObj -o $refOutDrc
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }

# 2) Rust decode ref .drc -> .obj
Write-Host "[2] Rust decode: $refOutDrc -> $refDecodedObj"
cargo run -p draco-cli -- decoder -i $refOutDrc -o $refDecodedObj
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }

# 3) Rust encode OBJ -> .drc
Write-Host "[3] Rust encode: $InputObj -> $rustOutDrc"
cargo run -p draco-cli -- encoder -i $InputObj -o $rustOutDrc
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }

# 4) Ref decode Rust .drc -> .obj
Write-Host "[4] Ref decode: $rustOutDrc -> $rustDecodedObj"
& $refDecoder -i $rustOutDrc -o $rustDecodedObj
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }

Write-Host ""
# Optional: compare .drc file sizes (byte-identical not required; decoded geometry should match)
$refSize = (Get-Item $refOutDrc).Length
$rustSize = (Get-Item $rustOutDrc).Length
Write-Host "Roundtrip OK. Generated:"
Write-Host "  Ref .drc:  $refOutDrc ($refSize bytes)"
Write-Host "  Rust .drc: $rustOutDrc ($rustSize bytes)"
Write-Host "  Ref-decoded:  $refDecodedObj (from ref .drc, decoded by Rust)"
Write-Host "  Rust-decoded: $rustDecodedObj (from Rust .drc, decoded by ref)"
Write-Host "Compare geometry manually or with diff if needed."
Pop-Location
