param(
    [string]$Python = 'python',
    [string]$OutputDirectory = 'dist/standalone',
    [switch]$SkipChecks
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$portableRoot = Join-Path (Split-Path -Parent $projectRoot) '.toolchain'
$target = 'x86_64-pc-windows-gnu'
$llvm = Get-ChildItem -LiteralPath $portableRoot -Directory -Filter 'llvm-mingw-*-msvcrt-x86_64' |
    Sort-Object Name -Descending | Select-Object -First 1
if (!$llvm) { throw 'A portable LLVM-MinGW MSVCRT toolchain is required in ../.toolchain.' }
$llvmBin = Join-Path $llvm.FullName 'bin'
$env:RUSTUP_HOME = Join-Path $portableRoot 'rustup'
$env:CARGO_HOME = Join-Path $portableRoot 'cargo'
$env:RUSTUP_TOOLCHAIN = "stable-$target"
$env:PATH = "$(Join-Path $env:CARGO_HOME 'bin');$llvmBin;$env:PATH"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $llvmBin 'x86_64-w64-mingw32-clang.exe'
$env:CC = $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER
$env:AR = 'llvm-ar'
$env:RUSTFLAGS = '-C link-self-contained=yes'
$env:TMP = Join-Path $portableRoot 'temp'
$env:TEMP = $env:TMP

function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program exited with $LASTEXITCODE" }
}

Push-Location -LiteralPath $projectRoot
try {
    $outputRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot $OutputDirectory))
    $repoPrefix = $projectRoot.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (!$outputRoot.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Standalone output must be inside the repository.'
    }
    New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
    $noticeRoot = Join-Path $outputRoot 'build-notices'
    Invoke-Checked $Python @('scripts/generate-notices.py', '--target', $target, '--output', $noticeRoot)
    $sysroot = (& rustc --print sysroot).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not locate Rust notices.' }
    $noticeFiles = @(
        (Join-Path $projectRoot 'LICENSE'),
        (Join-Path $projectRoot 'UPSTREAM.json'),
        (Join-Path $noticeRoot 'THIRD_PARTY_NOTICES.txt'),
        (Join-Path $sysroot 'share/doc/rust/COPYRIGHT-library.html'),
        (Join-Path $llvm.FullName 'LICENSE.TXT')
    )
    $mingwNotices = @(Get-ChildItem -LiteralPath (Join-Path $llvm.FullName 'x86_64-w64-mingw32/share/mingw32') -File -Filter 'COPYING*')
    if (!$mingwNotices.Count) { throw 'MinGW runtime license notices were not found.' }
    $noticeFiles += $mingwNotices.FullName
    $noticeParts = foreach ($file in $noticeFiles) {
        "`n===============================================================================`n$(Split-Path -Leaf $file)`n"
        Get-Content -LiteralPath $file -Raw -Encoding utf8
    }
    $env:CODEX_MIGRATE_NOTICES_FILE = Join-Path $noticeRoot 'standalone-notices.txt'
    [IO.File]::WriteAllText($env:CODEX_MIGRATE_NOTICES_FILE, ($noticeParts -join "`n"), [Text.UTF8Encoding]::new($false))
    if (!$SkipChecks) {
        Invoke-Checked 'cargo' @('fmt', '--all', '--', '--check')
        Invoke-Checked 'cargo' @('clippy', '--locked', '--all-targets', '--features', 'standalone', '--', '-D', 'warnings')
        $savedRuntime = [Environment]::GetEnvironmentVariable('CODEX_MIGRATE_CODEX_BIN')
        try {
            $env:CODEX_MIGRATE_CODEX_BIN = ''
            Invoke-Checked 'cargo' @('test', '--locked', '--all-targets', '--features', 'standalone')
        } finally {
            [Environment]::SetEnvironmentVariable('CODEX_MIGRATE_CODEX_BIN', $savedRuntime)
        }
    }
    Invoke-Checked 'cargo' @('build', '--locked', '--release', '--features', 'standalone', '--bins')
    $manifest = Get-Content -Raw -LiteralPath 'Cargo.toml'
    $version = [regex]::Match($manifest, '(?m)^version = "([^"]+)"').Groups[1].Value
    $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $projectRoot 'target' }
    $binary = Join-Path $targetRoot 'release/codex-migrate-gui.exe'
    $imports = & (Join-Path $llvmBin 'llvm-readobj.exe') --coff-imports $binary
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect EXE dependencies.' }
    $dlls = @($imports | Select-String '^\s+Name: (.+)$' |
        ForEach-Object { $_.Matches[0].Groups[1].Value.ToLowerInvariant() } | Sort-Object -Unique)
    $windowsDlls = @('advapi32.dll','bcryptprimitives.dll','dwmapi.dll','gdi32.dll','imm32.dll',
        'kernel32.dll','msvcrt.dll','ntdll.dll','ole32.dll','opengl32.dll','shell32.dll',
        'shlwapi.dll','user32.dll','uxtheme.dll')
    $unexpected = @($dlls | Where-Object { $_ -notin $windowsDlls -and $_ -notlike 'api-ms-win-*.dll' })
    if (!$dlls.Count -or $unexpected.Count) { throw "Unexpected EXE dependencies: $unexpected" }
    $exeName = "Codex-Migrate-Current-$version-Standalone-Windows-x64.exe"
    $output = Join-Path $outputRoot $exeName
    Copy-Item -LiteralPath $binary -Destination $output
    $checksum = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
    "$checksum  $exeName" | Set-Content -LiteralPath ($output + '.sha256') -Encoding ascii
    $commit = (& git rev-parse HEAD).Trim()
    $dirty = [bool](& git status --porcelain)
    @{ version=$version; edition='standalone'; target=$target; commit=$commit; source_dirty=$dirty;
        rust=(& rustc --version).Trim(); sha256=$checksum; dll_imports=$dlls;
        embedded_notices_sha256=(Get-FileHash -LiteralPath $env:CODEX_MIGRATE_NOTICES_FILE -Algorithm SHA256).Hash.ToLowerInvariant() } |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $outputRoot 'BUILD-INFO.json') -Encoding utf8
    Write-Output "Standalone executable: $output"
} finally { Pop-Location }
