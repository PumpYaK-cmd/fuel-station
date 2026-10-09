# Import trello boards (card be62122e)

base: <filled by agent>

## Context
design.md > Import. Source: `~/.trello/<boardId>/` with `board.json` (`name`, `desc`,
`closed`), `lists.json` (array of `{id, name, pos, closed}`), `cards/*.json` (`idList`,
`name`, `desc`, `pos`, `closed`, `dateCreated` ISO-8601, `comments`: array of `{date,
memberCreator: {fullName}, data: {text}}`). `attachments/` and `activity.log` are ignored.
Read-only on `~/.trello`. Store landed `src/store/`; read its API first.
Other cards running in parallel: MCP tools (src/mcp), GUI shell (src/ui), Install
(src/setup, src/main.rs). You own src/import, the one new store function below, and the
`import-trello` lines of src/main.rs (expect a trivial rebase conflict with Install's
subcommand; keep both). The GUI first-run button is NOT this card; the Add Project card
adds it and calls your `import::import_all`.

## Design
- `src/import/mod.rs` (serde structs for the JSON, only the fields above):
  - `pub struct ImportReport { pub imported: Vec<(String /*key*/, String /*name*/, usize
    /*cards*/)>, pub skipped: Vec<(String /*board name*/, String /*reason*/)> }`
  - `pub fn import_all(store: &mut Store, trello_dir: &Path) -> Result<ImportReport,
    String>`: every board dir, sorted by board name. Skip closed boards and boards whose
    name equals an existing project name ("already imported").
  - `fn read_board(dir) -> Result<store::ImportBoard, String>`.
  - `pub fn trello_dir() -> PathBuf`: `%USERPROFILE%\.trello`.
  - Key: `store::derive_key(name: &str, taken: &[String]) -> String` in
    `src/store/model.rs` (Add Project reuses it for folder names): split on
    non-alphanumerics; 2+ words -> initials, 1 word -> first 4 chars; uppercase, max 8; on
    collision with a taken key append 2, 3, ... (fuel-station -> FS, shototsu -> SHOT,
    yak-aviation -> YA, then yak-automotive -> YA2, "Fuel Station" -> FS). Unit-tested.
  - Path: first `[A-Za-z]:[\\/]` in `desc`; take to end of that line, cut at the first of
    ` (`, `. `, `; `, ` - `, then trim whitespace and trailing `.`. None if no match.
  - Lists: Backlog/Ready/Doing/Done by name (case-insensitive); any other list ->
    Backlog. Closed lists still map by name. Closed cards are skipped.
  - Order: within each target column, by (list pos, card pos).
  - `human` = title starts with `[HUMAN]` (title kept as is).
  - Comments oldest first; actor `{role: manager, name: fullName}`; created_at from `date`.
    Card created_at from `dateCreated` (fallback: now).
  - ISO-8601 parsing with `chrono` (in Cargo.toml).
  - Project: name = board name, ship_mode Git, everything else empty; the user fixes it in
    Repair setup later.
- `src/store/`: add `ImportBoard { project: NewProject, cards: Vec<ImportCard> }`,
  `ImportCard { column, title, body, human, created_at, comments: Vec<(Actor, String,
  i64)> }` to model.rs, and `Store::import_board(&mut self, &ImportBoard) -> Result<Project>`:
  one transaction, numbers 1..n in import order, positions per column, one `import` event
  for the project (`{manager, "station"}`, "imported N cards from trello"), no per-card
  events. Store test: import_board round-trip (numbers, positions, comments).
- `src/main.rs`: subcommand `import-trello [--dir <path>]` (default `trello_dir()`); prints
  one line per imported / skipped board.

## Tests
- `import` tests build a fake trello dir in a tempdir (two boards incl. a closed one, an
  extra list, a closed card, a `[HUMAN]` card, comments) and check the report and the store.
- Path extraction table test with these real descriptions:
  - `... Repo: C:\Users\gabri\Documents\Development\Shototsu` -> that path
  - `... Repo: C:\Users\gabri\Documents\Development\Forge\LOAD (source mirror ...` -> `...\Forge\LOAD`
  - `MSFS-based ... F35B. C:\Users\gabri\Documents\Development\YaK Aviation 2` -> `...\YaK Aviation 2`
  - `... Repo: C:\Users\gabri\Documents\Development\YaK Automotive. Spec in docs/, ...` -> `...\YaK Automotive`
  - `Kitsu Pen ... C:/Users/gabri/Documents/Development2/Kitsu Pen` -> that path
  - `... Repo: C:\Users\gabri\Documents\Development2\Fuel Station` -> that path
  - `no path here` -> None
- Then one ignored-by-default test is NOT wanted; instead, in your report, paste the output
  of `fuel-station import-trello` run with `FUEL_STATION_DB` set to a temp file against a
  temp copy of `~/.trello` (copy it first; never write into `~/.trello`).

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
Manual (user): `fuel-station import-trello` on the real db once the GUI is in; boards show.

## Out of scope
GUI button and folder prompt for path-less projects (Add Project card), cutover.

Tier:  1 implementer. Why: plan names every function, rule and test case; serde + one store fn.
Gate:  cargo fmt/clippy/test + temp-copy run. Why: covered by tests.
Scope: src/import, src/store (2 fns + 2 types), main.rs. Asks: none
