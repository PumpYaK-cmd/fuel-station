# GUI shell: sidebar, board columns, live refresh (card 7f379315)

base: 7307b4ec6c3656cc008e688cac02bb884d2672f6

## Context
design.md > UI (layout sketch, Sidebar, Board, Footer, Style rules). Spike landed
`src/ui/` (theme tokens + style fns in `ui::theme`, a swatch screen, the iced app builder
in `ui::run`). Store landed `src/store/`; the GUI reads through it only. Read both first.
iced 0.14 source: `~/.cargo/registry/src/*/iced-0.14.0`, `iced_widget-0.14.2`.
Other cards running in parallel: MCP tools (src/mcp), Install (src/setup, main.rs), Import
(src/import, src/store, main.rs). You own src/ui. Later cards add a card detail panel, an
activity feed + agents strip, and Add Project to this shell; keep the App state and view
split so they slot in (a `view_board`, `view_sidebar`, `view_footer` fn each), but build
none of them.

## Design
- Replace the swatch screen with the shell; delete `swatch.rs` (palette tuning moves to
  the real board). Keep `theme.rs` as is; add style fns there only if a new one is needed.
- `src/ui/mod.rs`: `App { store: Store, projects: Vec<Project>, selected: Option<String>
  /*project key*/, board: Vec<CardSummary>, data_version: i64, error: Option<String>,
  theme: ThemeChoice }`. `Store::open_default()` once at boot (long-lived connection).
  Boot selects the first project. Open failure -> the window shows the error text.
- Messages: `SelectProject(String)`, `Tick`, `ThemeChanged(ThemeChoice)`.
- Subscription: `iced::time::every(Duration::from_millis(300))` -> `Tick`. Tick reads
  `data_version()`; if changed, reload projects + board of the selected project. Store
  errors go to `error` (shown in the footer in DANGER), never panic.
- Layout (`row![sidebar, rule::vertical(1), main]`, `column![.., rule::horizontal(1),
  footer]`), spacing/padding 20 between regions, 3-10 inside lists:
  - Sidebar (fixed width ~200): "FUEL STATION" title, one `theme::button(selected)` per
    project showing its name. Empty -> muted text "no projects".
  - Main header: `KEY · name`.
  - Board: five columns from `Column::ALL`, each `container` with `theme::panel`, header
    `BACKLOG 4` (count in TEXT_MUTED, Review header in WARNING, Done in SUCCESS), a
    `scrollable` list of card boxes, `Length::Fill` width each.
  - Card box (`theme::card_box(card.human)`): key in TEXT_MUTED, title (wraps), tier tag
    when set (`[2]`), holder line when set: role glyph + name in ACCENT. Glyphs (Nerd
    Font): truck `\u{f0d1}`, fuel `\u{f0298}` (gas station), mechanic `\u{f0ad}` (wrench),
    manager `\u{f007}`. Check in the screenshot that they render; if one is tofu pick
    another codepoint from the same font and note it.
  - Footer: `station.db · data_version N` (path from `store::db_path()`), error if any,
    and a `pick_list` theme picker on the right: `ThemeChoice` = Station Night plus iced's
    built-in `Theme::ALL`. The style fns keep using the token constants; the picker
    switches the iced `Theme` (pumpbin parity).
- Colors only from `ui::theme`; no inline hex.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Launch check: `FUEL_STATION_DB=<temp file>`, seed two projects and ~8 cards across columns
(one human, one with a tier, one held via `checkin`) with a scratch program or test, run
the GUI, screenshot it (computer-use screenshot if available, else describe), then add a
card from a second process (e.g. a scratch test writing to the same file, or
`fuel-station mcp` stdio `card_add` if MCP tools have landed) and confirm the board
updates within a second. Scratch seeding code is not committed.
Manual (user): same live-update check against the real db; palette tuning.

## Out of scope
Card detail, activity feed, agents strip, Add Project button, import button, drag-and-drop.

Tier:  2 implementer-high. Why: iced 0.14 layout and subscription API read from source; visual judgment.
Gate:  cargo fmt/clippy/test + launch screenshot. Why: rendering is manual.
Scope: src/ui (2-3 files). Asks: none
