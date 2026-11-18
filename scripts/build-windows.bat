@echo off
rem Builds Swytch for Windows into dist\windows\:
rem   Swytch-<version>-windows-amd64.exe   standalone executable; run it with `install` to install it per user
rem Only Rust (https://rustup.rs) must be installed beforehand: the toolchain pinned in
rem rust-toolchain.toml is installed on first use. windres (MinGW binutils) embeds the version info
rem and the icon; without it the exe is built without them.
setlocal EnableExtensions
cd /d "%~dp0.."

where cargo >nul 2>nul || (echo Rust is required: https://rustup.rs & exit /b 1)
if not defined WINDRES (
  where windres >nul 2>nul || echo windres not found: the exe will have no version info and no icon.
)

for /f "tokens=3" %%v in ('findstr /b /c:"version = " Cargo.toml') do set "VERSION=%%~v"
if not defined VERSION (echo Could not read the version from Cargo.toml. & exit /b 1)

cargo build --release --locked || exit /b 1

if not exist dist\windows mkdir dist\windows
copy /y target\release\Swytch.exe "dist\windows\Swytch-%VERSION%-windows-amd64.exe" >nul || exit /b 1
echo.
echo Done: dist\windows\
dir /b dist\windows
exit /b 0
