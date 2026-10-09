# Agent files: fuel, fuel-high, mechanic (card 456577bc)

base: <filled by agent>

## Context
design.md > Agents. Sources: `~/.claude/agents/implementer.md`, `implementer-high.md`,
`reviewer.md` (read them; copy their contract text, do not paraphrase it). MCP tools landed
in `src/mcp` (names: card_show, report, comment, submit_review, review_result, checkin,
checkout). Install landed `src/setup/install.rs` and the `install` subcommand.
Other cards running in parallel: card detail + activity (src/ui), Add Project
(src/setup/{detect,write}.rs, src/ui), prompts (src/mcp). You own `src/setup/agents.rs`,
`src/setup/agents/*.md`, and one call line each in the install subcommand and GUI boot.

## Design
- `src/setup/agents/fuel.md`: frontmatter `name: fuel`, description adapted from
  implementer ("Station implementer. Tier 1: ..."), `model: sonnet`, `effort: medium`, no
  `tools:` line (all tools, MCP included). Body: line `# station-agent v1`, then the
  implementer contract verbatim except "plan" now means the plan field on the card (read
  with `card_show`), then a `## Station reporting` section:
  - Your name is given in the prompt; if not, use `fuel-<card id>`. `actor` on every call
    is `{role: "fuel", name: <your name>}`.
  - First call: `checkin(card)`. Then `card_show(card)` for the plan.
  - `report(card, ...)` one line at each milestone: plan read, implementation done, gate
    result (green, or red with the failing step), commit made.
  - End: commit in your worktree, then `submit_review(card, base, head)` with the plan's
    base sha and your HEAD sha, then `checkout`. A blocking question: `comment(card,
    question)` and end your turn with it; do not checkout.
- `fuel-high.md`: same from implementer-high (`model: opus`, `effort: medium`, its extra
  "rated hard" paragraph).
- `mechanic.md`: from reviewer (`model: opus`, `effort: medium`, tools line = the
  reviewer's tools plus `mcp__station__card_show, mcp__station__review_result,
  mcp__station__report, mcp__station__checkin, mcp__station__checkout`). Reporting: checkin,
  review, then `review_result(card, verdict, findings)` with one string per finding
  (`<file>:<line> <severity>: <summary>`), `clean` when none, then checkout. Its text
  answer stays the numbered list it gives today.
- `src/setup/agents.rs`:
  - `const AGENTS: [(&str, &str); 3]` = (file name, `include_str!`).
  - `pub fn install_agents(agents_dir: &Path) -> io::Result<Vec<(String, &'static str)>>`
    returning (file, action) with action `created` / `updated` / `current` /
    `skipped (user edited)`. Version = the `N` in a `# station-agent vN` line. Missing file
    -> create; no marker line -> skip; installed N < bundled N -> overwrite; else current.
    Creates the dir.
  - `pub fn agents_dir() -> PathBuf`: `%USERPROFILE%\.claude\agents`.
- Calls: the `install` subcommand runs `install_agents(&agents_dir())` after the exe copy
  and prints each line; GUI boot runs it once and puts an error, if any, in the footer
  error slot (no output otherwise).

## Tests (temp dirs)
Fresh dir -> 3 created; rerun -> current; older marker -> updated; file without marker ->
skipped and content unchanged; each bundled file has frontmatter `name:` matching its file
name and contains the marker.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Do NOT run `install` against the real `~/.claude/agents` from the gate; the user does.
Manual (user): `fuel-station install`; the three agents appear in Claude Code's agent list;
a Fuel run checks in and shows in the agents strip; whether the mechanic `tools:` line
admits the station tools (the Spike's manual check answers this; if it restricts wrongly,
drop the line).

## Out of scope
Prompts (own card), removing the old implementer/reviewer files.

Tier:  1 implementer. Why: text port with exact rules given, small fs logic with tests.
Gate:  cargo fmt/clippy/test. Why: install logic is tested; agent behaviour is manual.
Scope: src/setup/agents.rs + 3 md files + 2 call lines. Asks: none
