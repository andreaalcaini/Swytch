# Recipes must work in both sh and cmd.exe: only `cd dir && cmd`, no inline VAR=x, no rm/cp/mkdir -p.
# On this machine GNU make is `mingw32-make` (WinLibs); `make` works the same where it is installed.

.DEFAULT_GOAL := help
.PHONY: help dev build install package test fmt fmt-check clippy check icon icon-preview version

help:
	@echo make dev              build and run the app, debug build
	@echo make build            release build into target/release/Swytch.exe
	@echo make install          release build, then install it for this user: Swytch.exe install
	@echo make package          release build into dist/windows/Swytch-VERSION-windows-amd64.exe
	@echo make test             unit tests
	@echo make fmt              format the code
	@echo make check            rustfmt + clippy with zero warnings + tests: must pass before done
	@echo make icon             regenerate build/windows/icon.ico from the icon renderer
	@echo make icon-preview     write icon-preview.bmp: the icon at several sizes, light and dark
	@echo make version V=x.y.z  set the version in Cargo.toml and Cargo.lock

dev:
	cargo run

build:
	cargo build --release --locked

install:
	cargo run --release --locked -- install

package:
	powershell -NoProfile -Command "& .\scripts\build-windows.bat; exit $$LASTEXITCODE"

test:
	cargo test --locked

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

clippy:
	cargo clippy --all-targets --locked -- -D warnings

check: fmt-check clippy test

icon:
	cargo run --release -- export-icon build/windows/icon.ico

icon-preview:
	cargo run --release -- icon-preview icon-preview.bmp

version:
	powershell -NoProfile -ExecutionPolicy Bypass -File scripts/set-version.ps1 $(V)
