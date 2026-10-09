# Docking Bay redesign

Card: see board (Ready, "Docking Bay redesign: integrated title bar, animated backdrop").
Reference: `docs/mockups/2-dock.html` (open in a browser for the animations). Chosen over
Orbit, Nebula and Mission Control on 2026-10-09; the other three get deleted on ship.

base: 5244558bb4376af14024fa3725984af17f7a821d

## Context

The GUI works but is flat: OS title bar, opaque outlined panels, nothing moves. The theme
is a space fuel station. Keep the base (JetBrains Mono Nerd Font, 1 px outlines, radius 2,
Station Night palette, pumpbin spacing) and add: an in-app title bar on an undecorated
window, a blueprint-grid backdrop with a slow light sweep, bracket-corner columns with a
pulsing Doing bay, column fill gauges, left accent bars on cards, hover and new-card
feedback, a blinking LIVE tag on the activity feed.

Everything below is drawable in iced 0.14 as shipped in `Cargo.lock` (checked in the cargo
registry sources): `Application::decorations(false)`, `window::{drag, minimize,
toggle_maximize, close, latest, frames}`, `mouse_area::{on_press, on_double_click, on_enter,
on_exit}`, `stack!`, `canvas` + `canvas::Cache`, `geometry::Style::Gradient` (linear only),
`container::Style.shadow`, `progress_bar::girth`. No blur, no radial gradient, no letter
spacing, no transforms: the mockup's fade mask on the grid is approximated per segment, the
CSS rise-in becomes a colour flash.

## Design, file by file

### `src/ui/theme.rs`
Add tokens, all derived from the existing palette:
- `SURFACE_GLASS = SURFACE at alpha 0.93` (columns, activity, detail), `SIDEBAR_GLASS =
  SURFACE at 0.80`, `BAR_GRID = BORDER_STRONG at 0.5`, `SWEEP = PRIMARY at 0.08`.
- `COLUMN_CAP: f32 = 20.0` (gauge full at 20 cards; unit: cards).
Style fns (replace `panel` / `card_box`, add the rest):
- `glass(theme)`: container, background SURFACE_GLASS, border transparent (brackets draw
  the outline), radius 2.
- `sidebar(theme)`: SIDEBAR_GLASS, right border 1 px BORDER via the existing `rule`.
- `card(bar: Color, border: Color)`: container, background BACKGROUND (opaque), border 1 px
  `border`, radius 2. The bar is a sibling container in the row (see mod.rs), width 2,
  background `bar`.
- `gauge(color)`: progress_bar style, background BORDER, bar `color`, radius 0.
- `window_button(close: bool)`: button, no border, text TEXT_MUTED; Hovered → background
  SURFACE_HOVER and text TEXT, or DANGER background + white text when `close`.
- `scrollbar(theme)`: scrollable style, transparent container, rail transparent, scroller
  BORDER_STRONG 1 px wide at radius 0; hovered scroller PRIMARY.
- `input(theme, status)`: text_input style, background BACKGROUND, border BORDER, focused
  border PRIMARY, placeholder TEXT_MUTED, value TEXT, selection PRIMARY at 0.3.
- `picker(theme, status)`: pick_list, same colours as `input`, handle PRIMARY.
Keep `button(selected)` and `theme()` unchanged.

### `src/ui/fx.rs` (new)
Two `canvas::Program<Message>` impls, both stateless (`type State = ()`), both take `t:
f32` seconds since boot.
- `Backdrop { t, grid: &canvas::Cache }`. `draw`:
  1. fill bounds with BACKGROUND;
  2. grid from the cache: 40 px cells; every cell edge is one `Path::line` stroked 1 px in
     BAR_GRID with alpha scaled by `1 - clamp((d - 0.3) / 0.5, 0, 1)` where `d` is the
     distance of the segment midpoint from (50 %, 40 %) of the bounds, normalised by half
     the diagonal. That reproduces the mockup's radial mask;
  3. sweep, uncached: a rectangle 200 px tall, full width, at `y = ((t % 9) / 9) * (h +
     200) - 200`, filled with a vertical `Gradient::Linear` transparent → SWEEP → transparent.
  The cache is owned by `App` and cleared on `window::resize_events`.
- `Brackets { color: Color, alpha: f32 }`. `draw`: four 12 px L-shapes at the corners of
  the bounds, 1 px stroke, `color` at `alpha`. Used as the top layer of a `stack!` over each
  column.

### `src/ui/titlebar.rs` (new)
`pub fn view(app: &App) -> Element<Message>`: a `mouse_area` (on_press `DragWindow`,
on_double_click `ToggleMaximize`) around a 36 px row:
- brand: `\u{f0298}` in ACCENT + "FUEL STATION" size 11, TEXT;
- `space::horizontal()`;
- the agents strip moved here from `view_agents` (pulse dot: ACCENT with alpha
  `0.6 + 0.4 * sin(t * PI)`; count; one `glyph name card` per agent in ACCENT);
- three `window_button`s 46 px wide, full height: `\u{f2d1}` Minimize, `\u{f2d0}`
  ToggleMaximize, `\u{f00d}` Close.
Background SURFACE, bottom `rule::horizontal(1)`. Child buttons capture their own press
before the mouse_area sees it, so dragging never starts from a control; verify (checklist).

### `src/ui/mod.rs`
- `run()`: add `.decorations(false)` and `.window(window::Settings { min_size:
  Some(Size::new(960.0, 600.0)), ..Default::default() })`.
- `App` gains: `window: Option<window::Id>`, `booted: Instant`, `t: f32`, `hovered:
  Option<String>`, `appeared: HashMap<String, Instant>`, `reloaded: Instant`, `grid:
  canvas::Cache`.
- `Message` gains: `WindowId(Option<window::Id>)`, `Frame(Instant)`, `DragWindow`,
  `Minimize`, `ToggleMaximize`, `Close`, `Resized`, `HoverCard(Option<String>)`.
- `update`: on the first `Tick` with `window == None`, return
  `window::latest().map(Message::WindowId)`. `Frame(now)` sets `t`. Window messages map
  to `window::drag / minimize(id, true) / toggle_maximize / close` when `window` is Some,
  else `Task::none()`. `Resized` clears `grid`. `HoverCard` sets `hovered`.
- `subscription`: batch the existing tick and keys with `window::frames().map(Message::
  Frame)` and `window::resize_events().map(|_| Message::Resized)`.
- `try_reload`: after swapping `board`, insert every key not in the old board into
  `appeared` with `Instant::now()`, drop entries older than 1 s, set `reloaded`.
- `view`: `stack![canvas(Backdrop).width(Fill).height(Fill), column![titlebar::view(self),
  body, rule, footer]]`. Sidebar container styled `sidebar`. The board header row keeps
  `KEY · name` and the Repair/Pick button; `view_agents` is deleted (lives in the title
  bar now).
- `view_column`: header row = name (size 11, uppercase as now), count, then
  `progress_bar(0.0..=1.0, fill).girth(3).style(gauge(color))` with `fill = min(n,
  COLUMN_CAP) / COLUMN_CAP * ease_out(min((now - reloaded).as_secs_f32(), 1.0))`, colour
  PRIMARY / WARNING for Review / SUCCESS for Done; `rule::horizontal(1)` under the header.
  Body: `stack![container(column![header, list]).style(glass), canvas(Brackets {..})]`.
  Brackets colour BORDER_STRONG alpha 1; Doing uses PRIMARY with alpha `0.7 + 0.3 *
  sin(t * PI)`.
- `card_box(card, selected, hovered, flash)`: `row![bar, body]` inside the `card` style.
  Bar colour, first match wins: selected → ACCENT; human → WARNING; flash (key in
  `appeared`, age < 0.6 s) → ACCENT mixed toward BORDER_STRONG by age/0.6; hovered →
  PRIMARY; else BORDER_STRONG. Border: ACCENT when selected, WARNING when human, else
  BORDER. `mouse_area` gets `on_enter(HoverCard(Some(key)))` and
  `on_exit(HoverCard(None))`. Holder line size 11.
- Scrollables in columns and activity use `scrollbar`. `pick_list` in the footer uses
  `picker`. Footer container background SURFACE, text size 11.

### `src/ui/activity.rs`
Header row: "ACTIVITY" + "LIVE" size 10 in SUCCESS with alpha `if (t * 0.625) % 1.0 < 0.5
{ 1.0 } else { 0.25 }` (1.6 s blink). Time cell followed by `│` in BORDER_STRONG. Panel
styled `glass`, scrollable styled `scrollbar`. Takes `t` as a parameter.

### `src/ui/detail.rs`, `src/ui/setup_sheet.rs`
Panel style `glass`; `text_input` / `text_editor` styled `input`; scrollable `scrollbar`.
No layout change.

### `docs/design.md`
Rewrite the UI section's ASCII sketch and style rules to match: title bar row at the top,
agents in it, bracket columns, gauges, bars. Add a "Motion" paragraph: one clock `App::t`
from `window::frames()`, every animation is a pure function of `t` or of an `Instant`
stored on reload; no per-widget animation state. Add the new tokens to the palette table.

### `CLAUDE.md`
One rule line: "Window is undecorated: the title bar is `ui::titlebar`; drag and window
controls go through `window::{drag, minimize, toggle_maximize, close}` with `App::window`."

### `docs/mockups/`
Delete `1-orbit.html`, `3-nebula.html`, `4-control.html`, `index.html`, `gen.py`, all
`.png`. Keep `2-dock.html` and point to it from design.md.

## Verification

Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

Manual checklist (the gate can't see pixels):
1. Window opens without the Windows chrome; title bar drags the window, double-click
   maximises, the three glyph buttons minimise / maximise / close; clicking a button does
   not start a drag.
2. Grid fades toward the edges; the sweep crosses the window about every 9 s.
3. Doing brackets pulse; other columns' brackets are steady.
4. Gauges fill in over ~1 s on project switch; Done shows green, Review amber.
5. Hovering a card turns its bar blue; the open card is cyan; `[HUMAN]` is amber.
6. Add a card through the MCP (or `card add`) and watch it flash cyan once.
7. LIVE blinks; the activity feed still scrolls.
8. CPU with the window idle on a 150-card board stays under ~5 % on this machine. If not,
   replace `window::frames()` with `iced::time::every(Duration::from_millis(50))` and
   note it in the hand-over.
9. Resize: the grid redraws at the new size, nothing overflows at 960×600.
10. Theme picker still switches iced themes; the backdrop stays Station Night (accepted).

## Out of scope
Card rise-in translation (iced has no transforms), blur, drag-and-drop, toasts, a theme
picker that recolours the backdrop, Linux/macOS window-control conventions.

## Triage
    Tier:  2 implementer-high. Why: two canvas programs, hover and flash state, window
           plumbing; the plan names the pieces but the drawing math is the implementer's.
    Gate:  cargo fmt/clippy/test, manual: checklist 1-10 above. Why: all visual.
    Scope: 9 files (theme, fx, titlebar, mod, activity, detail, setup_sheet, design.md,
           CLAUDE.md) + mockup cleanup; touches every view fn, no store or MCP change.
           Asks: none

## Fact corrections (at claim)
- `canvas` is an opt-in iced feature: add it to `Cargo.toml`.
- Scrollable width is not a style field: thin scrollbars via
  `Direction::Vertical(Scrollbar::new().width(4).scroller_width(4))`, colours via `scrollbar`.
- Checklist 11: undecorated window still resizes from its edges on Windows; if not, file a
  card for `window::drag_resize` handles.
