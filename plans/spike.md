# Spike: cargo skeleton, Station Night theme, rmcp hello (card 0ba2ff5f)

base: <filled by agent>

## Context
The crate skeleton is already committed: `Cargo.toml` (all v1 deps), `src/main.rs` (clap,
`None` -> `ui::run()`, `mcp --project X` -> `mcp::run(&project)`), empty `store`, `setup`,
`import` modules, stub `ui::run` and `mcp::run`. This card fills `ui` and `mcp` only.
The Store card runs in parallel and owns `src/store/`; do not touch it.

API references (read the source, not memory):
- iced 0.14: `~/.cargo/registry/src/*/iced-0.14.0`, `iced_widget-0.14.2`. Use the
  `iced::application(...)` builder (`.theme`, `.font`, `.default_font`, `.subscription`).
- rmcp 3.5.1: `~/.cargo/registry/src/*/rmcp-3.5.1/tests/test_tool_macros.rs`,
  `test_prompt_macros.rs`, `test_prompt_routers.rs`, `src/transport/io.rs` (stdio).

## Design
- `assets/fonts/JetBrainsMonoNerdFont-Regular.ttf`: copy from
  `%LOCALAPPDATA%\Microsoft\Windows\Fonts\JetBrainsMonoNerdFont-Regular.ttf`. Commit it.
- `src/ui/theme.rs`: pub consts for every token in design.md > Palette (`Color` via
  `iced::color!`): BACKGROUND, SURFACE, SURFACE_HOVER, BORDER, BORDER_STRONG, TEXT,
  TEXT_MUTED, PRIMARY, ACCENT, SUCCESS, WARNING, DANGER. `pub fn theme() -> Theme` =
  `Theme::custom("Station Night", Palette {..})`. `pub const FONT: Font` = the Nerd Font by
  family name. Style fns that every later UI card uses:
  - `button(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style`: no fill,
    1px border radius 2, text+border PRIMARY, ACCENT when selected, background
    SURFACE_HOVER on Hovered.
  - `panel(&Theme) -> container::Style`: SURFACE background, BORDER 1px, radius 2.
  - `card_box(warning: bool) -> impl Fn(&Theme) -> container::Style`: as panel, border
    WARNING when `warning`.
- `src/ui/mod.rs`: `pub fn run()` starts the iced app with `theme::theme()`, the font loaded
  via `.font(include_bytes!(..))` and `.default_font(theme::FONT)`. One screen (`swatch.rs`):
  a grid of every token (filled square + name + hex, text in that color), an outlined button
  normal, one selected (hover is live), one bordered panel with text, and a
  `horizontal_rule(0)` / `vertical_rule(0)` per design.md > Style rules. Window title
  "Fuel Station". The GUI shell card replaces this screen later; keep it in one file.
- `src/mcp/mod.rs`: `pub fn run(project: &str)` builds a tokio multi-thread runtime and
  serves `Station { project: String }` over `rmcp::transport::stdio()`, waits for close.
  `#[tool_router]` with one tool `ping` -> text `"pong from station (project <KEY>)"`.
  `#[prompt_router]` with one prompt `hello` (no args) -> one user message
  "Call the station ping tool and report what it returned." Server info: name "station",
  tools + prompts capabilities. Nothing on stdout but the protocol; logs to stderr.
- No other files.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Also: `cargo run` opens the window (launch, close; report what rendered, a screenshot if you
can). Pipe an `initialize` + `notifications/initialized` + `tools/call ping` + `prompts/list`
exchange into `target/debug/fuel-station.exe mcp --project X` and paste the responses in
the report.

Manual (user): register a copy outside `target/` in a scratch project's `.mcp.json`; ping
works, `/station:hello` appears, a subagent can call ping, whether
`tools: mcp__station__ping` in an agent file restricts it. Tune palette hex on the swatch.

## Out of scope
Store, board UI, any other tool.

Tier:  2 implementer-high. Why: iced 0.14 and rmcp 3.5 APIs must be read from source, judgment on API shape.
Gate:  cargo fmt/clippy/test + launch check. Why: UI is visual, the gate is compile-level for it.
Scope: ~5 files in src/ui, src/mcp, assets. Asks: none
