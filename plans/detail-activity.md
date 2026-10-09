# GUI: card detail panel + activity feed and agents strip (cards cfff76f1, dc108a27)

base: <filled by agent>

Two cards, one agent, one plan: both edit the same `App` in `src/ui/mod.rs`. Ship them as
one commit series; paperwork on both cards.

## Context
design.md > UI (Card detail, Activity). `src/ui/mod.rs` (GUI shell) holds `App`, `Message`,
`reload`/`try_reload` (atomic: read all, then assign), `view_sidebar`, `view_board`,
`view_column`, `view_footer`, `card_box`, `role_glyph`. Store API in `src/store/mod.rs`:
`card_show`, `card_update(CardPatch)`, `card_move`, `comment`, `events(key, limit: usize)`,
`agents(key)`. iced 0.14 source: `~/.cargo/registry/src/*/iced-0.14.0`,
`iced_widget-0.14.2` (markdown widget: `iced_widget::markdown`; `markdown` feature is on).
Also folds in Backlog card b621e4fb: a holder older than 120 s stays on its card because
the GUI reloads only on data_version change.
Parallel cards: Add Project (src/setup/{detect,write}.rs; adds a sidebar button, a project
header button, a setup sheet file and first-run view to src/ui), agent files (one call in
GUI boot), prompts (src/mcp), import numbering (src/store). Expect Message/App/update
conflicts in src/ui/mod.rs with Add Project; keep both sides.

## Design
- New `src/ui/detail.rs`: `Detail { key: String, card: CardDetail, plan: Vec<markdown::Item>,
  title: String, body: text_editor::Content, comment: String, dirty: bool }` and
  `fn view(&self) -> Element<Message>`.
  - Panel (width 440, `theme::panel`, scrollable): header `FS-3 · Column` + close button
    (`\u{f00d}`); title `text_input`; body `text_editor` (height ~160); **Save** button
    enabled when dirty -> `card_update` with title/body; `Plan` section rendered with
    `markdown::view` (links: no-op message), "no plan" muted when empty; tier / human /
    commit_base..commit_head line; comments (`role glyph name · time` muted, body); last
    events (time, kind, message, muted); move buttons `← Prev` / `Next →` (hidden at the
    ends, `Column::prev/next`) and one button per other column.
  - All writes use actor `Actor { role: Manager, name: "manager" }` through store; errors
    go to `app.error`.
- `App` gains `detail: Option<Detail>`, `events: Vec<Event>`, `agents: Vec<Agent>`,
  `ticks: u32`.
- Messages: `OpenCard(String)`, `CloseDetail`, `TitleEdited(String)`,
  `BodyAction(text_editor::Action)`, `Save`, `CommentEdited(String)`, `AddComment`,
  `MoveTo(Column)`, `Step(bool /*forward*/)`, `LinkClicked(markdown::Uri)` (no-op).
- `card_box` becomes a button (`theme::button`-like hover is not needed: wrap the existing
  container in `button(...).style(transparent)` or `mouse_area` with `on_press(OpenCard)`);
  the open card's box uses `theme::card_box` with ACCENT border (add `card_box_selected`
  style fn in theme.rs, or a `selected` param; one fn, not two copies).
- Keyboard: `←`/`→` move the open card one column (`Step`) via iced's keyboard key-press
  subscription for uncaptured events only, so arrows inside a focused text field don't
  move cards; `Esc` closes the panel. Verify in the source that the listener skips
  captured events.
- Reload: `try_reload` also loads `events(key, 100)`, `agents(key)`, and, when a detail is
  open, `card_show(detail.key)`. If the card is gone, close the panel. Refresh the
  detail's card, comments, events and plan items, but leave `title`/`body`/`comment`
  buffers alone while `dirty` (or the comment box is non-empty); reset them from the card
  when not dirty.
- Tick: as now, plus every 17th tick (~5 s) reload unconditionally so agent liveness
  (`agents()` 120 s window, board holders) expires on screen (card b621e4fb).
- New `src/ui/activity.rs`: `fn view(events, ...)`: bottom panel under the columns, fixed
  height ~170, `ACTIVITY` header, scrollable list newest first: `HH:MM` local time
  (`chrono::Local`), role glyph, card key (or blank), message; `report` kind in TEXT,
  others in TEXT_MUTED; review_result with findings in DANGER, ship in SUCCESS.
- Header strip (in `view_board` header row): right-aligned `● N agents` (ACCENT dot when
  N>0, muted when 0), then each live agent `glyph name card`.
- Colors only from `ui::theme`.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Launch check with `FUEL_STATION_DB` on a temp file and scratch seeding (not committed):
cards with a markdown plan, comments, events, two live agents. Screenshot the board with
the detail panel open and the activity feed. You may click inside the Fuel Station window
to open a card and press the move buttons (computer-use is fine for that window only);
confirm in a screenshot that the card moved and an event appeared. Don't touch other apps.
Manual (user): edit title/body and save; add a comment; ←/→ with and without focus in a
field; an agent's checkin from an MCP session shows in the strip within a second and drops
off ~2 minutes after its last checkin.

## Out of scope
Drag-and-drop, filtering, editing tier/plan from the GUI.

Tier:  2 implementer-high. Why: iced 0.14 widgets (markdown, text_editor, keyboard subscription) read from source; UI state judgment.
Gate:  cargo fmt/clippy/test + launch screenshots. Why: UI is visual.
Scope: src/ui (mod.rs, theme.rs, new detail.rs, activity.rs). Asks: none
