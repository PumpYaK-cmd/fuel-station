# setup-async: setup sheet work off the UI thread, Repair detects (cards 689f2d3e + 837b769f)

base: fdde8f5

## Context
- `src/ui/setup_sheet.rs` `Sheet::update` `Submit` runs `write::apply` (git init/add/commit)
  synchronously; `Sheet::add` / `repair` / `repair_at` run `detect::detect` /
  `detect::facts` (git subprocesses) synchronously from `update` in `src/ui/mod.rs`. A big
  folder freezes the window.
- Imported projects (`src/import/mod.rs` `read_board`) have `ship_mode` git and empty
  `default_branch`, `verify_cmd`, `worktree_dir`. `Message::Repair` opens the sheet without
  detection; `repair_at` merges empty string fields but never `ship_mode`, so a Rojo folder
  stays git and Set up would `git init` it.

Ruling: Repair on a project with a path runs detection and fills empty fields. A project
whose `default_branch` is empty has never been through the sheet (the sheet always fills a
branch), so it also takes the detected `ship_mode`.

## Design
`src/ui/setup_sheet.rs`:
- `Sheet::add(form: NewProject, facts: Facts)` and
  `Sheet::repair(project: &Project, found: Option<(NewProject, Facts)>)` become pure
  constructors (no I/O). `repair` with `Some(found)` does what `repair_at` does now (fill
  path and empty string fields from `found`, take its `facts`) and also sets
  `form.ship_mode = found.ship_mode` when `project.default_branch.is_empty()`. With `None`,
  `facts` is `Facts::default()`. Delete `repair_at`.
- `pub fn detect(dir: PathBuf, taken: Vec<String>) -> Task<(NewProject, Facts)>` that runs
  `detect::detect` + `detect::facts` on a blocking thread
  (`tokio::task::spawn_blocking` inside `Task::perform`; iced runs on tokio here).
- `Sheet` gets `busy: bool`. `Submit`: save to the store synchronously as now (fast). On
  `Ok`, set `busy = true` and return the project key plus a task running
  `write::apply(&project, &installed_exe())` on a blocking thread, mapped to a new
  `Message::Applied(Vec<Written>)`. `Applied(result)` → `busy = false`,
  `self.result = Some(result)`.
  Signature: `fn update(&mut self, message, store) -> (Option<String>, Task<Message>)`.
- `view_form`: while `busy`, Set up has no `on_press` and the sheet shows
  `text("setting up…").color(theme::TEXT_MUTED)`.

`src/ui/mod.rs`:
- `Message::Picked(key, Some(dir))` → return `setup_sheet::detect(dir, taken)` mapped to a
  new `Message::Detected(Option<String>, NewProject, Facts)`. Keep however `taken` is built
  in that arm (another card in this batch changes that line to include archived keys; on
  rebase keep its version).
- `Message::Repair(key)`: project with a non-empty path → the same detect task on that path
  with `Some(key)`; no path → `Sheet::repair(p, None)` immediately.
- `Detected(None, form, facts)` → `app.sheet = Some(Sheet::add(form, facts))`.
  `Detected(Some(key), form, facts)` → find the project by key,
  `Sheet::repair(p, Some((form, facts)))`.
- `Message::Sheet(message)`: map the returned task with `Message::Sheet`; reload on
  `Some(key)` as now.
- If the user navigates away before `Applied` arrives (SelectProject clears `app.sheet`),
  `Applied` finds no sheet and is dropped.

Tests: `Sheet::repair` merge is now pure; add `#[cfg(test)] mod tests` in
`setup_sheet.rs`: an imported-like project (empty branch, Git) with a found Studio form
takes Studio and fills verify/branch; a configured project (branch "main", Git, verify set)
keeps Git and its non-empty fields. Build `Project` values directly.

## Verification
Gate. Manual for the user:
1. Add Project on a large folder: window keeps animating while detection runs.
2. Set up on a folder without git: "setting up…" shows, window stays responsive, the result
   list appears.
3. Repair setup on an imported project pointing at a Rojo folder: ship mode shows studio,
   gate/branch filled.

## Out of scope
Re-detecting ship mode for projects already configured through the sheet.

Tier:  2 implementer-high. Why: async Task plumbing across sheet and app, result ownership after navigation.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test, manual list above. Why: UI threading isn't unit testable.
Scope: 2 files (ui/setup_sheet.rs, ui/mod.rs sheet/repair arms). Asks: none
