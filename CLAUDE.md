# Fuel Station

Rust desktop app (iced 0.14) + stdio MCP server (rmcp 3.5) over one SQLite db. Replaces the
trello CLI workflow. Spec: `docs/design.md`; read the section a card points at before planning.

## Rules
- One binary, subcommands: none = GUI, `mcp --project <KEY>`, `install`, `import-trello`.
- Only `store` writes SQL. UI and MCP call `store` functions.
- Never point `.mcp.json` at `target/`: running MCP servers lock the exe (see design.md, Windows exe lock).
- The exe is GUI-subsystem on Windows: child processes use `creation_flags(0x0800_0000)` (CREATE_NO_WINDOW) or each opens a console window.
- Window is undecorated: the title bar is `ui::titlebar`; drag and window controls go through `window::{drag, minimize, toggle_maximize, close}` with `App::window`.
- UI colors come from `ui::theme` tokens, never inline hex.
- iced 0.14 rules: `rule::horizontal(1)` / `rule::vertical(1)`; the arg is drawn thickness.

## Board & gate
- Board: `trello --backend local --board fuel-station` · lists Backlog / Ready / Doing / Done · `[HUMAN]` cards need the user's hands: don't grab them.
- Runbook: `CONTRIBUTING.md`. Ship mode: git. Default branch main, no remote yet.
- (git) Worktrees (`/farm` only): `../fuel-station-wt`. Bootstrap in a fresh worktree: none.
- Verify gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`. Anything the gate can't cover → manual checklist in the plan.
