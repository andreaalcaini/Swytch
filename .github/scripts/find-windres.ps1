# Finds windres (MinGW binutils) for build.rs and exports it as WINDRES for the next steps.
# The PATH is left alone, so rustc keeps using the linker of its own GNU toolchain.
$ErrorActionPreference = 'Stop'

$cmd = Get-Command windres -ErrorAction SilentlyContinue
$candidates = @()
if ($cmd) { $candidates += $cmd.Source }
$candidates += 'C:\mingw64\bin\windres.exe', 'C:\msys64\mingw64\bin\windres.exe', 'C:\msys64\ucrt64\bin\windres.exe',
  'C:\ProgramData\mingw64\mingw64\bin\windres.exe'
$found = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $found) {
  Write-Host 'windres not found: installing MinGW with Chocolatey'
  choco install mingw --no-progress -y | Out-Null
  $found = 'C:\ProgramData\mingw64\mingw64\bin\windres.exe'
  if (-not (Test-Path $found)) { throw 'windres is still missing after installing MinGW.' }
}
Write-Host "windres: $found"
"WINDRES=$found" >> $env:GITHUB_ENV
