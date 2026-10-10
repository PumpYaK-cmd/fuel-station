# Fuel Station

A card board for driving Claude Code. One Windows app with a GUI and an MCP server over a
local SQLite database. You file cards; Claude claims one, writes a plan onto it, implements
it (or hands it to a `fuel` subagent), gets it cold-reviewed by a `mechanic` subagent, and
ships it. The GUI shows every step live.

Windows only. Design and internals: [docs/design.md](docs/design.md).

## Requirements

- Windows 10/11
- [Rust](https://rustup.rs) (stable, MSVC toolchain; rustup installs the Visual Studio
  Build Tools prompt if they are missing)
- Git
- [Claude Code](https://docs.claude.com/en/docs/claude-code) with the `claude` CLI on `PATH`
  and logged in

## Install

Run these in a normal terminal (PowerShell or Windows Terminal), **not** from a session
inside the Claude desktop app: the desktop app is an MSIX package that redirects AppData
writes, and `install` refuses to run there. Claude Code in a terminal is fine.

```bash
git clone https://github.com/PumpYaK-cmd/fuel-station.git
```

```bash
cd fuel-station
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts/update.ps1
```

`update.ps1` builds release and runs `fuel-station install`, which:

- copies the exe to `%LOCALAPPDATA%\FuelStation\bin\fuel-station.exe` (projects point at
  this copy, never at `target/`),
- installs the skills `/grab`, `/farm`, `/review`, `/card` into `~/.claude/skills/`,
- installs the agents `fuel`, `fuel-high`, `mechanic` into `~/.claude/agents/`.

Skills and agents carry a `# station-skill v<N>` / `# station-agent v<N>` marker line. A newer build overwrites them;
delete the marker line in a file to keep your own edits.

The board lives in `%USERPROFILE%\.fuel-station\station.db`.

Start the GUI:

```powershell
& "$env:LOCALAPPDATA\FuelStation\bin\fuel-station.exe"
```

Pin it to the taskbar from there.

## Add your project

1. In the GUI sidebar, **+ Add Project** and pick the repo folder.
2. Station detects the stack (Cargo, npm, Rojo, Python) and pre-fills the setup sheet:
   key (e.g. `ABC`, cards become `ABC-1`), ship mode, default branch, verify gate (the
   command that must pass before a card ships), worktree dir, bootstrap and ship command.
   Check the verify gate; it is the one value that matters most.
3. **Set up** writes:
   - `.mcp.json`: a `station` server entry pointing at the installed exe,
   - `CLAUDE.md`: a short `<!-- station -->` block naming the project key.

   Shared team repo where you don't want to commit those? Tick **Local only**: the block
   goes into `CLAUDE.local.md` (git-excluded) and the server is registered with
   `claude mcp add --scope local` instead.
4. Open Claude Code in the project. It asks once to approve the `station` MCP server; approve it.

## Use

In a Claude Code session in the project:

| Command | Does |
|---|---|
| `/card <sentence>` | Files a Backlog card |
| `/grab` | Takes the top Ready card end to end: plan, implement, review, ship |
| `/grab ABC-12` | Same, for a specific card |
| `/farm` | Plans a batch of Ready cards and runs one `fuel` agent per card in worktrees |
| `/review` | Cold `mechanic` review of the current card's diff |

Move cards from Backlog to Ready in the GUI when they are worth doing. Mark a card
**human** to keep agents off it. The card's RUN section can also start `/grab` headless in
the project (`claude -p`, auto permissions, $10 budget cap per run).

One card per session: after a card ships, start a new session.

## Update

```bash
git pull
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts/update.ps1
```

This rebuilds, reinstalls, refreshes skills and agents, and restarts the GUI if it was open.
Open Claude sessions keep the old MCP server until restarted.

## Setting this up with Claude

Point Claude Code (in a terminal, not the desktop app) at this repo and say:

> Read README.md and install Fuel Station for me, then tell me what to click to add my project at `<path>`.

Claude can check the requirements and run the install. The **Add Project** step is in the
GUI and is yours to click, as is approving the MCP server in your project.
