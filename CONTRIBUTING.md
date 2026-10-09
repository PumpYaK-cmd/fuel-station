# CONTRIBUTING — how a card gets shipped

Runbook for one card, start to finish. Read it when told to "take/grab a ticket" or
"follow CONTRIBUTING". Project facts it relies on live in `CLAUDE.md` (or `CLAUDE.local.md`)
→ **Board & gate**: board, ship mode, default branch, worktree dir + bootstrap, verify
commands, ship command. Keep the phase list in your task list so phase 6 doesn't get lost.

Tickets are `trello` cards. `T` below means `trello --backend local --board <board>`.

Two ship modes, declared in Board & gate:
- **git** — code lives in the repo; work on the default branch; ship = commit + push. No
  feature branches, no PRs, no merge commits. Under `/farm` only, each agent works in its own
  worktree so agents don't collide, and lands by fast-forward onto the default branch.
- **studio** — the folder mirrors a Studio place; no branches/worktrees; ship = the project's
  push command, then the user applies in Studio. Skip every worktree line below. If the folder
  is itself in git, what it holds (memory, plans, this runbook) is committed on the default
  branch after shipping and pushed when a remote exists.

## 1. Claim
- Top card: `T grab --from "Ready" --to "Doing"` (atomic — safe with several agents; the card
  you get may not be the one you saw on top, that's the point). Named card:
  `T card move <id> Doing`. Never grab a `[HUMAN]` card.
- `T card show <id>` — description, comments, linked plan. Card is the pointer, the plan is the
  spec.
- git mode: on the default branch, up to date (`git pull --ff-only` when a remote exists).
  Record `git rev-parse HEAD` as `base:` in the plan; `/review` diffs against it.
  `/farm` only: `git worktree add <worktree-dir>/<short-name> -b <short-name> <default-branch>`
  and run the per-worktree bootstrap (gitignored files don't exist in a fresh worktree).
- studio mode: refresh the local mirror first if Board & gate names a sync/extract command.

## 2. Plan
- Read the code the card points at; trace callers; note blast radius. Unknown external facts
  (engine behaviour, library contract) → `/research`, don't guess.
- Write `plans/<short-name>.md`: context, file-by-file design, verification, out of scope.
  A small card gets a 5-line plan — still written.
- Ambiguity: take the reading the card's wording and the code most directly support, state
  it in the card comment, continue. Stop and ask only when a wrong guess would make the work
  useless, or when the plan changes the card's scope, feel, or anything outward-facing.
  Otherwise `T comment add <id> "<2-line TLDR of the approach, pointer to the plan>"` and
  continue. (Under `/farm`, the overseer rules instead.)
- End the plan with the triage block (`~/.claude/orchestration.md` → Triage block: Tier, Gate,
  Scope) and print it before phase 3. The Tier line goes in the card comment TLDR too.

## 3. Implement
- Follow the plan and `CLAUDE.md`'s hard rules. Match local style. No narrative comments.
- New convention, knob or gotcha → one line in `CLAUDE.md` (or the project's memory dir), not
  a paragraph.
- Pre-existing bug or unrelated improvement found on the way: a Backlog card (6.4), never a
  fix in this diff, unless the card cannot work without it.

## 4. Verify
- Run the full verify gate from Board & gate. Must be green. If the gate is only a compile
  check, say so in the hand-over — don't imply it was tested.
- What the gate can't cover (Studio feel, rendering, network) goes in the plan's
  verification section as a manual checklist for the user.
- Never auto-run anything that costs money or touches a live external service.
- Scratch checks are not committed. New test files only where the card asks or the repo
  already keeps tests for this kind of change, sized like their neighbours.

## 5. Review
- git mode: commit (imperative subject, body = why).
- `/review` — fresh reviewer agent, diff only (studio mode: diff = the files you changed vs
  the backup/extract, name them). Fix every finding; anything too big becomes a follow-up card.
- `/farm` only: `git rebase <default-branch>` in the worktree, re-run the gate. Conflict in
  code you didn't mean to change → take the default branch's side. Messy rebase → restart.

## 6. Ship + paperwork
1. git mode: `git push` when a remote exists. `/farm` only: from the root checkout
   `git merge --ff-only <short-name>`, push, kill anything running from the worktree,
   `git worktree remove <dir>`, `git worktree prune`, `git branch -d <short-name>`.
   studio mode: run the ship command from Board & gate; if the push refuses because the live
   source changed, re-sync and re-apply — never force over a teammate's edit. Folder in git →
   commit the memory/plan changes on the default branch and push.
2. Delete `plans/<short-name>.md` — `plans/` is for open work only.
3. `T card move <id> Done`, then `T comment add <id> "<what changed, files, commit hash or
   push result, manual checks for the user>"` — real newlines.
4. Out-of-scope things found on the way → `T card add Backlog "<title>" "<desc>"`
   referencing this card. Don't bury them in commit messages.
5. Hand-over to the user: behaviour delta, files, commit/push, manual checklist. Short.
6. Stop. One card per session.
