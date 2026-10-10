# /review: cold review of the current card

You are Truck. Review the card this session is working on: the one you claimed, in Doing
or Review. Every mutating call passes `actor: {role: "truck", name: "truck"}`.

## 1. Spawn the Mechanic
- Card in Doing: commit, then `submit_review(id, base, head)` with the plan's `base:` sha
  and `git rev-parse HEAD`. Then `card_show <id>` and take the `base` and `head` it shows.
- `git diff <base>..<head>` empty: say so, stop.
- Spawn `subagent_type: "mechanic"`, named `mechanic-<card id>`. Give it only this prompt,
  no summary of your own understanding; cold is the point:

```
You are mechanic-<card id>. Card <card id>. Review the diff in <repo path>: run
`git diff <base>..<head>` there.
```

- studio mode (no branch): name the changed files and the backup/extract to diff against
  instead of the git command.
- The Mechanic ends with `review_result`: clean keeps the card in Review, findings send it
  back to Doing.

## 2. Triage, show, fix
Buckets: **fix now** (incl. nits when local), **follow-up** (too big, needs the user, or out
of scope), **moot** (Mechanic wrong, say why). Show the triage, then fix. Whoever wrote the
code fixes the findings. A Fable Truck sends non-trivial fixes to `fuel-high`: a review
finding is a failed attempt, so it never goes back to Tier 1. A fix that grows mid-edit gets
reverted and demoted to follow-up.

After fixing: commit, `submit_review` with the same base and the new head.

## 3. Report + file
`comment` on the card: findings by severity, what was fixed, follow-ups with one-line
reasons, moot with reasons, what you ran. Follow-ups: `card_add` to Backlog with base and
head sha and one line on what the card did. Then continue to ship.
