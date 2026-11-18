# AGENTS.md

Guidelines for AI coding agents (and people) working in this repository.
What lives where and how the pieces fit together is in [STRUCTURE.md](STRUCTURE.md): read it first,
instead of re-reading the whole repo.

These guidelines come from a shared template: https://github.com/jacopofilonzi/agents-guideline.
See "Updating these guidelines" below to align them with the latest version.

## What it is

Swytch: a tray app that switches a monitor's input (DisplayPort ↔ HDMI) over DDC/CI from global
hotkeys, a tray menu or a quick switch window. Stack: Rust (stable, edition 2021, toolchain
`x86_64-pc-windows-gnu`), plain Win32 through `raw-dylib`, zero dependencies. Targets: Windows 10 1703+ / 11, x64.

## Workflow

- **Non-trivial changes start with a plan.** For a feature, refactor, new dependency or anything touching
  several files: read what you need, then present the plan (what changes, which files, trade-offs, open
  questions) and wait for my approval before editing. Small, obvious fixes can go straight in. If the plan
  changes significantly along the way, stop and ask again.
- **Big changes go to one agent, small ones you do yourself.** The goal is to save the main conversation's
  context: the agent does the heavy reading and editing in its own context. Don't spawn sub-agents for
  everything. For a big change, once I've approved the plan, write it out in full (goal, steps, files it
  owns, constraints, how to verify) and launch a single agent to carry it out following that plan; it
  answers with a short report (what changed, which files, `make check` result, open issues), not file
  dumps. Then check it from the diff and `make check` instead of re-reading everything it read. Small
  changes you make directly in the main conversation, without agents.
- **Propose an architecture change when something outgrows its start.** A part that began small and
  simple may keep gaining responsibilities. When it starts to strain its original approach (state wired
  by hand, duplicated markup or logic, ever-growing glue code), stop and propose moving to a better fit,
  with trade-offs and a migration path, instead of piling more onto the simple version. Don't migrate
  without my approval. (Here: hand-written Win32 windows are fine for two small dialogs; a third,
  richer window would be the moment to discuss a UI toolkit.)
- When something is genuinely my decision (UX, user-visible naming, data formats, breaking changes), ask
  instead of guessing. Otherwise pick the sensible default and say what you picked.
- Run commands from the repo root through the Makefile (`make help` lists them; on the main dev machine
  GNU make is `mingw32-make`). A new recurring command goes into the Makefile too.
- **While developing, use the dev build** (`make dev` = `cargo run`). Build the release exe (`make build`,
  `make package`) only to check something specific to it (size, embedded resources, startup time).
- **Always run `make check` before declaring work done.** It must pass with zero errors and zero warnings.
  If you couldn't run something (a monitor you don't have, a reboot, a platform you're not on), say so
  instead of claiming it works.
- Makefile recipes must work in both `sh` and `cmd.exe` (GNU make on Windows falls back to `cmd.exe`): only
  `cd dir && command`, no inline `VAR=x cmd`, no `rm`/`cp`/`mkdir -p`, no parentheses in `echo`; use
  PowerShell scripts in `scripts/` (or the app itself) for file operations.
- Every new env var is read in one place and documented in STRUCTURE.md. The only ones today are the build's:
  `WINDRES` (path to windres) and `SWYTCH_REQUIRE_RESOURCES=1` (fail the build without resources; CI).
  Never commit `.env` or secrets.
- **Never cause real side effects without asking.** These act for real on this machine:
  `ddc::set_input` and the `dp`/`hdmi`/`toggle` commands (they switch the real monitor; switching it to
  HDMI blanks the screen you are working on), the hotkeys AltGr+F13…F15 while the app runs,
  `install`/`uninstall` and the autostart checkbox (scheduled task `Swytch`, `Run` values), saving the
  settings (`%APPDATA%\Swytch\config.ini`). Reading the current input or the capabilities is harmless.
  When running the app for a check, stop it afterwards and restore the installed instance if one was running.

### Commits

- Small, step-by-step commits as the work progresses, never a single commit at the end. Conventional
  prefixes: `feat:`, `fix:`, `refactor:`, `chore:`, `docs:`, `test:`, `ci:`.
- **Exception, experiments:** while we're trying things out (styling, layouts, alternative approaches),
  don't commit. Commit only once I've picked the version to keep.
- Mockups and design alternatives are never committed: they only serve to choose; the chosen one lives in the code.
- Push, tag and release only when I ask.

### Keep the agent docs in sync, in the same change

`STRUCTURE.md` and this file exist so agents don't have to re-read the whole repo. When you add, move,
rename or delete a file or module, or change a command, config key, invariant, data format or workflow
command, update `STRUCTURE.md` / `AGENTS.md` (and `README.md` for user-facing changes) in the same commit
or right after. Before declaring work done, check that every path, name and value they mention for the
areas you touched still matches the code.

### Updating these guidelines

When I ask you to check for new guidelines, fetch the latest template from
https://github.com/jacopofilonzi/agents-guideline (raw files:
`https://raw.githubusercontent.com/jacopofilonzi/agents-guideline/main/{AGENTS,CLAUDE,STRUCTURE}.md`)
and align this project's copies with it:

- Bring in new and changed generic rules (workflow, commits, code, testing…) and drop the ones removed
  upstream, without asking.
- Keep everything project-specific: filled-in placeholders, invariants, recipes, pitfalls, the whole
  content of `STRUCTURE.md`, and rules this project added or deliberately changed.
- Ask only when an upstream change conflicts with the project's current style or guidelines (e.g. it
  contradicts a project-specific rule or a convention the code follows); list each conflict with both versions.
- Then summarize what changed and commit it as `docs: sync agent guidelines`.

## Invariants — don't break these

- **Hotkeys are live before anything else.** In `app::run` they are registered right after the main
  window exists, before the tray icon and before any UI work. Nothing slow (DDC enumeration, file
  scans, network) runs on the startup path. Check the "hotkeys ready after N ms" log line after changes there.
- **The UI is lazy.** Fonts, window classes and comctl32 v6 are created in `ui::ensure_ready`, only when a
  window opens; the tray-only process stays at a few MB.
- **Start at sign-in is the logon scheduled task**, never a `Run` value (Windows delays those by seconds).
  The task must not need admin rights (`LeastPrivilege`, current user).
- **`config.ini` is durable user data:** key names stay stable, unknown keys are ignored, invalid or
  missing values fall back to the defaults per key, and old files (including the previous version's,
  migrated from `%APPDATA%\monitorSwitch`) keep loading. Saves are atomic (temp file + rename).
- **One instance:** mutex `Swytch_SingleInstance`; `install` updates by sending `CMD_EXIT` to `SwytchMain`
  and waiting for it to exit. Keep those names and the `CMD_*` numbers stable.
- The hotkeys are unregistered while the settings window is open (the recorder needs the keys) and
  registered again in its `WM_DESTROY`.
- No dependencies: Win32 goes through `src/win32.rs` with `raw-dylib`, so the GNU toolchain alone builds
  the app. Ask before adding a crate.

## Code

- Everything is written in English (code, comments, commits, docs), except the language packs.
- Keep modules small and focused: one module per area; split a file when it grows several responsibilities.
- Comments explain *why*, not *what*. Put the design of a non-trivial module in its top-level doc comment
  (upstream formats, invariants, data layout).
- Never ignore errors: log them with context (`logging::log`). No `unwrap()`/`expect()`/unchecked casts on
  external data (files, monitor replies, user input). Every `unsafe` block says why it is sound (`// SAFETY:`).
- Errors carry a machine-readable kind (e.g. `ddc::DdcError`) and an English form for the log; the UI maps
  the kind to its own text. User-facing text never comes from an error's `Debug`/log form.
- Use the project's logger, never `println!`/`eprintln!` (the exe has no console).
- Win32 declarations live only in `src/win32.rs`; other modules use them, they don't declare FFI.
  Build paths with `Path::join`, never by concatenating `\`.
- Deleting user data means moving it to the Recycle Bin / soft delete, unless agreed otherwise.

### UI

- Every UI string goes through `i18n` (`tr()`), with both the English and the Italian text (English is the
  default and the fallback). Both packs are values of one struct, so a missing key is a compile error; a
  test checks the `{0}` placeholders match.
- Layout coordinates are written at 96 DPI and scaled with `app::s`; never hard-code physical pixels.
- Windows that may be needed while the main screen is switched away (the quick window) open on the
  secondary display.

## Testing

- Unit tests live next to the code (`#[cfg(test)] mod tests`). Test parsing against fixtures copied from
  real replies (`testdata/`), never live monitor calls; tests must pass on a machine without DDC/CI.
- Before committing a fixture taken from a real device, remove serial numbers and anything that
  identifies a person. Never commit real user files.
- End-to-end checks drive the built exe (window messages to `SwytchMain`, simulated hotkeys) and read
  `Swytch.log`. Never simulate the HDMI hotkey on the dev machine (see side effects above).

## Versions and releases

- Versions are `x.y.z`: `x` major (only when I decide), `y` feature, `z` fix. A feature bumps `y` and resets `z`.
- The version lives in `Cargo.toml` (and `Cargo.lock`): change it only with `make version V=x.y.z`.
  `build.rs` stamps it into the exe's version info; the app reads it as `crate::VERSION`.
- Tags and releases only when I ask. Releasing = commit the version bump, push, then push the tag
  `vX.Y.Z`: `release.yml` checks that the tag matches `Cargo.toml` and publishes the release.
- CI runs the checks and tests on every push and pull request, but builds and publishes artifacts only on a
  `vX.Y.Z` tag or a manual run, never on a plain push.

## Adding a hotkey action

1. Add a `CMD_*` constant and its arm in `app::command`, plus a hotkey id in `HOTKEY_IDS` /
   `HOTKEY_LOG_NAMES` and the `WM_HOTKEY` match in `app::main_proc`.
2. Grow `Config::hk` with a new key (`config.rs`: default, `parse`, `to_ini`, tests); old files without it
   must keep loading.
3. Add the action name to `i18n` (`action_*`, both languages, `Strings::actions`), a row in the settings
   window (`ui/settings.rs`, window height) and an entry in the tray menu (`tray::show_menu`).
4. Update README.md (usage table) and STRUCTURE.md.

## Known pitfalls

- The main dev machine is Windows 11; shells are PowerShell and Git Bash. Commands and scripts must work there.
- **Git Bash rewrites env values and arguments that look like paths** (`/c` becomes `C:/`): prefix the
  command with `MSYS2_ARG_CONV_EXCL='*'`, or go through PowerShell.
- A running `.exe` locks its binary on Windows: stop it (tray → Exit, or `install` does it) before
  replacing the installed copy. `cargo build` is not affected (it writes to `target/`).
- **Shells started by the Claude desktop app (MSIX) see a virtualised `%APPDATA%`/`HKCU`**: files and
  registry values written from there may land in `…\Packages\Claude_…\LocalCache` instead of the real
  location. To inspect or change the real state, run the command outside the package (e.g. through a
  temporary scheduled task).
- **Windows' foreground lock:** a window opened by a test process may stay behind others. Triggering one of
  the app's own hotkeys first (e.g. AltGr+F13, harmless when already on DisplayPort) gives the app the
  right to come to the front.
- **AltGr is Ctrl+Alt.** QMK/VIA keyboards often send F13+ with AltGr; bare F13 hotkeys never fire then.
- rustup won't install the toolchain from `rust-toolchain.toml` (its channel names the `-gnu` target): run
  `rustup toolchain install stable-x86_64-pc-windows-gnu` once; CI does it explicitly.
- `schtasks /XML` only accepts UTF-16 with a BOM; the XML must say `encoding="UTF-16"`.
- The Samsung T28E316 on the dev machine has no DDC/CI: "auto" correctly picks the Lenovo R27qe.
- A 256 px DIB in the .ico would add ~260 KB to the exe: the largest embedded size is 128 px on purpose.
