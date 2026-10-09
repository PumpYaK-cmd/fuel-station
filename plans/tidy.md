# tidy: three small backlog fixes (cards da4f8ad1, 06c7b9a7, 72859bf0)

base: 9ddc7da39f95f247a790ea1b8edb13174e4d274a

## 1. Parked leave overwritten (da4f8ad1) - `src/ui/mod.rs` `request_leave`
Ruling: the latest action wins, except nothing replaces a parked `Leave::Quit`.
In the `Some(d) if d.dirty` arm: `if !matches!(d.leaving, Some(Leave::Quit)) { d.leaving =
Some(target); }`. Unit test only if `Leave` handling is already unit-tested; otherwise none.

## 2. Shared app dir (06c7b9a7) - `src/setup/install.rs`, `src/ui/mod.rs`
- `install.rs`: `pub fn app_dir() -> Option<PathBuf>` = `LOCALAPPDATA` joined with
  `FuelStation`. `bin_dir()` becomes `app_dir().expect("LOCALAPPDATA is not set").join("bin")`.
- `ui/mod.rs` `crash_log()`: `crate::setup::install::app_dir().unwrap_or_default()
  .join("crash.log")` (no `expect`: it runs inside the panic hook). Export path as needed
  (`setup::install` visibility) without widening anything else.
- `store::db_path` uses `APPDATA`, a different dir: leave it.

## 3. Toast app id when registration failed (72859bf0) - `src/ui/notify.rs`
- `#[cfg(windows)] static REGISTERED: AtomicBool = AtomicBool::new(false);`
  `register_app_id` stores `true` on success.
- `toast`: `#[cfg(windows)] if REGISTERED.load(Ordering::Relaxed) { notification.app_id(APP_ID); }`.

## Verification
Gate. Manual: none beyond normal use.

Tier:  1 implementer. Why: every edit is spelled out.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test. Why: compile + existing tests cover it.
Scope: 3 files (ui/mod.rs request_leave + crash_log, setup/install.rs, ui/notify.rs). Asks: none
