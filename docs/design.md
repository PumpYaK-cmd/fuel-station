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

Truck spawns Fuel and Mechanic through the Agent tool. Station never starts a process;
it tracks. Every agent reports through Station's MCP tools, and the GUI shows it live.
Station launching agents itself is a later card (v2), not v1.

## Architecture

One crate, one binary, `fuel-station.exe`:

- `fuel-station` → GUI (iced 0.14).
- `fuel-station mcp --project <KEY>` → stdio MCP server (rmcp 3.5). Claude Code starts one
  per session from the project's `.mcp.json`. Subagents inherit the parent's MCP tools, so
  Fuel and Mechanic share Truck's connection.

Both modes open the same SQLite database: `%APPDATA%\FuelStation\station.db`, WAL mode,
`busy_timeout` 5000 ms on every connection. The MCP server works with the GUI closed.
The GUI polls `PRAGMA data_version` on one long-lived connection every 300 ms and reloads
the visible board when it changes (it changes only on commits from other connections, so
the GUI refreshes itself after its own writes).

Modules: `store` (schema, migrations, all card operations), `mcp` (tools and prompts over
`store`), `ui` (iced), `setup` (Add Project, agent install), `import` (trello boards).
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

MCP prompts appear in Claude Code as `/station:<name>`. They replace the skills and
`CONTRIBUTING.md`, and ship with the binary, so projects need no runbook file:

- `/station:grab [card]`: Truck runs one card end to end (claim, plan onto the card,
  triage block, implement or hand to Fuel, Mechanic review, ship).
- `/station:farm [n]`: Truck plans a batch and spawns one Fuel per card in worktrees.
- `/station:review`: spawn Mechanic on the current card's `base..head`.
- `/station:card <text>`: file a card from a sentence.

## Agents

Station installs `fuel.md`, `fuel-high.md`, `mechanic.md` into `~/.claude/agents/` on first
run and when their bundled version changes (they carry a `# station-agent v<N>` line, and a
user-edited file without it is left alone). They are the current `implementer`,
`implementer-high`, `reviewer` contracts plus the Station reporting rules (checkin,
`report` at each milestone, `submit_review` or `review_result` at the end). Truck has no
agent file; its behaviour is in the prompts.

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
     "use the station MCP tools; runbook is `/station:grab`".
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
dark blue palette instead of Catppuccin.

```
┌──────────────┬────────────────────────────────────────────────────────────┐
│  FUEL STATION│ FS · Fuel Station                     ● 3 agents  ⌕ filter │
├──────────────┼──────────┬──────────┬──────────┬──────────┬────────────────┤
│ ▸ fuel-stat. │ BACKLOG 4│ READY 3  │ DOING 2  │ REVIEW 1 │ DONE 18        │
│   shototsu   │ ┌──────┐ │ ┌──────┐ │ ┌──────┐ │ ┌──────┐ │ ┌──────┐       │
│   forge-load │ │FS-14 │ │ │FS-9  │ │ │FS-7  │ │ │FS-6  │ │ │FS-5  │       │
│   kitsu-pen  │ │title │ │ │title │ │ │ fuel │ │ │ mech │ │ │      │       │
│              │ └──────┘ │ └──────┘ │ └──────┘ │ └──────┘ │ └──────┘       │
│              │          │          │          │          │                │
│ + Add Project├──────────┴──────────┴──────────┴──────────┴────────────────┤
│              │ ACTIVITY  19:42 fuel   FS-7  gate green, submitting         │
│              │           19:40 mech   FS-6  2 findings                     │
├──────────────┴────────────────────────────────────────────────────────────┤
│ station.db · data_version 1182                               theme ▾      │
└───────────────────────────────────────────────────────────────────────────┘
```

- **Sidebar**: projects as outlined buttons; selected = accent outline. `+ Add Project` at the
  bottom.
- **Board**: five bordered columns with a count. Cards are outlined boxes: key, title, tier
  tag, and the glyph and name of the agent holding it (truck, gas pump, wrench Nerd Font
  glyphs). `[HUMAN]` cards get a warning outline.
- **Card detail**: side panel on click: title, body, plan (iced `markdown` widget),
  comments, events, move buttons, edit fields.
- **Activity**: live event feed for the project, newest first.
- **Footer**: db status and theme picker, as pumpbin has.
- Moving cards in v1: buttons in the detail panel and `←`/`→` shortcuts. Drag-and-drop is a
  backlog card.

### Style rules (from pumpbin)

- Font: JetBrains Mono Nerd Font, bundled, used for all text; Nerd Font glyphs instead of
  icon images.
- Buttons: no fill, 1 px border, text and border in `primary`; selected uses `accent`;
  hover lifts the fill to `surface_hover`.
- Panels: `container::Style::default().with_border(border, 1)`; rules are
  `rule::horizontal(1)` / `rule::vertical(1)` (iced 0.14: the arg is drawn thickness).
- Padding and spacing 20 between regions, 3 to 10 inside lists.
- Square-ish corners: radius 2.

### Palette: "Station Night" (tune in the spike)

| Token          | Hex       | Use |
|----------------|-----------|-----|
| background     | `#0A0E14` | window |
| surface        | `#0F1520` | columns, panels |
| surface_hover  | `#151D2B` | hovered card / button |
| border         | `#1C2636` | panel and card outlines |
| border_strong  | `#2A3A52` | focused panel |
| text           | `#C8D3E6` | body |
| text_muted     | `#5E6E88` | timestamps, counts |
| primary        | `#3D8BFF` | buttons, links, outlines of actionable items |
| accent         | `#6CC4FF` | selected project, active card, agent glyphs |
| success        | `#2FD3A0` | Done, gate green |
| warning        | `#F0B44C` | `[HUMAN]`, Review |
| danger         | `#FF5C6C` | failed gate, findings |

Built with `Theme::custom("Station Night", Palette { background, text, primary, success,
warning, danger })`; `border`, `surface*`, `text_muted` and `accent` are constants in
`ui::theme` used by the style functions.

## Verify gate

`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
`store` and `import` get unit tests (temp-dir databases). UI and MCP-in-Claude behaviour are
manual checklists on their cards.

## v1 scope

In: store, MCP tools and prompts, agent files, GUI (sidebar, board, card detail, activity,
live refresh, theme), Add Project, install, trello import, cutover of the global workflow.

Out (backlog): drag-and-drop, search and filter, toasts, tray icon, Station launching
agents, per-card time/cost stats, multi-window.
