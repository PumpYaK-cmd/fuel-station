# Fuel Station: design

Agent-orchestration workbench. Replaces the `trello --backend local` CLI, the
`CONTRIBUTING.md` runbook and the `/grab`, `/farm`, `/onboard` skills with one Rust app
that has a GUI and an MCP server.

## Roles

| Role     | Who                                    | Model                  |
|----------|----------------------------------------|------------------------|
| Manager  | The user                               | -                      |
| Station  | This app: GUI + MCP server + datastore | -                      |
| Truck    | The Claude Code session the Manager talks to. Plans, writes cards, spawns Fuel and Mechanic, rules on questions, ships. | Opus (session model) |
| Fuel     | Implementer subagent. One card, one plan. | `fuel`: Sonnet, medium · `fuel-high`: Opus, medium |
| Mechanic | Cold reviewer subagent. Gets the diff location only. | Opus, medium |

Truck spawns Fuel and Mechanic through the Agent tool. Every agent reports through
Station's MCP tools, and the GUI shows it live.

### Launching

Station can start Truck on a card from the card detail panel (RUN section).
- Command: `claude -p "/grab <KEY>" --output-format stream-json --verbose
  --permission-mode auto --max-budget-usd 10` in the project's `path`, plus
  `--mcp-config <path>/.mcp.json` when that file exists. One run per card; several cards
  may run at once.
- Auth is whatever the user's `claude` CLI is logged into; Station stores no keys.
- Permissions: `auto`, never `bypassPermissions`. Denials show in the run log.
- Cost: `RUN_BUDGET_USD` (10) per run; the final `result` line's cost and turns show in
  the run status.
- Process: `CREATE_NO_WINDOW`, stdout and stderr piped into one log (last 300 lines).
  Stop = `taskkill /PID <pid> /T /F`, then the stream is aborted. Closing Station stops
  every running run the same way.
- Runs live in memory only. The card gets `report` events from `manager`: "launched
  Truck", "Truck run stopped", "Truck run exited: ...".
- Enabled when the project has a path, the card is not human, not in Done, and no run is
  active for it. A running card shows `▶ truck` on the board.

## Architecture

One crate, one binary, `fuel-station.exe`:

- `fuel-station` → GUI (iced 0.14).
- `fuel-station mcp --project <KEY>` → stdio MCP server (rmcp 3.5). Claude Code starts one
  per session from the project's `.mcp.json`. Subagents inherit the parent's MCP tools, so
  Fuel and Mechanic share Truck's connection.

On Windows the exe is GUI-subsystem (no console from Explorer). CLI subcommands attach to
the parent console; interactive cmd/PowerShell don't wait for it, so use
`Start-Process -Wait` / `start /wait` when the exit code matters. Pipes wait as usual.

Both modes open the same SQLite database: `%USERPROFILE%\.fuel-station\station.db`
(`FUEL_STATION_DB` overrides), WAL mode,
`busy_timeout` 5000 ms on every connection. The MCP server works with the GUI closed.
The GUI polls `PRAGMA data_version` on one long-lived connection every 300 ms and reloads
the visible board when it changes (it changes only on commits from other connections, so
the GUI refreshes itself after its own writes).

Modules: `store` (schema, migrations, all card operations), `mcp` (tools over `store`),
`ui` (iced), `setup` (Add Project, agent and skill install), `import` (trello boards).
`store` is the only module that writes SQL.

Crates: `iced` 0.14, `rmcp` 3.5 (`transport-io`, `macros`), `rusqlite` 0.40 (`bundled`),
`rfd` 0.17 (folder picker), `serde`, `schemars`, `tokio`, `clap`.

### Windows exe lock

Every open Claude session holds a running `fuel-station.exe mcp`. Windows will not let
`cargo build` overwrite a running exe. So `.mcp.json` never points into `target/`:
`fuel-station install` copies the build to `%LOCALAPPDATA%\FuelStation\bin\fuel-station.exe`
(a locked exe is renamed to `fuel-station.old.exe`, which Windows allows for a running exe,
and deleted at a later start once its sessions close), and
project setup writes that path.

The MSI (`installer/`, per-user, `pwsh installer/build.ps1`; WiX 5 via `dotnet tool install
--global wix --version 5.0.2`, since v6+ asks for an OSMF EULA) cannot replace a locked
file, so it owns a second copy in `%LOCALAPPDATA%\FuelStation\app\` (Start menu shortcut,
fuel pump icon) and runs that copy's `install` after InstallFinalize. Uninstall removes the
`app\` copy only: `bin\` may be locked by open sessions and `.mcp.json` files point at it;
the database in `%USERPROFILE%\.fuel-station` stays.

`install` also refreshes `app\` when it exists, so a local update needs no MSI:
`scripts/update.ps1` builds release, stops the GUI (processes without an `mcp` argument),
runs `install` and relaunches the GUI if one was open. Open sessions keep the old `mcp` server
until restarted. A manual `install` with the GUI open leaves `fuel-station.old.exe` in `app\`
until the next start; an uninstall before then leaves that file and its folder behind.

### MSIX AppData redirect

Claude desktop is an MSIX package. Writes its child processes (the `mcp` server, Claude's
shell) make under `%APPDATA%` or `%LOCALAPPDATA%` go to a private copy in
`%LOCALAPPDATA%\Packages\Claude_<id>\LocalCache\`, while the GUI started from Explorer uses
the real folder. A db under AppData splits into two boards, so it lives in the user
profile instead. `\\localhost\C$\...` paths bypass the redirect when debugging from inside.

## Data model

```
projects  id, key (e.g. "FS"), name, path, ship_mode (git|studio), default_branch,
          verify_cmd, worktree_dir, bootstrap_cmd, ship_cmd, created_at, archived
cards     id, project_id, number, title, body, plan, column, position, tier,
          human (bool), commit_base, commit_head, created_at, updated_at
comments  id, card_id, actor_role, actor_name, body, created_at
events    id, project_id, card_id, actor_role, actor_name, kind, message, created_at
agents    id, project_id, role, name, card_id, status, started_at, last_seen
```

- Card ids show as `<KEY>-<number>`, e.g. `FS-12`. MCP tools take that form.
- Columns are fixed: **Backlog, Ready, Doing, Review, Done**. Review is new: Fuel submits,
  Mechanic reviews, Truck ships.
- `actor_role` is `manager | truck | fuel | mechanic`.
- **The plan lives on the card** (`cards.plan`, markdown), not in `plans/*.md`. Station
  renders it; Fuel reads it with `card_show` from any worktree; nothing to delete on ship.
  This replaces the "plans live in `plans/`" rule for Station projects.
- Project facts that used to be the `Board & gate` section of `CLAUDE.md` (verify gate,
  branch, worktree dir, ship command) live on the project row and come back from
  `project_info`. `CLAUDE.md` keeps a three-line pointer.

## MCP server (`station`)

Every mutating tool takes `actor: {role, name}`. The MCP connection is shared by Truck and
its subagents, so the server cannot tell callers apart without it.

| Tool | Does |
|------|------|
| `project_info` | Gate, branch, ship mode, worktree dir, bootstrap, ship command |
| `board(column?)` | Compact card list per column |
| `card_show(id)` | Card, plan, comments, last events |
| `card_add(column, title, body)` | New card |
| `card_update(id, title?, body?, plan?, tier?, human?)` | Edit fields |
| `card_move(id, column)` | Move |
| `grab(from="Ready")` | Atomic claim of the top non-human card into Doing (one `IMMEDIATE` transaction) |
| `comment(id, body)` | Comment |
| `report(id, message)` | Progress line → activity feed only |
| `submit_review(id, base, head)` | Doing → Review, records the commit range |
| `review_result(id, verdict, findings)` | Mechanic's verdict: `clean` or findings list |
| `ship(id, summary, commit)` | Review → Done + closing comment |
| `checkin(role, name, card?)` / `checkout` | Agent presence for the agents strip |

## Skills

The runbook ships with the binary as user skills, so projects need no runbook file:

- `/grab [card]`: Truck runs one card end to end (claim, plan onto the card, triage block,
  implement or hand to Fuel, Mechanic review, ship).
- `/farm [n]`: Truck plans a batch and spawns one Fuel per card in worktrees.
- `/review`: spawn Mechanic on the current card's `base..head`.
- `/card <text>`: file a card from a sentence.

Bodies live in `src/setup/skills/*.md`; `setup::skills` wraps each in frontmatter and a
`# station-skill v<N>` line and installs it as `~/.claude/skills/<name>/SKILL.md`.

## Agents

Station installs `fuel.md`, `fuel-high.md`, `mechanic.md` into `~/.claude/agents/`. They
are the implementer and reviewer contracts plus the Station reporting rules (checkin,
`report` at each milestone, `submit_review` or `review_result` at the end). Truck has no
agent file; its behaviour is in the skills.

Agents and skills install on every GUI start and on `fuel-station install`: written when
missing or when the bundled version is newer than the installed marker line; a file without
the marker is user-edited and left alone.

## Add Project

1. Sidebar `+ Add Project` → folder picker (`rfd`).
2. Station inspects the folder: git repo? default branch, remote; `Cargo.toml`,
   `package.json`, `default.project.json` / `*.project.json` (Rojo), `pyproject.toml` →
   suggested verify gate and ship mode; existing `CLAUDE.md`.
3. Setup sheet with every value pre-filled and editable: name, key, ship mode, branch,
   verify gate, worktree dir, bootstrap, ship command.
4. **Set up** writes, idempotently:
   - `.mcp.json`: adds or replaces the `station` entry
     (`command` = installed exe, `args` = `["mcp", "--project", "<KEY>"]`), keeps others.
   - `CLAUDE.md`: adds or replaces a fenced `<!-- station -->` block with the key and
     "use the station MCP tools; runbook is `/grab`".
   - `git init` + `.gitignore` + initial commit when ship mode is git and there is no repo.
5. A result list shows each file written. The same sheet later offers **Repair setup**.

Claude Code asks once per project to approve a new `.mcp.json` server. That prompt is the
user's to click.

## Import

`fuel-station import-trello` (also a button on first run): reads `~/.trello/*/`
(`board.json`, `lists.json`, `cards/*.json`, comments), creates one project per board, maps
Backlog/Ready/Doing/Done, keeps card order, comments and authors, tags `[HUMAN]` titles as
`human`. Project path comes from the board description ("one line + repo path"); a board
without one becomes a project with no path, and the GUI asks for the folder. Read-only on
`~/.trello`.

## UI

iced 0.14, layout and widget style taken from pumpbin (minimal, outlined), with our own
dark blue palette, dressed as a space docking bay. Reference mockup:
`docs/mockups/2-dock.html` (open in a browser for the animations).

```
┌───────────────────────────────────────────────────────────────────────────┐
│ ◆ FUEL STATION                ● 3 agents ◆ fuel-1 FS-7 ◆ fuel-3 FS-9  _ □ ✕ │
├──────────────┬────────────────────────────────────────────────────────────┤
│ fuel-station │ FS · Fuel Station                           Repair setup  │
│ shototsu     │ ┌BACKLOG 4 ━━──┐┌READY 3 ━─────┐┌DOING 2 ─────┐ ...        │
│ forge-load   │ │▌FS-14        ││▌FS-9         ││▌FS-7  [T1]  │            │
│ kitsu-pen    │ │▌title        ││▌title        ││▌◆ fuel-1    │            │
│              │ └──────────────┘└──────────────┘└─────────────┘            │
│ + Add Project│ ACTIVITY LIVE  19:42 │ ◆ FS-7  gate green, submitting      │
├──────────────┴────────────────────────────────────────────────────────────┤
│ station.db · data_version 1182                               theme ▾      │
└───────────────────────────────────────────────────────────────────────────┘
```

- **Window**: undecorated. `ui::titlebar` is the title bar: brand, the agents strip (pulsing
  dot, count, one `glyph name card` per agent), minimise / maximise / close. Pressing the bar
  drags the window, double-click maximises. Minimum size 960×600.
- **Backdrop**: a canvas under everything: 40 px blueprint grid fading out from (50 %, 40 %)
  of the window, and a 200 px light band sweeping top to bottom every 9 s.
- **Sidebar**: projects as outlined buttons; selected = accent outline. `+ Add Project` at the
  bottom. Translucent surface over the backdrop.
- **Board**: five translucent columns with corner brackets instead of an outline; the Doing
  brackets pulse in `primary`. Header: name, count, a 3 px gauge (full at 20 cards; Review
  amber, Done green) that fills in over 1 s on project switch. Cards are opaque boxes with a
  2 px left bar: `accent` when open, `warning` for `[HUMAN]`, flashes `accent` when a card
  first appears, `primary` on hover, else `border_strong`.
- **Card detail**: side panel on click: title, body, plan (iced `markdown` widget),
  comments, events, move buttons, edit fields.
- **Activity**: live event feed for the project, newest first; a blinking `LIVE` tag.
- **Footer**: db status and theme picker, as pumpbin has. The backdrop stays Station Night
  whatever theme is picked.
- Moving cards in v1: buttons in the detail panel and `←`/`→` shortcuts. Drag-and-drop is a
  backlog card.
- **Toasts and tray** (`ui::notify`): Windows toasts (notify-rust), only while the window is
  unfocused, for every project: `submit_review`, `ship`, `review_result` with findings, and
  `card_move` into Review or Done. Title = project name. Polled from `events_after` on the
  live-refresh tick; events older than startup never toast. Tray icon (tray-icon) while the
  GUI runs: left click restores and focuses, menu Show / Quit. Close still quits.

### Motion

One clock: `App::t`, seconds since boot, set from `window::frames()`. Every animation is a
pure function of `t` (sweep, bracket pulse, agent dot, LIVE blink) or of an `Instant` stored
on reload (`App::reloaded` for gauges, `App::appeared` for new-card flashes). No per-widget
animation state. The grid is a `canvas::Cache`, rebuilt when the window size changes; only the sweep and the
brackets redraw every frame.

### Style rules (from pumpbin)

- Font: JetBrains Mono Nerd Font, bundled, used for all text; Nerd Font glyphs instead of
  icon images.
- Buttons: no fill, 1 px border, text and border in `primary`; selected uses `accent`;
  hover lifts the fill to `surface_hover`. Window buttons: no border, `danger` fill on close
  hover.
- Panels: `theme::glass` (translucent surface, no outline); rules are `rule::horizontal(1)` /
  `rule::vertical(1)` (iced 0.14: the arg is drawn thickness).
- Inputs, pick lists, scrollbars: `theme::input` / `editor` / `picker` / `scrollbar`; thin
  4 px scrollbars via `theme::thin()`.
- Padding and spacing 20 between regions, 3 to 10 inside lists. Labels and chrome text 11.
- Square-ish corners: radius 2.

### Palette: "Station Night"

| Token          | Hex       | Use |
|----------------|-----------|-----|
| background     | `#0A0E14` | window, backdrop, card fill |
| surface        | `#0F1520` | title bar, footer |
| surface_hover  | `#151D2B` | hovered button |
| border         | `#1C2636` | card outlines, rules, gauge track |
| border_strong  | `#2A3A52` | brackets, card bars at rest |
| text           | `#C8D3E6` | body |
| text_muted     | `#5E6E88` | timestamps, counts |
| primary        | `#3D8BFF` | buttons, gauges, Doing brackets, hovered card bar |
| accent         | `#6CC4FF` | selected project, open card, agent glyphs, new-card flash |
| success        | `#2FD3A0` | Done, gate green, LIVE |
| warning        | `#F0B44C` | `[HUMAN]`, Review |
| danger         | `#FF5C6C` | failed gate, findings, close hover |
| surface_glass  | surface at 0.93 | columns, activity, detail, setup sheet |
| sidebar_glass  | surface at 0.80 | sidebar |
| bar_grid       | border_strong at 0.5 | backdrop grid |
| sweep          | primary at 0.08 | backdrop sweep band |

Built with `Theme::custom("Station Night", Palette { background, text, primary, success,
warning, danger })`; the rest are constants in `ui::theme` used by the style functions.

## Verify gate

`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
`store` and `import` get unit tests (temp-dir databases). UI and MCP-in-Claude behaviour are
manual checklists on their cards.

## v1 scope

In: store, MCP tools and prompts, agent files, GUI (sidebar, board, card detail, activity,
live refresh, theme), Add Project, install, trello import, cutover of the global workflow.

Out (backlog): drag-and-drop, search and filter, per-card
time/cost stats, multi-window.
