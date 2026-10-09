param([ValidateSet('Test','Build','Release','Fmt','Check')][string]$Mode = 'Test', [switch]$Gui)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$portableRoot = Join-Path (Split-Path -Parent $projectRoot) '.toolchain'
if (Test-Path (Join-Path $portableRoot 'cargo/bin/cargo.exe')) {
    $env:RUSTUP_HOME = Join-Path $portableRoot 'rustup'
    $env:CARGO_HOME = Join-Path $portableRoot 'cargo'
    $llvm = Get-ChildItem -LiteralPath $portableRoot -Directory -Filter 'llvm-mingw-*-msvcrt-x86_64' | Sort-Object Name -Descending | Select-Object -First 1
    if (!$llvm) { throw 'Portable LLVM-MinGW was not found.' }
    $llvmBin = Join-Path $llvm.FullName 'bin'
    $env:PATH = "$(Join-Path $env:CARGO_HOME 'bin');$llvmBin;$env:PATH"
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $llvmBin 'x86_64-w64-mingw32-clang.exe'
    $env:CC = $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER
    $env:AR = 'llvm-ar'
    $env:RUSTFLAGS = '-C link-self-contained=yes'
    $env:TMP = Join-Path $portableRoot 'temp'
    $env:TEMP = $env:TMP
}
Push-Location -LiteralPath $projectRoot
try {
    [string[]]$cargoArgs = switch ($Mode) {
        'Test' { @('test') }
        'Build' { @('build') }
        'Release' { @('build','--release') }
        'Fmt' { @('fmt','--all','--','--check') }
        'Check' { @('clippy','--all-targets') }
    }
    if ($Gui -and $Mode -ne 'Fmt') { $cargoArgs += @('--features','gui') }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo exited with $LASTEXITCODE" }
} finally { Pop-Location }
