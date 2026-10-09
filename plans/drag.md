# drag: drag-and-drop cards between and within columns (card 9d4e33ee)

base: (set at claim)

## Context
`src/ui/mod.rs`: `card_box` wraps each card in a `mouse_area` whose `on_press` opens the
detail (`Message::OpenCard`, which goes through the unsaved-edits guard). `view_column`
stacks a `glass` panel and a `fx::Brackets` canvas. Store already has
`card_move(key, actor, column)` (appends to the end of the target column) and
`card_reorder(key, actor, position)` (position within its column, clamped).
iced 0.14 has no drag-and-drop widget; `mouse_area` gives `on_press`, `on_release`,
`on_enter`, `on_exit`.

Rulings: press on a card starts a potential drag; it becomes a drag once the pointer enters
another card or another column. Release with no drag = click (opens the card as now).
Drop on a card inserts before that card; drop on a column's empty area appends. Releasing
outside any column cancels. Any card can be dragged, `[HUMAN]` included (the manager is the
one moving it). No ghost under the cursor; feedback is: dragged card bar `primary` and
border `primary`, target column brackets `accent`, target card shows a 2 px `accent` rule
above it, cursor `Grabbing` while dragging.

## Design
`src/ui/mod.rs`:
- `struct Drag { key: String, from: Column, over: Option<Column>, before: Option<String>,
  moved: bool }`; `App.drag: Option<Drag>`.
- Messages: `Press(String)`, `EnterCard(String)` (replaces `HoverCard` and also feeds the
  drag), `ExitCard(String)` (replaces `UnhoverCard`), `EnterColumn(Column)`,
  `DropOn(Column)`, `CancelDrag`.
- `card_box`: `on_press(Press(key))`, `on_enter(EnterCard(key))`,
  `on_exit(ExitCard(key))`; interaction `Grabbing` when this card is being dragged,
  `Pointer` otherwise. Takes a `dragged: bool` and `drop_before: bool` flag for the visuals
  above.
- `view_column`: wrap the returned stack in
  `mouse_area(..).on_enter(EnterColumn(col)).on_release(DropOn(col))`. Brackets use
  `theme::ACCENT` when `drag.over == Some(col) && drag.moved`.
- `update`:
  - `Press(key)`: `drag = Some(Drag { key, from: card column, over: Some(from),
    before: None, moved: false })`.
  - `EnterCard(key)`: hover as now; if dragging and `key != drag.key`: `before =
    Some(key)`, `over` = that card's column, `moved = true`.
  - `ExitCard(key)`: unhover as now; if dragging and `before == Some(key)`, `before = None`.
  - `EnterColumn(col)`: if dragging and `col != over`: `over = Some(col)`, `before = None`,
    `moved = true`.
  - `DropOn(col)`: take `drag`. Not moved → behave as `OpenCard(drag.key)` (route through
    the same code path so the unsaved-edits guard applies). Moved → target index = index
    of `before` among `col`'s cards excluding the dragged one, else that count (append).
    If `col != from`, `card_move` then `card_reorder` to the index (skip reorder when
    appending); else `card_reorder` only if the index differs from the current one. Use
    `after_write` for errors/reload.
  - `CancelDrag`: `drag = None`.
- Subscription: when `drag.is_some()`, add `iced::event::listen_with` mapping
  `mouse::Event::ButtonReleased(Left)` with status `Ignored` → `CancelDrag` (the column
  `mouse_area` captures releases over a column, so only releases elsewhere arrive here;
  verify that `mouse_area::on_release` captures in iced 0.14 source and, if it does not,
  cancel on `Ignored`-only and still let DropOn win by processing order. Report which).
- Index math as a pure fn `fn drop_index(cards: &[&CardSummary], dragged: &str,
  before: Option<&str>) -> usize` with unit tests (`#[cfg(test)] mod tests` in
  `src/ui/mod.rs`; another card may have created it, append to it): before first, before
  middle, append, dragged card excluded, before = dragged card.

## Verification
Gate. Manual for the user:
1. Click a card: opens as before. Unsaved edits guard still shows when a dirty detail is
   open.
2. Drag a card to another column's empty area: lands at the bottom. Onto a card: lands
   above it. Within a column: reorders.
3. Drag and release over the sidebar or title bar: nothing moves.
4. Activity feed shows the move/reorder events; MCP `board` agrees with the order.

## Out of scope
Ghost image, auto-scroll while dragging, drag between projects.

Tier:  2 implementer-high. Why: event ordering and capture semantics in iced need judgment.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: interaction only verifiable by hand; index math tested.
Scope: 1 file (ui/mod.rs: card_box, view_column, update, subscription, tests). Asks: none
