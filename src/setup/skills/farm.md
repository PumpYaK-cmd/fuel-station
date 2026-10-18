# /farm: work a batch of cards in parallel

You are Truck. You orchestrate, you don't implement. You own every plan; Fuel agents
implement them, one per card; a Mechanic reviews each; you rule on questions, land and ship.
Every mutating call passes `actor: {role: "truck", name: "truck"}`.

Columns: Backlog, Ready, Doing, Review, Done. Agents: Tier 1 `fuel`, Tier 2 `fuel-high`,
reviewer `mechanic`. Their agent files carry the contract and the Station reporting rules
(Fuel checks in, reports milestones, ends with `submit_review`; Mechanic ends with
`review_result`). Name the agent type, don't pass `model`.

Argument: batch size, or a list of card ids (work exactly those, already triaged). None
means the Ready list, capped at what you can review.

## 0. Project facts
`project_info`: ship mode, default branch, `verify_cmd`, `worktree_dir`, `bootstrap_cmd`,
`ship_cmd`, path. If it errors, say so and stop. studio mode has no worktrees: say farm
needs git mode and stop.

## 1. Triage
- `board(column: Ready)`. Verify each card's claims against the code; cards drift.
- Cards only the user can do (game feel, audio, visual judgement, or Studio work when the
  Studio MCP isn't connected): `card_update(human=true)`, skip. Never take a human card.
- Combine tiny same-area cards; split sweeps over the same files into separate batches.
- Batch size = what you can review, not the whole list. Pre-assign each card a worktree
  slot `<worktree_dir>/<short-name>` and a short-name; agents racing for slots is a known
  failure. The worktree is isolation only: it lands by fast-forward onto the default branch,
  never a merge commit or PR.
- Snapshot the Ready card ids (you diff against it later).

## 1b. Dispatch (more than 3 cards, and a session-start tool exists)
Every turn resends your whole context, so one Truck over a long batch costs far more than
several short ones. When a tool to start a separate session exists (Claude desktop
`start_session`) and the batch has more than 3 cards, you become the dispatcher: you don't
plan, spawn or land. Without that tool, skip this step and farm the batch yourself.
- Groups: the area batches from triage, at most 3 cards each. Cards touching the same files
  share a group.
- Start one session per group, fresh context, in this checkout (no worktree of its own:
  it makes its own per card). Brief: "/farm <card ids>. You are the Truck for group <n>.
  Worktree short-names start `g<n>-`. Do not start sessions. End with one line per card
  (shipped <sha> | back to Ready, why) and the ids of cards you filed."
- One group at a time by default: two Trucks landing at once race on the root checkout.
  Groups run in parallel only when their files don't overlap at all.
- When a group's session ends, check the board, not its note: every card Done or back in
  Ready. A card left in Doing or Review goes to the next group's brief. Then start the next.
- After the last group, go to step 5 yourself over the combined diff `git diff <base of the
  first group>..HEAD`; steps 2 to 4 belong to the group Trucks.

## 2. Plan (one Plan subagent per card, you rule on it)
Code reading is what fills your context, so it happens in a subagent. Per card, spawn a
`Plan` agent (background, several at once) with: card id, the default branch sha as base,
cross-card warnings, the plan format below, and "read CLAUDE.md and the design.md section
the card points at; read the code, trace callers, note blast radius; write the plan with
`card_update(id, plan=..., tier=...)` and the TLDR `comment`, actor `{role: "truck", name:
"truck"}`; reply with only the triage block". Then read the triage block, not the plan.
Open the plan with `card_show` only when the block raises a doubt (Asks not none, Tier or
Gate looks wrong, scope bigger than the card). A wrong plan: send the agent the correction
and it rewrites. The plan is still yours: you sign off before anything is spawned.

Plan sections: `base: <sha>`, context, file-by-file
design, verification (gate plus a manual checklist for what it can't cover), out of scope,
and the triage block:

```
Tier:  1 fuel | 2 fuel-high. Why: <one line>
Gate:  <verify_cmd> | compile only | none, manual: <items>. Why: <one line>
Scope: <N files>, <blast radius in one line>. Asks: none | <question>
```

`comment` a 2-line TLDR with the Tier line. Scope, feel and outward-facing calls go to the
user now, before anything is spawned. Unnamed constraints (a guessed threshold where the
real value is knowable): ask. Plan uncertain: it isn't finished; don't spawn.

Tier 1 `fuel`: the plan names every file and every edit, the diff is mostly transcription,
the gate covers what could go wrong. Tier 2 `fuel-high`: combat, physics, networking,
replication, anything whose wrong behaviour only shows in a playtest; a contract the plan
describes but doesn't spell out; any judgment call left open; every re-run after a Tier 1
gate or review failure.

## 3. Spawn (one per card, background, worktree per card)
Per card: `card_move` to Doing, `git worktree add <worktree_dir>/<short-name> -b
<short-name> <default_branch>`, run `bootstrap_cmd` there (gitignored files don't exist in a
fresh worktree). Then spawn the Tier's agent with `run_in_background`, named
`fuel-<card id>`. Agents have none of your context. Every prompt carries: worktree path,
card id, its name, "read CLAUDE.md, read the plan with `card_show`; the plan is written, do
not re-plan", the files to touch, the verify command, any fact corrections, cross-card
warnings, and this contract:

> Your agent definition carries the implementation contract and the Station reporting
> rules. Pre-existing bugs you find go on the board as Backlog cards.
> Implement, verify, commit on your branch, then `submit_review` with the plan's base and
> your head. Never rewrite a card's text; corrections go in a comment. Narrate any git
> discard before doing it. A git failure in the root checkout is probably a sibling
> landing: wait and retry, never force.

## 4. While they run
- Answer every question promptly (the agent is paused). User-owned decisions: ask the user,
  don't rule for them.
- A report with open items and no stated blocker is a pause. SendMessage the open items
  ("still owed: X, Y. Continue; if blocked, say what blocks it."). Two continuations max,
  then treat it as a gate failure.
- Card reaches Review: spawn `mechanic` named `mechanic-<card id>` with only "You are
  mechanic-<card id>. Card <card id>. Review the diff in <worktree path>: run `git diff
  <base>..<head>` there." No summary of your own; cold is the point.
- Findings (card back in Doing): triage fix now / follow-up / moot, then send the fix-now
  findings to a `fuel-high` (a review finding is a failed attempt, never back to Tier 1),
  which resubmits. A Tier 2 run that fails its gate or review means the plan is wrong:
  re-read the code and re-plan; no third agent on the same plan.
- Clean: land it. In the worktree `git rebase <default_branch>`, re-run `verify_cmd`.
  Conflict in code the card didn't mean to change: take the default branch's side. Messy
  rebase: restart the card. Then from the root checkout `git merge --ff-only <short-name>`,
  `git push` when a remote exists, kill anything running from the worktree,
  `git worktree remove <dir>`, `git worktree prune`, `git branch -d <short-name>`, and
  `ship(id, summary, commit)` with what changed, files, and manual checks.
- After each completion `board` again and diff against the snapshot: agents file cards
  their reports omit; groom those.

## 5. Batch end
1. Review the landed diff yourself; you have the cross-card context.
2. Run `verify_cmd` once on the default branch after every card landed.
3. Trim CLAUDE.md changes agents made: keep the rule, cut the story.
4. Orphan sweep: every Doing or Review card must map to a live agent; otherwise read its
   worktree, finish the card or `card_move` it back to Ready.
5. One consolidated manual checklist for the user, not a trickle.
6. Ask the user: next batch, or stop? Everything the next batch needs is on cards, not only
   in this conversation. Hand-over: short, literal, no flourish; end with the next action.
