# notify: OS toasts and a tray icon (card 127f115d)

base: fdde8f5

## Context
design.md lists toasts and tray icon as v2. The card: toast when a card enters Review or
Done, and on agent errors. The GUI already polls `PRAGMA data_version` every 300 ms
(`Message::Tick` in `src/ui/mod.rs`) and reloads; events carry `kind` and `message`.

Rulings:
- Toasts are Windows notifications (the user is usually in another window when an agent
  finishes), shown only while the Station window is not focused.
- Toast-worthy events, across all projects: `submit_review` ("FS-7 ready for review"),
  `ship` ("FS-7 shipped"), `review_result` whose message does not end in `Review: clean`
  ("FS-7 review: N findings"), and `card_move` whose message ends in `-> Review` or
  `-> Done`. Title = project name, body = the event message. "Agent errors" maps to review
  findings; nothing else records errors today.
- Tray icon: always present while the GUI runs. Left click → restore + focus the window.
  Menu: **Show**, **Quit**. Close button still quits (no hide-to-tray).

## Crates
- `tray-icon = "0.26"` (tauri-apps; Windows tray on the winit thread, menus via its `menu`
  re-export of muda).
- `notify-rust = "4"` (Windows backend through WinRT toasts).
Both widely used and maintained; no custom Win32 code.

## Design
- `src/store/mod.rs`: `pub fn events_after(&self, id: i64) -> Result<Vec<Event>>` across all
  projects, `WHERE e.id > ?1 ORDER BY e.id`, LIMIT 50; and
  `pub fn last_event_id(&self) -> Result<i64>` (`COALESCE(MAX(id), 0)`). Test in
  `src/store/tests.rs`: two projects, events after a given id come back in order.
- New `src/ui/notify.rs`:
  - `pub fn toast_text(e: &Event) -> Option<String>`: the mapping above, pure, unit-tested
    in the same file.
  - `pub fn toast(title: &str, body: &str)`: `notify_rust::Notification::new()
    .appname("Fuel Station").summary(title).body(body).show()`, error ignored. Call it on a
    blocking thread (`std::thread::spawn`) so the UI never waits on WinRT.
  - `pub struct Tray { _icon: tray_icon::TrayIcon, show: MenuId, quit: MenuId }` built by
    `pub fn tray() -> Option<Tray>` (None on error, logged to `app.error` by the caller).
    Icon: 32×32 RGBA generated in code: `theme::ACCENT` filled circle on transparent
    (no asset file). Tooltip "Fuel Station".
  - `pub fn tray_messages(tray: &Tray) -> Vec<TrayAction>` draining
    `TrayIconEvent::receiver()` (left click up → `Show`) and `MenuEvent::receiver()`
    (`show`/`quit` ids) with `try_recv`. `enum TrayAction { Show, Quit }`.
- `src/ui/mod.rs`:
  - `App` gets `tray: Option<notify::Tray>`, `focused: bool` (start `true`),
    `last_event: i64` (from `last_event_id()` in `open()`, so old events never toast).
  - Create the tray in `boot` (it runs on the event loop thread after iced starts). If it
    must be created in `open()`, that is also on the main thread; pick whichever works and
    say which.
  - Subscription: add `window::events()` filtered to `Focused` → `Message::Focus(true)`,
    `Unfocused` → `Message::Focus(false)` (always on, also when no detail is open).
  - `Message::Tick`: after the existing data_version logic, when the version changed,
    `events_after(last_event)`; update `last_event` to the max id; if `!focused`, toast
    each mapped event (title = project name from `app.projects` by `project_key`, falling
    back to the key). Then drain `tray_messages`: `Show` → `window::minimize(id, false)`
    chained with `window::gain_focus(id)`; `Quit` → `window::close(id)`.
    Tick now may return a Task; restructure the arm accordingly.
- `docs/design.md` UI section: one bullet for toasts + tray stating the rulings above; drop
  "toasts, tray icon" from the v1 Out list.

## Verification
Gate. Manual for the user:
1. Tray icon appears; left click restores a minimised window; Show/Quit menu work.
2. Minimise or focus another window, move a card to Review from an MCP session (or from
   another Station instance): a Windows toast appears. With Station focused: no toast.
3. Starting Station with old events in the db shows no toast burst.

## Out of scope
Hide-to-tray on close, notification click actions, per-project mute.

Tier:  2 implementer-high. Why: two new platform crates on the winit thread, event plumbing judgment.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: OS integration isn't testable here.
Scope: 6 files (Cargo.toml, store/mod.rs, store/tests.rs, ui/notify.rs new, ui/mod.rs boot/Tick/subscription, docs/design.md). Asks: none
