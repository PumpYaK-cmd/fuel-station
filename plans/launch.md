# launch: Station launches Truck on a card with live output and stop (card 88ae4d5c)

base: (set at claim)

## Context
design.md says Station never starts a process (v1). This card is the v2 design pass and
implementation. Truck's runbook is the MCP prompt `/station:grab <card>`
(`src/mcp/prompts/grab.md`); Truck checks in, reports and ships through the station MCP
tools, so the board, activity feed and agents strip already show progress. What Station
adds: start the session, show its raw output, stop it, show its cost.

## Rulings (design pass)
- **What runs**: `claude -p "/mcp__station__grab <KEY>" --output-format stream-json --verbose
  --permission-mode auto --max-budget-usd <RUN_BUDGET_USD>` in the project's `path`, plus
  `--mcp-config <path>/.mcp.json` when that file exists (headless sessions don't show the
  project MCP approval prompt). One run per card; several cards may run at once.
- **Auth**: whatever the user's `claude` CLI is logged into. Station stores no keys.
- **Permissions**: `auto` mode, the same mode the user runs interactively; denials show in
  the run log. No `bypassPermissions`.
- **Cost**: `--max-budget-usd 10` per run (`RUN_BUDGET_USD: f64 = 10.0`, unit USD, one
  constant). The final `result` line's `total_cost_usd` shows in the run header.
- **Process**: spawned with `CREATE_NO_WINDOW` (Station is a GUI-subsystem exe), stdout
  piped, stderr piped into the same log, `kill_on_drop(true)`. Stop = `taskkill /PID <pid>
  /T /F` (claude starts child processes: node, cargo, git) and then abort the stream.
  Closing Station stops every run the same way: `impl Drop for Run` kills the tree when
  the run is still `Running`, so no close path needs to know about runs.
- **Not stored**: runs live in memory only. The durable record is what Truck writes through
  MCP. The launch itself is recorded as a `report` event on the card ("launched Truck",
  "Truck run stopped", "Truck run exited: <subtype>, $<cost>") via `Store::report` with
  actor `manager`.
- **UI**: in the card detail panel, a RUN section under the move buttons: `Launch Truck`
  (enabled when the project has a path, the card is not human, not in Done, and no run is
  active for it) or `Stop`; a status line (`running`, `exited 0 · $1.23 · 14 turns`,
  `stopped`, `failed to start: <err>`); the last 300 log lines in a scrollable anchored
  to the bottom. The board card of a running card shows `▶ truck` in `accent` under the
  title (unless it already shows a holder).

## Design
New `src/launch.rs` (no iced, no SQL; declared in `main.rs`):
- `pub const RUN_BUDGET_USD: f64 = 10.0;`
- `pub fn command(dir: &Path, card_key: &str) -> tokio::process::Command` building the
  invocation above (Windows: `creation_flags(0x0800_0000)`).
- `pub enum Line { Text(String), Done { subtype: String, cost: Option<f64>, turns:
  Option<i64> } }` and `pub fn parse(line: &str) -> Vec<Line>` for one stream-json line:
  `system/init` → `Text("session started")`; `assistant` content blocks: `text` → its
  text (each line of it), `tool_use` → `"→ <name> <first 80 chars of compact input
  JSON>"`; `user` `tool_result` with `is_error: true` → `"← error <first 120 chars>"`,
  other tool results skipped; `result` → `Done { subtype, total_cost_usd, num_turns }`;
  non-JSON → `Text(line)`; unknown types skipped. Unit tests with literal sample lines
  for each case.
- `pub fn kill_tree(pid: u32)`: `std::process::Command::new("taskkill")
  .args(["/PID", &pid.to_string(), "/T", "/F"])` with `CREATE_NO_WINDOW`, output ignored.

`src/ui/runs.rs` (new) + `src/ui/mod.rs` + `src/ui/detail.rs`:
- `pub struct Run { pid: Option<u32>, lines: VecDeque<String> (cap 300), status: Status,
  handle: iced::task::Handle }`, `enum Status { Running, Exited { subtype, cost, turns },
  Stopped, Failed(String) }`. `App.runs: HashMap<String, Run>` keyed by card key.
- `pub fn start(dir: PathBuf, key: String) -> (Task<Message>, Handle)`: a
  `Task::run(stream, ..)` over an `iced::stream::channel` that spawns `command`, sends
  `RunEvent(key, Started(pid))`, then one `RunEvent(key, Out(Line))` per parsed line from
  stdout and stderr (merge both readers), then `RunEvent(key, Ended(Option<i32>))`;
  spawn error → `RunEvent(key, FailedToStart(String))`. `.abortable()` for the handle.
  `RunEvent` payload enum is `Clone + Debug`.
- Messages: `Launch`, `StopRun`, `RunEvent(String, RunEvent)`. `Launch` acts on the open
  detail's card; `StopRun` too. `Launch` writes the `report` event; `Ended` writes the
  exit report with subtype/cost if a `Done` line arrived, else the exit code; `StopRun`
  kills the tree, aborts, writes "Truck run stopped", sets `Stopped`.
- Window close: nothing to add in update; `Drop for Run` covers it (App drops when iced
  exits). The guards card is reworking `Message::Close` in parallel; do not touch it.
- `Detail::view` takes `Option<&Run>` and renders the RUN section; card_box gets a
  `running: bool`.
- `docs/design.md`: replace "Station never starts a process; it tracks." and the "Station
  launching agents itself is a later card (v2)" sentence with a short **Launching** section
  stating the rulings above; drop "Station launching agents" from the v1 Out list.

## Verification
Gate (parse tests). Manual for the user (costs money: the user runs these, the implementer
does not launch a real session):
1. Open a Ready card in a set-up project, Launch Truck: status `running`, log fills,
   activity shows "launched Truck", Truck's own checkin/report events follow.
2. Stop: process tree gone (Task Manager: no claude/node/cargo left from it), status
   `stopped`.
3. Let a small card finish: status shows exit, cost and turns; card ends in Review/Done
   per the runbook.
4. Launch on a project without `.mcp.json`: run still starts; log shows the MCP tool
   failure.
The implementer may check spawning with a harmless stand-in: temporarily point
`command` at `cmd /c echo {"type":"result","subtype":"success","total_cost_usd":0.01,"num_turns":1}`
locally to see the UI path, and must not commit that.

## Out of scope
Persisting runs, resuming sessions, launching Fuel/Mechanic directly, per-project budget
settings, `/station:farm` launches.

Tier:  2 implementer-high. Why: process lifecycle, streaming into iced, Windows kill semantics.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: real runs cost money and are the user's to start.
Scope: 6 files (launch.rs new, main.rs mod line, ui/runs.rs new, ui/mod.rs, ui/detail.rs, docs/design.md). Asks: none (budget 10 USD is a ruling, flagged in the hand-over)
