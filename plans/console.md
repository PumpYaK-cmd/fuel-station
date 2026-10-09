# console: no console window for the GUI (card 7efd1979)

base: fdde8f5

## Context
`fuel-station.exe` is a console-subsystem binary, so starting it from Explorer opens a
console next to the window. The `mcp` subcommand talks over piped stdio; `install` and
`import-trello` print to the console they were started from.

## Design
`src/main.rs` only.
- Top of file: `#![cfg_attr(windows, windows_subsystem = "windows")]`.
- A GUI-subsystem process started with piped handles (Claude Code starting `mcp`) still
  gets them through STARTUPINFO, so `mcp` needs nothing.
- For every other invocation with arguments (`install`, `import-trello`, `--help`, clap
  errors), attach to the parent console before `Cli::parse()` so output is visible:
  ```rust
  #[cfg(windows)]
  fn attach_parent_console() {
      unsafe extern "system" {
          fn AttachConsole(process_id: u32) -> i32;
      }
      const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
      unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
  }
  ```
  Call it in `main` when `std::env::args_os().nth(1)` is `Some` and is not `"mcp"`.
  No crate; kernel32 is linked by std. Return value ignored (no parent console means
  nothing to print to, same as from Explorer today).
- If `println!` output does not appear in the parent PowerShell after attaching, say so in
  the report with what you saw; do not add a crate without saying why.

## Verification
Gate. Manual (implementer runs these, reports output):
1. `cargo build`, then `target\debug\fuel-station.exe --help` from PowerShell prints help.
2. `target\debug\fuel-station.exe import-trello --dir <empty temp dir>` prints its result
   or error line.
3. Pipe an MCP `initialize` request into `target\debug\fuel-station.exe mcp --project XX`
   (any key): a JSON-RPC response or error comes back on stdout.
Manual for the user: double-click the exe in Explorer, no console window appears.

## Out of scope
Changing how `install` reports errors.

Tier:  1 implementer. Why: one file, code given in the plan.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, plus the three manual runs above. Why: subsystem behaviour isn't covered by tests.
Scope: 1 file (src/main.rs), affects process startup for every subcommand. Asks: none
