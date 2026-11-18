# Sets the app version: Cargo.toml is the only place it is written; Cargo.lock follows it.
# build.rs stamps it into the exe's version info, src/main.rs exposes it as VERSION.
# Usage: make version V=x.y.z   (or: powershell -File scripts/set-version.ps1 x.y.z)
param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "The version must be x.y.z, got '$Version'." }
Set-Location (Join-Path $PSScriptRoot '..')

$path = (Resolve-Path 'Cargo.toml').Path
$toml = [IO.File]::ReadAllText($path)
$re = [regex]'(?m)^version = "[^"]*"'
if (-not $re.IsMatch($toml)) { throw 'No version line found in Cargo.toml.' }
[IO.File]::WriteAllText($path, $re.Replace($toml, "version = ""$Version""", 1)) # UTF-8 without BOM

cargo update --workspace --offline
if ($LASTEXITCODE -ne 0) { throw 'cargo could not update Cargo.lock.' }
Write-Host "Version set to $Version (Cargo.toml, Cargo.lock). Commit it, then tag v$Version to release."
