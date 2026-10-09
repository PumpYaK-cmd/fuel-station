# Store: SQLite schema, migrations, card operations (card eb462b24)

base: befcc012c8612c1f3b38dd3744ea912e997414a5

## Context
design.md > Data model, Architecture, MCP server. Crate skeleton is committed; `src/store/`
is empty. The Spike card runs in parallel and owns `src/ui`, `src/mcp`; touch only
`src/store/`, plus `#[allow(dead_code)]` on `mod store;` in `src/main.rs` if clippy needs
it because nothing calls store yet (one attribute there, not per-item allows).
`store` is the only module that writes SQL. Every later card (MCP tools, GUI, import,
setup) calls the API below, so keep these names and shapes exactly.

## Files
- `src/store/mod.rs`: `Store`, errors, public API.
- `src/store/schema.rs`: migrations.
- `src/store/model.rs`: types, re-exported from `store`.
- `src/store/tests.rs` (`#[cfg(test)] mod tests;`).

## Types (model.rs)
All structs `Debug, Clone, Serialize`. Enums `Debug, Clone, Copy, PartialEq, Eq, Serialize,
Deserialize, JsonSchema`, serde lowercase.
- `Role { Manager, Truck, Fuel, Mechanic }`
- `Actor { role: Role, name: String }` (also Deserialize, JsonSchema, PartialEq)
- `Column { Backlog, Ready, Doing, Review, Done }`, `Column::ALL: [Column; 5]`,
  `as_str()` ("Backlog".."Done"), `FromStr` case-insensitive, `next()` / `prev()` -> Option.
- `ShipMode { Git, Studio }`, `Verdict { Clean, Findings }`
- `Project { id: i64, key, name, path: Option<String>, ship_mode, default_branch,
  verify_cmd, worktree_dir, bootstrap_cmd, ship_cmd, created_at, archived: bool }` (string
  fields `String`, empty = unset, except path).
- `NewProject` = Project minus id / created_at / archived (also Deserialize, Default with
  ship_mode Git).
- `Card { id: i64, key: String /* "FS-12" */, project_key, number: i64, title, body, plan,
  column: Column, position: i64, tier: Option<String>, human: bool, commit_base:
  Option<String>, commit_head: Option<String>, created_at, updated_at }`
- `CardSummary { key, title, column, position, tier, human, holder: Option<Actor> }`; holder
  = a live agent (see `agents`) whose card is this card.
- `Comment { id, card_key, actor: Actor, body, created_at }`
- `Event { id, project_key, card_key: Option<String>, actor: Actor, kind: String, message,
  created_at }`
- `Agent { actor: Actor, card_key: Option<String>, status: String, started_at, last_seen }`
- `CardDetail { card: Card, comments: Vec<Comment> /* oldest first */, events: Vec<Event>
  /* last 20, newest first */ }`
- `CardPatch { title, body, plan, tier: Option<String>, human: Option<bool> }`, all
  `Option`, Default + Deserialize.
- Timestamps: unix seconds `i64`.

## Errors
`pub enum StoreError { Sql(rusqlite::Error), NotFound(String), Invalid(String) }`, one-line
`Display`, `std::error::Error`, `From<rusqlite::Error>`. `pub type Result<T>`.

## Connection
- `pub fn db_path() -> PathBuf`: env `FUEL_STATION_DB` if set, else
  `%APPDATA%\FuelStation\station.db`.
- `Store::open(path: &Path) -> Result<Store>`: create parent dir, `journal_mode=WAL`,
  `busy_timeout=5000`, `foreign_keys=ON`, run migrations. `Store::open_default()`.
- `data_version(&self) -> Result<i64>` (`PRAGMA data_version`).
- Migrations: `PRAGMA user_version`; `MIGRATIONS: &[&str]`, apply each above the current
  one in one transaction and bump. v1 = the five tables of design.md > Data model, with
  `UNIQUE(key)` on projects, `UNIQUE(project_id, number)` on cards,
  `UNIQUE(project_id, role, name)` on agents, indexes on cards(project_id, column,
  position) and events(project_id, created_at). `column` is a reserved word: quote it or
  name the SQL column `col`.

## API
`&self` for reads, `&mut self` for writes. Every write is one transaction and inserts one
`events` row with the actor, kind = the function name, message = short human text.
- `projects(&self) -> Vec<Project>` (non-archived, by name); `project(&self, key)`;
  `project_add(&mut self, &NewProject) -> Project` (key uppercased, must match
  `[A-Z0-9]{1,8}`, else Invalid); `project_update(&mut self, key, &NewProject) -> Project`.
  Project events use actor `{manager, "station"}` and card_key None.
- `board(&self, key, column: Option<Column>) -> Vec<CardSummary>` ordered by column order
  then position.
- `card(&self, card_key) -> Card`; `card_show(&self, card_key) -> CardDetail`. Card keys
  parse as `KEY-N`; bad form -> Invalid; missing -> NotFound("FS-9").
- `card_add(&mut self, project_key, &Actor, Column, title, body) -> Card`: number = max+1
  per project, position = end of column.
- `card_update(&mut self, card_key, &Actor, &CardPatch) -> Card`, bumps updated_at.
- `card_move(&mut self, card_key, &Actor, Column) -> Card`: to the end of the target column.
- `card_reorder(&mut self, card_key, &Actor, position: i64) -> Card`: within its column,
  then renumber positions 0..n.
- `grab(&mut self, project_key, &Actor, from: Column) -> Option<Card>`: `BEGIN IMMEDIATE`
  (`transaction_with_behavior(TransactionBehavior::Immediate)`), top (lowest position)
  non-human card in `from` -> end of Doing; None if there is none.
- `comment(&mut self, card_key, &Actor, body) -> Comment`.
- `report(&mut self, card_key, &Actor, message)`: event only (kind "report").
- `submit_review(&mut self, card_key, &Actor, base, head) -> Card`: must be in Doing else
  Invalid; -> Review; sets commit_base / commit_head.
- `review_result(&mut self, card_key, &Actor, Verdict, findings: &[String]) -> Card`: must be
  in Review. Adds a comment ("Review: clean", or "Review: N findings" plus one "- " line
  each). Clean stays in Review; Findings -> Doing.
- `ship(&mut self, card_key, &Actor, summary, commit) -> Card`: from Review or Doing ->
  Done, commit_head = commit, comment with summary and commit.
- `checkin(&mut self, project_key, &Actor, card: Option<&str>)`: upsert agents row, status
  "active", last_seen now (started_at set on insert only). `checkout(&mut self, project_key,
  &Actor)`: status "gone". Both write events.
- `agents(&self, project_key) -> Vec<Agent>`: status active and last_seen within 120 s.
- `events(&self, project_key, limit) -> Vec<Event>` newest first.

## Tests (temp dirs via `tempfile`)
Migrations idempotent on reopen; project_add + card_add numbering (FS-1, FS-2) and
positions; move appends to end; reorder; grab takes top non-human, skips human, None on
empty; **two `Store`s on the same file in two threads grab from a Ready column of 20 cards,
every card grabbed exactly once**; submit_review / review_result / ship transitions and
invalid-state errors; every mutation writes an event; agents liveness window (age
last_seen with raw SQL in the test); data_version changes after another connection writes.

## Verification
Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## Out of scope
MCP wiring, UI, import (that card adds its own store fn).

Tier:  2 implementer-high. Why: foundation API every card builds on; transaction and concurrency semantics.
Gate:  cargo fmt/clippy/test with unit tests. Why: fully covered by tests.
Scope: 4 new files in src/store. Asks: none
