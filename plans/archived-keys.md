# archived-keys: key/name checks see archived projects (cards 139090c6 + 9c774718)

base: fdde8f5

## Context
`Store::projects()` filters `archived = 0`. Two callers use it to build "taken" lists:
- `src/ui/mod.rs` `Message::Picked` builds `taken` keys for `Sheet::add` / `repair_at`.
- `src/import/mod.rs` `import_all` builds `taken` keys and `names`.
`insert_project` checks all rows, so a derived key can collide with an archived project, and
an archived project's board can be re-imported.

## Design
- `src/store/mod.rs`: add
  `pub fn project_names(&self) -> Result<Vec<(String, String)>>` returning `(key, name)` for
  every project row, archived or not, `ORDER BY key`. Plain `SELECT key, name FROM projects`.
- `src/ui/mod.rs` `Message::Picked`: `taken` = keys from `app.store.project_names()`; on
  error set `app.error` and return `Task::none()`. Touch only that arm (another card is
  reworking the sheet arms in parallel).
- `src/import/mod.rs` `import_all`: build `taken` and `names` from `store.project_names()`
  instead of `store.projects()`.
- `src/store/tests.rs`: one test: add two projects, mark one archived with a raw
  `UPDATE projects SET archived = 1` through the store's connection (tests live inside the
  store module), assert `projects()` omits it and `project_names()` includes it.
- `src/import/tests.rs`: one test: archived project named like a board → board skipped
  "already imported"; archived project holding the derived key → import picks the next key.
  Reuse the helpers already in that file.

## Verification
Gate.

## Out of scope
Any UI to archive projects (nothing sets `archived` yet).

Tier:  1 implementer. Why: one query, two call sites, tests next to existing ones.
Gate:  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test. Why: store and import are unit tested.
Scope: 5 files (store/mod.rs, store/tests.rs, import/mod.rs, import/tests.rs, ui/mod.rs one arm). Asks: none
