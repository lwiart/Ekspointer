# Construit la version publiable d'Ekspointer : tests, exécutable portable et programme d'installation.
#
#   powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
#
# Prérequis : Rust (rustup), Build Tools Visual Studio avec les outils C++, Inno Setup 6.
# Les tests de l'exécutable lancent leurs propres instances, isolées de tout Ekspointer déjà ouvert.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$cargo = Join-Path $PSScriptRoot 'cargo.cmd'

function Invoke-Step([string]$Title, [scriptblock]$Action) {
    Write-Host "== $Title" -ForegroundColor Cyan
    & $Action
    if ($LASTEXITCODE -ne 0) { throw "Échec : $Title" }
}

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Ekspointer $version"

Invoke-Step 'Tests unitaires et tests d''intégration de la logique' { & $cargo test }
Invoke-Step 'Tests d''intégration de l''exécutable' { & $cargo test --test process -- --ignored --test-threads=1 }
Invoke-Step 'Compilation de la version release' { & $cargo build --release }

$target = (& $cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
$exe = Join-Path $target 'release\ekspointer.exe'
$sizeMb = (Get-Item $exe).Length / 1MB
if ($sizeMb -ge 6) { throw ("L'exécutable pèse {0:N2} Mo : la limite est de 6 Mo." -f $sizeMb) }
Write-Host ("Exécutable : {0:N2} Mo" -f $sizeMb)

$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$portable = Join-Path $dist "Ekspointer-$version-portable.exe"
Copy-Item $exe $portable -Force

$iscc = @(
    (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
    (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) { throw "Inno Setup 6 est introuvable. Installez-le : winget install JRSoftware.InnoSetup" }

Invoke-Step 'Programme d''installation' { & $iscc /Q "/DAppVersion=$version" "/DExePath=$exe" (Join-Path $root 'installer\ekspointer.iss') }

Write-Host ''
Write-Host 'Fichiers produits :' -ForegroundColor Green
Get-ChildItem $dist -Filter "Ekspointer-*$version*" | ForEach-Object { '  {0}  ({1:N2} Mo)' -f $_.FullName, ($_.Length / 1MB) }
