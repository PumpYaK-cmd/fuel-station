# drag-fx: drag-and-drop animations + one-write move (cards 716b6692, c45a5901)

base: 9ddc7da39f95f247a790ea1b8edb13174e4d274a

## Context
Drag today (`src/ui/mod.rs`: `Drag`, `Message::Press/EnterCard/ExitCard/EnterColumn/DropOn/
CancelDrag`, `view_column`, `card_box`): the dragged card recolors in place, a 2px accent
line marks the drop slot, target column brackets turn accent. The user picked all four
animations below. `window::frames()` already drives `App.t` every frame, so any
`Instant`-based value in `view` animates without new subscriptions. Existing idiom for
time-based fades: `appeared: HashMap<String, Instant>` + `FLASH_SECS`, `grow` easing in
`view_column`. Follow that idiom (Instant stamps + ease function), not `iced::Animation`.

Card c45a5901 is folded in: the cross-column drop calls `card_move` then `card_reorder`
(two transactions, two events). Fix it in the same DropOn rewrite.

## Design

### Store (`src/store/mod.rs`, `src/store/tests.rs`)
- New `pub fn card_place(&mut self, card_key, actor, column: Column, position: i64) ->
  Result<Card>`: one `self.write`. If `column != card.column`: set `col` (as `move_to`
  does), then reorder the target column with the card inserted at `position` clamped
  (the body of `card_reorder`, moved into a private `fn reorder(tx, project_id, card_id,
  column, position) -> Result<usize>` returning the clamped index; `card_reorder` calls it
  too). One event: cross-column → kind `card_move`, message exactly
  `"Moved {key} {from} -> {to}"` (`notify::toast_text` matches the `-> Review` / `-> Done`
  suffix: do not change the text); same column → kind `card_reorder`, message as
  `card_reorder` writes today.
- Tests next to the existing card_move/card_reorder tests: cross-column place at 0 lands
  first in the target column with one new event of kind `card_move`; same-column place
  equals `card_reorder`; position past the end clamps.

### Drag state (`src/ui/mod.rs`)
`Drag` gains:
- `cursor: Point` (window coords, from the subscription below), `lifted: Option<Instant>`
  (set when `moved` first becomes true),
- `slot: Option<(Column, Option<String>)>` + `slot_at: Instant`: the current drop slot
  (column, card it goes before; `None` = end) and when it was set,
- `prev: Option<((Column, Option<String>), Instant)>`: the slot being closed.
- `over_at: Instant`: when `over` last changed.
Replace `before` with `slot`. One helper `fn set_slot(drag, slot)` pushes the old slot
into `prev` and stamps `slot_at` only when the slot actually changes.

Messages:
- `DragMove(Point)` from a subscription added next to the existing `CancelDrag` one while
  `app.drag.is_some()`: `mouse::Event::CursorMoved { position }` → `DragMove(position)`.
- `EnterCard(key)` while dragging → `set_slot(Some((column, Some(key))))`.
- `ExitCard` no longer touches the slot (an exiting card clearing the slot is what makes
  an opening gap oscillate).
- New `EnterTail(Column)`: each column list ends with a `mouse_area` over a
  `DROP_GAP`-tall empty container (always present, invisible) → `set_slot(Some((col, None)))`.
- `EnterColumn(col)` keeps setting `over`/`moved`; it sets the slot to `(col, None)` only
  when the slot's column differs from `col`.
- `DropOn(col)`: slot column must equal `col`, else treat as `(col, None)`. Compute index
  with `drop_index` as today, then call `card_place` once (skip when same column and
  index unchanged). On `Ok`, insert the key into `app.appeared` and `app.landed =
  Some((key, Instant::now()))`.

### The four effects
Constants at the top of the drag section with units:
`DROP_GAP: f32 = 44.0` (px), `GAP_SECS: f32 = 0.15`, `LIFT_SECS: f32 = 0.12`,
`LAND_SECS: f32 = 0.25`, `GHOST_WIDTH: f32 = 260.0` (px), `GHOST_OFFSET: Vector =
(24, 14)` (cursor position inside the ghost, px). One `fn ease_out(x: f32) -> f32`
(`1 - (1 - x.clamp(0,1))^3`, the formula `grow` already uses; make `grow` call it).

1. **Floating ghost.** Split `card_box`'s inner face (key/tier row, title, holder line,
   accent bar) into `fn card_face(card, bar: Color) -> Element`. In `App::view`, when a
   drag has `moved`, push a top layer onto the root `stack!`:
   `pin(float(container(card_face(card, PRIMARY)).width(GHOST_WIDTH).style(theme::ghost))
   .scale(1.0 + 0.04 * ease_out(lift)))` positioned at `cursor - GHOST_OFFSET`, `pin`
   sized `Fill`. The ghost holds no interactive widget (no button / mouse_area): the
   stack only hides the cursor from lower layers when an upper layer reports a mouse
   interaction, so a passive ghost keeps hover and drop working underneath.
   New `theme::ghost`: `card` style with `PRIMARY` border, `BACKGROUND.scale_alpha(0.92)`
   background, a `Shadow` (black 0.5 alpha, offset (0, 6), blur 18). The source card
   stays in its slot as a placeholder: face text `TEXT_MUTED`, border
   `PRIMARY.scale_alpha(0.4)`, bar `BORDER`.
2. **Gap opens at drop slot.** Replace the 2px line: the card whose key is the slot's
   `before` renders, inside its own `mouse_area` and above its face, a container of height
   `DROP_GAP * ease_out(slot_age / GAP_SECS)` styled `theme::drop_slot` (border
   `ACCENT.scale_alpha(0.6)`, background `ACCENT.scale_alpha(0.08)`). The card matching
   `prev` renders the same container at `DROP_GAP * (1 - ease_out(prev_age / GAP_SECS))`
   until it reaches 0. A `None` slot opens the gap in the column's tail area instead.
   The gap sits inside the hovered card's mouse_area so the card under the cursor never
   slides out from under it.
3. **Landing settle.** `App.landed: Option<(String, Instant)>`; the landed card renders as
   `float(card_box(..)).scale(1.0 + 0.05 * (1 - ease_out(age / LAND_SECS)))` and its
   bar flashes through the existing `appeared` path. Clear `landed` in `view`-independent
   code: on the next `Frame` after `LAND_SECS`.
4. **Target column glow.** `fx::Brackets` gains `len: f32` (replaces the `BRACKET`
   const at call sites, `BRACKET` stays the resting length). Target column:
   `len = BRACKET + 8 * ease_out(over_age / GAP_SECS)`, `alpha = 0.75 + 0.25 *
   (t * 2π).sin()`, color `ACCENT`; its panel uses new `theme::glass_lit`
   (`SURFACE_GLASS` brightened by mixing toward `ACCENT` at 0.06; add a token
   `SURFACE_GLASS_LIT` in `theme.rs`, no inline hex).

Remove `Drag.before`, the 2px line and the `drop_before` param. Update the `drop_index`
tests only as far as signatures change.

## Verification
Gate. Manual (the user; visual feel cannot be gated):
1. Drag a card: ghost lifts (slight scale) under the cursor, source slot dims.
2. Move over cards in another column: gap opens smoothly above the hovered card, the old
   gap closes, no flicker with the cursor held still on a gap or card edge.
3. Move below the last card: gap opens at the end. Drop: card lands there, scales down
   into place, accent flash. Activity shows ONE "Moved X A -> B" line.
4. Reorder within a column: one "Moved X to position N" line.
5. Target column brackets grow and pulse; leaving the column returns them to rest.
6. Release outside any column: drag cancels, no ghost left behind. Click without moving
   still opens the card.

## Out of scope
Auto-scroll while dragging, keyboard drag, drag between projects.

Tier:  2 implementer-high. Why: layout feedback loops (gap vs hover) need judgment; behaviour only shows when run.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: animation feel is visual.
Scope: 5 files (store/mod.rs, store/tests.rs, ui/mod.rs, ui/fx.rs, ui/theme.rs), drag path and board view only. Asks: none
