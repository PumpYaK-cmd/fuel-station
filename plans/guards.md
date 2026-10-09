# guards: unsaved-edit, stale-detect and startup-error gaps (cards 51a722ea, 24e80c0c, 86a6abae, dda69740)

base: (set at claim)

## Context
`src/ui/mod.rs` after wave 1:
- `request_leave(app, Leave)` / `leave(app, Leave)` guard a dirty detail for Close, Open,
  Select (`src/ui/detail.rs` `Leave`).
- 51a722ea: `Message::Sheet(..)` calls `app.reload(Some(key))` when `Sheet::update` returns
  a new project key; `try_reload` drops a dirty detail when the project changes.
- 24e80c0c: `Message::Close` closes the window with a dirty detail.
- 86a6abae: `Message::Detected` always replaces `app.sheet`; a slow detection for project A
  can open over B or overwrite a sheet the user is editing; nothing shows while it runs.
  Pick folder inside an open repair sheet relies on `Detected` replacing that sheet.
- dda69740: with the GUI-subsystem exe, `eprintln!` in `ui::run` and panics go nowhere.

## Design
- `Leave` gets `Quit`. `leave` returns `Task<Message>`: `Quit` →
  `app.window.map_or_else(Task::none, window::close)`, others `Task::none()`.
  `request_leave` returns that task (or `Task::none()` when it only sets `leaving`);
  callers return it. `Message::Close` → `return request_leave(app, Leave::Quit)`.
  The other code paths that call `window::close` (the tray Quit from the notify card, if
  present on main) also go through `request_leave(app, Leave::Quit)`.
- `Message::Sheet`: when `key` is `Some` and differs from `app.selected`, call
  `request_leave(app, Leave::Select(key))`; same key → `app.reload(Some(key))` as now.
  `Leave::Select` must not clear `app.sheet` (it doesn't today; the SelectProject arm
  does), so the result list stays visible.
- Detection requests: `App.detecting: Option<u64>` and `App.next_detect: u64`.
  `Picked(key, Some(dir))` and the path branch of `Repair` take `id = next_detect`,
  increment it, set `detecting = Some(id)`, and send `Detected(id, key, form, facts)`.
  `Detected` with `Some(id) != detecting` is dropped; otherwise set `detecting = None` and
  open the sheet as now. `SelectProject`, `AddProject`, `PickFolder`, `Repair` (start of a
  new request replaces the old id), `Sheet(Close)` and `Import` clear or replace
  `detecting`.
  While `detecting.is_some()` and no sheet is open, `view_main` shows
  `container(text("inspecting folder…").color(theme::TEXT_MUTED)).padding(20)` instead of
  the board; with a sheet open, the sheet's title row appends ` · inspecting folder…`
  (pass a `bool` into `Sheet::view`).
- Startup errors, `src/main.rs` + `src/ui/mod.rs`:
  - `fn crash_log() -> PathBuf`: `%LOCALAPPDATA%\FuelStation\crash.log` (next to
    `setup::install::bin_dir()`'s parent; reuse whatever helper gives the FuelStation dir).
  - GUI mode only (`None` arm in `main`): `std::panic::set_hook` that appends
    `<unix secs> <panic info>` to `crash_log()` (create dir, ignore errors) and shows
    `rfd::MessageDialog::new().set_level(rfd::MessageLevel::Error).set_title("Fuel
    Station").set_description(<info + "\nlogged to <path>">).show()`.
  - `ui::run` `Err(e)` → the same dialog with `e`, plus the log line.
  - Keep the existing in-window error for `open()` failures.
- Tests: none for the UI; the plan's manual list covers it.

## Verification
Gate. Manual for the user:
1. Edit a card, click the window ✕: Unsaved changes bar; Discard closes the window; Save
   saves and closes; Keep editing stays.
2. Edit a card, Add Project → Set up: result list shows; after Done the bar is up on the
   old card; Discard switches to the new project.
3. Click Repair setup, immediately click another project: no sheet pops up later.
   "inspecting folder…" shows while detection runs.
4. Pick folder inside a repair sheet still refreshes that sheet.
5. Temporarily force a panic (local edit, not committed): dialog appears, crash.log has
   the line. Implementer does this once and reports.

## Out of scope
ca8d1eb5 (CLI exit codes, ruled separately by the lead).

Tier:  1 implementer. Why: every edit named, all in the existing guard/detect paths.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: UI flows have no tests.
Scope: 4 files (ui/mod.rs, ui/detail.rs, ui/setup_sheet.rs view signature, main.rs). Asks: none
