# Add Project: picker, detection, setup sheet, file writes (card 40ef50ca)

base: <filled by agent>

## Context
design.md > Add Project. Landed before this card: store (incl. `derive_key`), GUI shell
(`src/ui`), install (`setup::install::installed_exe()`), import (`import::import_all`,
`import::trello_dir`). Read them first. This card also carries the import card's GUI part:
the first-run import button and a folder prompt for path-less projects.
Other cards running in parallel: card detail + activity (src/ui: detail panel, activity
feed, header agents strip), prompts (src/mcp), agent files (src/setup/agents*). You own
src/setup/{detect,write}.rs, the sidebar button, the setup sheet and first-run views.
Expect rebase conflicts in `src/ui/mod.rs` Message/App/update; keep both sides.

## Design
- `src/setup/detect.rs`: `pub fn detect(dir: &Path, taken_keys: &[String]) -> NewProject`
  pre-filling every field:
  - name = folder name; key = `store::derive_key(name, taken_keys)`; path = dir.
  - git: `git -C <dir> rev-parse --abbrev-ref HEAD` ok -> default_branch = output (`HEAD`
    or failure -> "main").
  - verify_cmd / ship_mode, first match wins: `Cargo.toml` -> `cargo fmt --check && cargo
    clippy --all-targets -- -D warnings && cargo test`, git; `default.project.json` or any
    `*.project.json` -> "", studio; `package.json` -> `npm run lint && npm test` when both
    scripts exist, else `npm test` when test exists, else "", git; `pyproject.toml` ->
    `py -3 -m pytest`, git; none -> "", git.
  - worktree_dir = `../<folder>-wt` for git, "" for studio; bootstrap_cmd, ship_cmd "".
  - `pub struct Facts { pub is_git: bool, pub remote: Option<String>, pub has_claude_md:
    bool }` from `pub fn facts(dir) -> Facts` (remote via `git remote get-url origin`),
    shown read-only on the sheet.
- `src/setup/write.rs`: `pub fn apply(p: &Project, exe: &Path) -> Vec<Written>`,
  `Written { path: PathBuf, action: String /* "created" | "updated" | "unchanged" |
  "error: .." */ }`. Requires `p.path`. Idempotent:
  - `.mcp.json`: parse (missing -> `{}`; invalid JSON -> error entry, do not overwrite),
    set `mcpServers.station = {"type":"stdio","command": exe, "args": ["mcp","--project",
    KEY]}`, keep every other key, write pretty JSON + newline.
  - `CLAUDE.md`: replace the text between `<!-- station -->` and `<!-- /station -->`
    (inclusive) or append it (blank line before) or create the file. Block body:
    `Station project \`KEY\`. Use the station MCP tools for the board; runbook is
    \`/station:grab\`.`
  - git mode and no `.git`: `git init -b <branch>`; `.gitignore` when missing (`/target`
    for Cargo, `node_modules/` for npm, else no file); `git add -A`; `git commit -m
    "Initial commit"`. Each step one Written entry; failures become error entries.
- UI (src/ui, new file `setup_sheet.rs` for the sheet view + its state):
  - Sidebar bottom: `+ Add Project` (`theme::button(false)`) -> `rfd::AsyncFileDialog::new()
    .pick_folder()` via `Task::perform` -> detect -> sheet.
  - Sheet (replaces the board area): a labelled `text_input` per NewProject field, ship mode
    as `pick_list`, Facts read-only, buttons **Set up** and Cancel. Set up -> `project_add`
    (new) or `project_update` (repair), then `write::apply(&project,
    &installed_exe())`, then a result list (path + action, errors in DANGER) with Done.
    Store errors (bad key, duplicate) show inline, sheet stays open.
  - Project header gets **Repair setup** -> the same sheet prefilled from the row. A
    project with no path shows **Pick folder** on its header instead, which runs the picker
    then opens the repair sheet with detected values merged for empty fields.
  - First run: no projects and `import::trello_dir()` exists -> main area shows
    **Import trello boards** -> `import_all` -> report list (imported key/name/cards,
    skipped with reason).
- Colors from `ui::theme` only.

## Tests (temp dirs)
detect: each manifest kind, studio by `*.project.json`, git branch on a `git init -b trunk`
temp repo. write::apply: fresh dir creates .mcp.json + CLAUDE.md; rerun -> "unchanged";
existing .mcp.json with another server keeps it; existing CLAUDE.md with an old station
block replaces only the block; invalid .mcp.json -> error entry and file untouched; git
init path creates repo with one commit.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Launch check with `FUEL_STATION_DB` on a temp file: Add Project on a temp folder copy,
screenshot the sheet and the result list.
Manual (user): Add Project on a real repo; Claude Code then asks to approve the station
server; Repair setup rewrites nothing on a second run; first-run import button on an empty
db.

## Out of scope
Agent file install (own card), drag-and-drop, editing projects outside the sheet.

Tier:  2 implementer-high. Why: async picker + multi-view UI state in iced, file merge logic, judgment in layout.
Gate:  cargo fmt/clippy/test + launch screenshot. Why: sheet is visual.
Scope: src/setup (2 new files), src/ui (sidebar, header, new sheet file). Asks: none
