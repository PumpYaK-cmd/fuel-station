# MCP prompts: /station:grab, farm, review, card (card 713117ca)

base: 9b02f45770e9611c7083883374aba1e53b0679a6

## Context
design.md > MCP server (prompts), Roles, Data model ("the plan lives on the card").
Sources to port (read all): `~/.claude/templates/CONTRIBUTING.md`,
`~/.claude/skills/grab/SKILL.md`, `~/.claude/skills/farm/SKILL.md`,
`~/.claude/skills/review/SKILL.md`, `~/.claude/orchestration.md`. MCP tools landed in
`src/mcp` with a placeholder `hello` prompt; rmcp prompt reference:
`~/.cargo/registry/src/*/rmcp-3.5.1/tests/test_prompt_macros.rs`.
Agent files card runs in parallel and defines `fuel`, `fuel-high`, `mechanic` agents with
these reporting rules: Fuel checkins, reports milestones, ends with `submit_review`;
Mechanic ends with `review_result`. Other parallel cards touch src/ui and src/setup. You own
src/mcp prompts.

## Design
- `src/mcp/prompts/{grab,farm,review,card}.md`, loaded with `include_str!`. Replace
  `hello`.
- Prompts and args: `grab` (`card`, optional: card id, else top Ready), `farm` (`n`,
  optional: batch size), `review` (none: current Doing/Review card of this session),
  `card` (`text`, required). Each returns one user message: the md text with the
  argument appended as a final line `Argument: <value>` (or `Argument: none`).
- Port rules, change mechanics:
  - Board ops -> station tools (`board`, `grab`, `card_show`, `card_move`, `card_add`,
    `card_update`, `comment`, `report`). Every call passes `actor {role: "truck", name:
    "truck"}`.
  - Project facts (gate, branch, worktree dir, bootstrap, ship mode/command) come from
    `project_info`, not CLAUDE.md "Board & gate".
  - The plan is written to the card with `card_update(plan=...)`, not `plans/*.md`;
    nothing to delete at ship. Implementers read it with `card_show`.
  - Columns: Backlog, Ready, Doing, Review, Done. Flow: Truck claims (Doing) -> implements
    or spawns Fuel -> Fuel `submit_review` (Review) -> Truck spawns `mechanic` with only
    the repo path and `git diff <base>..<head>` -> `review_result` (findings send it back
    to Doing; fix, resubmit) -> Truck ships (merge/push per ship mode) and calls `ship`.
  - Agent names: Tier 1 `fuel`, Tier 2 `fuel-high`, reviewer `mechanic`; spawn prompts
    give each agent a name (`fuel-<card>`) and the card id.
  - grab.md: CONTRIBUTING phases 1-6 + the grab skill + the orchestration triage block,
    adapted. farm.md: the farm skill + orchestration Delegating rules, adapted (worktree
    slots from `project_info.worktree_dir`). review.md: the review skill with Mechanic.
    card.md: file one Backlog card from the sentence: short imperative title (<60 chars),
    body = what and why in 1-3 lines; read related cards with `board` first to avoid
    duplicates; return the card id.
  - Keep the rules (one card per session, `[HUMAN]` cards skipped, triage block, rebase
    + gate before ship, out-of-scope -> Backlog card, hand-over style). Drop trello CLI
    lines and `plans/` mechanics. No em dashes.
- Card 47223a8d folded in: tools take `Column` lowercase only but print and document
  `Ready`. In `src/store/model.rs` add `#[serde(alias = "Backlog")]` etc. on each `Column`
  variant (capitalised form accepted, schema unchanged) and a tools test that `board`
  accepts `"Ready"`. Prompts may then write columns either way.
- `src/mcp/mod.rs`: `#[prompt_router]` with the four prompts; descriptions one line each.

## Tests
Unit test: each prompt returns non-empty text containing its argument line; no prompt
text contains `trello` or `plans/`.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Smoke: `prompts/list` and `prompts/get grab` over stdio; paste the output head.
Manual (user): `/station:grab` in a scratch project runs a card end to end.

## Out of scope
Agent files, Station launching agents (v2).

Tier:  2 implementer-high. Why: porting runbook text is judgment-heavy; the prompts are the workflow.
Gate:  cargo fmt/clippy/test + stdio smoke. Why: in-Claude behaviour is manual.
Scope: src/mcp (mod.rs + 4 md files), Column serde aliases in store/model.rs. Asks: none
