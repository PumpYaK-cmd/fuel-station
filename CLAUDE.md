# Fuel Station

Rust desktop app (iced 0.14) + stdio MCP server (rmcp 3.5) over one SQLite db. Replaces the
trello CLI workflow. Spec: `docs/design.md`; read the section a card points at before planning.

## Rules
- One binary, subcommands: none = GUI, `mcp --project <KEY>`, `install`, `import-trello`.
- Only `store` writes SQL. UI and MCP call `store` functions.
- Never point `.mcp.json` at `target/`: running MCP servers lock the exe (see design.md, Windows exe lock).
- The exe is GUI-subsystem on Windows: child processes use `creation_flags(0x0800_0000)` (CREATE_NO_WINDOW) or each opens a console window.
- Window is undecorated: the title bar is `ui::titlebar`; drag and window controls go through `window::{drag, minimize, toggle_maximize, close}` with `App::window`.
- State shared between the GUI and `mcp` never lives under AppData: Claude desktop (MSIX) redirects its children's AppData writes (design.md, MSIX AppData redirect).
- UI colors come from `ui::theme` tokens, never inline hex.
- After `ship` on this project, run `pwsh scripts/update.ps1`: builds release, installs to `bin\` and `app\`, restarts an open GUI.
- iced 0.14 rules: `rule::horizontal(1)` / `rule::vertical(1)`; the arg is drawn thickness.

<!-- station -->
Station project `FS`. Use the station MCP tools for the board; runbook is `/grab`.
<!-- /station -->
