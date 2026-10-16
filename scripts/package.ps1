[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $projectRoot 'dist'
$portable = Join-Path $dist 'QBS-0.1.0-Windows-x64'

Push-Location $projectRoot
try {
    $cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
    $cargoPath = if ($cargo) { $cargo.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    if (-not (Test-Path -LiteralPath $cargoPath)) {
        throw 'Rust Cargo was not found. Install Rust 1.86+ with the MSVC toolchain.'
    }
    & $cargoPath build --release --locked
    New-Item -ItemType Directory -Path $portable -Force | Out-Null
    Copy-Item 'target\release\qbs.exe', 'target\release\qbs-cli.exe', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_LICENSES.md', 'README.md' -Destination $portable -Force
    Compress-Archive -LiteralPath $portable -DestinationPath (Join-Path $dist 'QBS-0.1.0-Windows-x64.zip') -Force

    $iscc = Get-Command iscc.exe -ErrorAction SilentlyContinue
    $isccCandidates = @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    )
    $isccPath = if ($iscc) { $iscc.Source } else { $isccCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1 }
    if ($isccPath -and (Test-Path -LiteralPath $isccPath)) {
        & $isccPath 'installer\qbs.iss'
    }
    else {
        Write-Warning 'Inno Setup 6 was not found; portable ZIP created, installer skipped.'
    }
}
finally {
    Pop-Location
}

