# MCP tools over store (card 349abc13)

base: <filled by agent>

## Context
design.md > MCP server (tool table). Spike landed `src/mcp/mod.rs` (rmcp 3.5 stdio server,
`Station`, a `ping` tool, a `hello` prompt). Store landed `src/store/` with the API every
tool calls (read `src/store/mod.rs` and `model.rs` first; do not change store unless a
tool cannot work without it, and say so in the report). rmcp reference:
`~/.cargo/registry/src/*/rmcp-3.5.1/tests/test_tool_macros.rs`.
Other cards running in parallel: GUI shell (src/ui), Install (src/setup, src/main.rs),
Import (src/import, src/store, src/main.rs). You own src/mcp.

## Design
- `Station { project: String, store: Arc<std::sync::Mutex<Store>> }`. `run()` opens
  `Store::open_default()`; on failure print the error to stderr and exit 1. The project key
  is not checked at startup; tools that need it return the store's NotFound error.
- Remove `ping` (superseded by `project_info`). Keep the `hello` prompt (the prompts card
  replaces it).
- One `#[tool]` per row of the design.md tool table, params as `Parameters<T>` structs
  deriving `Deserialize, JsonSchema` with doc comments as descriptions. Mutating tools take
  `actor: Actor` (store's type). Card ids are `KEY-N` strings. Columns are store `Column`.
  - `project_info()` -> the project row as `key: value` lines.
  - `board(column?: Column)` -> text, one block per column: `Ready (3)` then
    `  FS-3  title  [tier]  {human}  @fuel:name` (omit empty parts).
  - `card_show(id)` -> text: header line `FS-3 [Doing] title`, tier/human/commits line,
    `## Body`, `## Plan`, `## Comments` (`<time> role:name` + body), `## Events` (last 20).
  - `card_add(actor, column, title, body)`, `card_update(actor, id, title?, body?, plan?,
    tier?, human?)`, `card_move(actor, id, column)`, `grab(actor, from?=Ready)` (empty ->
    text "nothing to grab in Ready", not an error), `comment(actor, id, body)`,
    `report(actor, id, message)`, `submit_review(actor, id, base, head)`,
    `review_result(actor, id, verdict, findings: Vec<String>)`, `ship(actor, id, summary,
    commit)`, `checkin(actor, card?)`, `checkout(actor)` -> one-line confirmations, e.g.
    `FS-3 -> Review (base abc123, head def456)`. card_add / grab return the card key and
    title.
  - Times render as `YYYY-MM-DD HH:MM` UTC (`chrono` is in Cargo.toml).
- Card 130f1ac2 folded in: in `Store::card_update`, `tier: Some("")` clears the tier to
  NULL (one store change, plus a store test). The `card_update` tool doc says "empty tier
  clears it".
- Errors: a `StoreError` (or poisoned mutex) becomes `CallToolResult::error` with the
  one-line `Display` text. Never panic, never a protocol error for domain failures.
- Tool descriptions: one sentence each, from the design table; the `actor` doc says
  "who is calling: role is manager|truck|fuel|mechanic, name is your agent name".
- Split if mod.rs passes ~400 lines: `src/mcp/tools.rs` for the tool impls and text
  formatting.

## Tests
A few unit tests on the text formatting helpers (board block, card_show header) with a temp
store (`FUEL_STATION_DB` is not needed: build `Station` with `Store::open(tempdir)`).

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Smoke: with `FUEL_STATION_DB` pointing at a temp file, add a project via a test or a
scratch call, then pipe `initialize`, `tools/list`, `tools/call card_add`, `tools/call
board` into `target/debug/fuel-station.exe mcp --project <KEY>`; paste responses.
Manual (user): run every tool from a Claude session in a scratch project.

## Out of scope
Prompts (own card), agent files, UI.

Tier:  2 implementer-high. Why: rmcp macro API read from source; tool schema shape is the Claude-facing contract.
Gate:  cargo fmt/clippy/test + stdio smoke. Why: in-Claude behaviour is manual.
Scope: src/mcp (1-2 files), one line in store::card_update. Asks: none
