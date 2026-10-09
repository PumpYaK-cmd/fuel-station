# dirty-guard: never drop unsaved card edits silently (card c644945f)

base: fdde8f5

## Context
`src/ui/mod.rs` `update`: `Message::CloseDetail` and `Message::OpenCard` replace
`app.detail` even when `Detail.dirty`. `Message::Escape` ignores the key when dirty, with no
feedback. `Message::SelectProject` leads `try_reload` to drop the detail when the project
changes, also losing edits.

Ruling: confirm inline. A dirty detail asked to go away shows a bar at the top of the
panel: "Unsaved changes" + **Save** / **Discard** / **Keep editing**.

## Design
- `src/ui/detail.rs`:
  - `#[derive(Debug, Clone)] pub enum Leave { Close, Open(String), Select(String) }`
  - `Detail` gets `pub leaving: Option<Leave>`, `None` in `new`.
  - `view`: when `leaving` is `Some`, render above `header` a row with
    `text("Unsaved changes").color(theme::WARNING)`, then buttons `Save`
    (`Message::SaveAndLeave`, no `on_press` when the title is blank), `Discard`
    (`Message::DiscardAndLeave`), `Keep editing` (`Message::KeepEditing`), all
    `theme::button(false)`.
- `src/ui/mod.rs`:
  - New messages `SaveAndLeave`, `DiscardAndLeave`, `KeepEditing`.
  - `fn leave(app: &mut App, leave: Leave)` performs the action: `Close` →
    `app.detail = None`; `Open(key)` → the current OpenCard body; `Select(key)` → the
    current SelectProject body (clear sheet/import, `reload(Some(key))`).
  - `OpenCard(key)`, `CloseDetail`, `Escape`, `SelectProject(key)`: if `app.detail` is
    dirty, set `d.leaving = Some(..)` and return; otherwise call `leave`. `OpenCard` of the
    card already open does nothing. `SelectProject` of the already selected project while
    dirty does nothing.
  - `Escape` while `leaving` is set acts as `KeepEditing`.
  - Factor the existing `Save` arm into `fn save(app: &mut App) -> bool` (true on success
    or nothing to save). `Save` calls it. `SaveAndLeave`: if `save` succeeds, take
    `leaving` and call `leave`; else keep the bar (error shows in the footer as now).
  - `DiscardAndLeave`: take `leaving`, set `dirty = false`, call `leave`.
  - `KeepEditing`: `leaving = None`.
  - `Detail::refresh` leaves `leaving` untouched.

## Verification
Gate (compile + clippy; the UI has no tests). Manual for the user:
1. Edit a card title, click another card: bar appears, edits stay.
2. Discard → the other card opens. Save → saved, other card opens.
3. Edit, then click ✕ / press Esc / click another project: bar appears each time; Keep
   editing hides it; Esc while the bar shows hides it.
4. No edits: ✕, Esc, other card, other project behave as before.

## Out of scope
Dirty state for the comment box.

Tier:  1 implementer. Why: every edit named; the state machine is spelled out.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: iced UI has no tests.
Scope: 2 files (ui/mod.rs update arms, ui/detail.rs), card detail panel only. Asks: none
