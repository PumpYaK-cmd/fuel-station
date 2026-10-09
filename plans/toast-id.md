# toast-id: toasts show as Fuel Station, not Windows PowerShell (card 219f362a)

base: (set at claim)

## Context
`src/ui/notify.rs` `toast` uses `notify_rust::Notification::new().appname(..)`. On Windows
notify-rust ignores `appname` and falls back to PowerShell's AppUserModelID, so toasts
read "Windows PowerShell". Windows accepts an unpackaged app's AUMID when it is registered
under `HKCU\Software\Classes\AppUserModelId\<id>` with a `DisplayName` value; no Start-menu
shortcut needed.

## Design
- `Cargo.toml`: `[target.'cfg(windows)'.dependencies] winreg = "0.56"`.
- `src/ui/notify.rs`:
  - `const APP_ID: &str = "FuelStation.Station";`
  - `#[cfg(windows)] pub fn register_app_id() -> Result<(), String>`: create/open
    `HKCU\Software\Classes\AppUserModelId\FuelStation.Station`, set `DisplayName` =
    `"Fuel Station"` (REG_SZ). Idempotent.
  - `toast`: on Windows call `.app_id(APP_ID)` (notify-rust 4 has
    `Notification::app_id`); drop `.appname` on Windows if it does nothing there, keep it
    elsewhere via `cfg`.
- `src/ui/mod.rs` `boot` (where the tray is created): call `notify::register_app_id()`
  under `#[cfg(windows)]`; on error put `"toast id: {e}"` in `app.error` with
  `get_or_insert_with`, the way the tray error is reported.

## Verification
Gate. Manual: the implementer may fire one toast locally (a scratch `#[test] #[ignore]` or
a temporary call, not committed) and report the source name Windows shows. Manual for
the user: a real toast reads "Fuel Station" as its source.

## Out of scope
Toast icon (`IconUri`), click actions.

Tier:  1 implementer. Why: one registry write and one builder call, both named.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, plus the one local toast. Why: OS toast source is not testable in the gate.
Scope: 3 files (Cargo.toml, ui/notify.rs, ui/mod.rs boot). Asks: none
