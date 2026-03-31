# Run cargo fuzz with MSVC env (vcv-rs submodule).
# On Windows by default: build without sanitizers and run harness on corpus (avoids
# sanitizer_cov_trace_pc_guard_cleanup / ASan DLL ABI mismatch). Set DRACO_FUZZ_USE_CARGO_FUZZ=1
# to use "cargo fuzz run" (requires matching ASan/compiler-rt).
# Usage: .\scripts\fuzz_run.ps1 [target] [-- fuzz-args...]
# Example: .\scripts\fuzz_run.ps1 mesh_decode -- -runs=100

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$crateRoot = Split-Path -Parent $scriptDir
$workspaceRoot = (Resolve-Path (Join-Path $crateRoot "..\..")).Path

# Apply MSVC environment via vcv-rs from workspace submodule crates/vcv-rs
if ($IsWindows -ne $false) {
    $vcvCrate = Join-Path $workspaceRoot "crates\vcv-rs"
    if (Test-Path (Join-Path $vcvCrate "Cargo.toml")) {
        $psEnv = & cargo run -p vcv-rs --release --manifest-path (Join-Path $workspaceRoot "Cargo.toml") -- -q -f ps 2>$null
        if ($LASTEXITCODE -eq 0 -and $psEnv) {
            $psEnv | Invoke-Expression
            Write-Host "MSVC env applied (vcv-rs from crates/vcv-rs)."
        }
    }
}

$target = "mesh_decode"
$extra = @()
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq "--") {
        $extra = $args[($i + 1)..($args.Count - 1)]
        break
    }
    if ($i -eq 0 -and $args[0] -notmatch "^-") { $target = $args[0] }
}

$useCargoFuzz = ($env:DRACO_FUZZ_USE_CARGO_FUZZ -eq "1")

if (($IsWindows -ne $false) -and -not $useCargoFuzz) {
    # Build standalone corpus runner (no libFuzzer entry point) and run on corpus dir.
    $corpusBin = "${target}_corpus"
    Push-Location $workspaceRoot
    Write-Host "cargo build --release -p draco-rs-fuzz --bin $corpusBin"
    & cargo build --release -p draco-rs-fuzz --bin $corpusBin
    if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
    $exe = Join-Path $workspaceRoot "target\release\$corpusBin.exe"
    if (-not (Test-Path $exe)) { $exe = Join-Path $workspaceRoot "target\x86_64-pc-windows-msvc\release\$corpusBin.exe" }
    if (-not (Test-Path $exe)) { Write-Host "Error: $corpusBin.exe not found after build."; Pop-Location; exit 1 }
    $corpus = Join-Path $crateRoot "fuzz\corpus\$target"
    New-Item -ItemType Directory -Force -Path $corpus | Out-Null
    Write-Host "$exe $corpus $($extra -join ' ')"
    & $exe $corpus @extra
    $exitCode = $LASTEXITCODE
    Pop-Location
    exit $exitCode
}

# Add ASan DLL to PATH only when using cargo fuzz on Windows
if ($IsWindows -ne $false -and $useCargoFuzz) {
    $asanDll = "clang_rt.asan_dynamic-x86_64.dll"
    $asanDir = $env:ASAN_DLL_DIR
    if (-not $asanDir) {
        $sysroot = & rustc +nightly --print sysroot 2>$null
        if ($sysroot) {
            $try = Join-Path $sysroot "lib\rustlib\x86_64-pc-windows-msvc\lib"
            if (Test-Path (Join-Path $try $asanDll)) { $asanDir = $try }
        }
    }
    if (-not $asanDir) {
        $llvmBase = "C:\Program Files\LLVM\lib\clang"
        if (Test-Path $llvmBase) {
            $verDirs = Get-ChildItem -Path $llvmBase -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending
            foreach ($v in $verDirs) {
                $try = Join-Path $v.FullName "lib\windows"
                if (Test-Path (Join-Path $try $asanDll)) { $asanDir = $try; break }
            }
        }
    }
    if ($asanDir -and (Test-Path (Join-Path $asanDir $asanDll))) {
        $env:PATH = $asanDir + [System.IO.Path]::PathSeparator + $env:PATH
        Write-Host "Using ASan runtime: $asanDir"
    } else {
        Write-Host "Note: DRACO_FUZZ_USE_CARGO_FUZZ=1 but $asanDll not found; you may get DLL/entry point errors."
    }
}

Push-Location $crateRoot
Write-Host "cargo fuzz run $target $($extra -join ' ')"
& cargo fuzz run $target @extra
$exitCode = $LASTEXITCODE
Pop-Location
exit $exitCode
