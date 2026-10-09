param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$NoticeDirectory = 'dist/notices',
    [string]$OutputDirectory = 'dist/release'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    if ($Target -ne 'x86_64-pc-windows-msvc') { throw 'Official packages use Windows x64 MSVC.' }
    $manifest = Get-Content -Raw -LiteralPath 'Cargo.toml'
    $versionMatch = [regex]::Match($manifest, '(?m)^version = "([^"]+)"')
    if (!$versionMatch.Success) { throw 'Cargo package version was not found.' }
    $version = $versionMatch.Groups[1].Value
    $archiveName = "Codex-Migrate-Current-$version-Windows-x64.zip"
    $outputRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot $OutputDirectory))
    $workspacePrefix = $projectRoot.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (!$outputRoot.StartsWith($workspacePrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Package output must be inside the repository.'
    }
    New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
    $packageRoot = Join-Path $outputRoot ('package-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $packageRoot | Out-Null
    foreach ($exe in @('codex-migrate.exe', 'codex-migrate-gui.exe')) {
        Copy-Item -LiteralPath "target/$Target/release/$exe" -Destination $packageRoot
    }
    foreach ($document in @('LICENSE', 'README.md', 'README.zh-CN.md', 'CURRENT-VERSION.md', 'CHANGELOG.md', 'CONTRIBUTING.md', 'SECURITY.md', 'TRADEMARKS.md', 'THIRD_PARTY_NOTICES.md', 'UPSTREAM.json')) {
        Copy-Item -LiteralPath $document -Destination $packageRoot
    }
    foreach ($notice in @('THIRD_PARTY_NOTICES.txt', 'THIRD_PARTY_COMPONENTS.json')) {
        Copy-Item -LiteralPath (Join-Path $NoticeDirectory $notice) -Destination $packageRoot
    }
    New-Item -ItemType Directory -Path (Join-Path $packageRoot 'docs') | Out-Null
    foreach ($document in @('current-validation.md', 'current-runtime-results.json', 'publishing.md')) {
        Copy-Item -LiteralPath "docs/$document" -Destination (Join-Path $packageRoot 'docs')
    }
    Copy-Item -LiteralPath 'docs/third-party' -Destination (Join-Path $packageRoot 'docs') -Recurse
    $toolchainRoot = (& rustc --print sysroot).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not locate the Rust toolchain notices.' }
    $rustNoticeRoot = Join-Path $packageRoot 'rust-toolchain-notices'
    New-Item -ItemType Directory -Path $rustNoticeRoot | Out-Null
    $rustNotices = Get-ChildItem -LiteralPath (Join-Path $toolchainRoot 'share/doc/rust') -File |
        Where-Object { $_.Name -match '^(COPYRIGHT|LICENSE)' }
    if (!$rustNotices) { throw 'Rust toolchain license texts were not found.' }
    $rustNotices | Copy-Item -Destination $rustNoticeRoot
    $commit = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not record the package commit.' }
    @{ version = $version; commit = $commit; target = $Target; rust = (& rustc --version).Trim() } |
        ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath (Join-Path $packageRoot 'BUILD-INFO.json')
    $hashLines = foreach ($exe in @('codex-migrate.exe', 'codex-migrate-gui.exe')) {
        (Get-FileHash -LiteralPath (Join-Path $packageRoot $exe) -Algorithm SHA256).Hash.ToLower() + "  $exe"
    }
    $hashLines | Set-Content -Encoding ascii -LiteralPath (Join-Path $packageRoot 'SHA256SUMS.txt')
    $archivePath = Join-Path $outputRoot $archiveName
    Compress-Archive -Path (Join-Path $packageRoot '*') -DestinationPath $archivePath -Force
    ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLower() + "  $archiveName") |
        Set-Content -Encoding ascii -LiteralPath ($archivePath + '.sha256')
    Write-Output $archivePath
    Write-Output ($archivePath + '.sha256')
} finally { Pop-Location }
