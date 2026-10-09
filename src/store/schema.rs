use rusqlite::{Connection, TransactionBehavior};

use super::{Result, StoreError};

pub const MIGRATIONS: &[&str] = &["
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    path TEXT,
    ship_mode TEXT NOT NULL,
    default_branch TEXT NOT NULL,
    verify_cmd TEXT NOT NULL,
    worktree_dir TEXT NOT NULL,
    bootstrap_cmd TEXT NOT NULL,
    ship_cmd TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE cards (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    number INTEGER NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    plan TEXT NOT NULL DEFAULT '',
    col TEXT NOT NULL,
    position INTEGER NOT NULL,
    tier TEXT,
    human INTEGER NOT NULL DEFAULT 0,
    commit_base TEXT,
    commit_head TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(project_id, number)
);
CREATE INDEX cards_board ON cards(project_id, col, position);
CREATE TABLE comments (
    id INTEGER PRIMARY KEY,
    card_id INTEGER NOT NULL REFERENCES cards(id),
    actor_role TEXT NOT NULL,
    actor_name TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE events (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    card_id INTEGER REFERENCES cards(id),
    actor_role TEXT NOT NULL,
    actor_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX events_project ON events(project_id, created_at);
CREATE TABLE agents (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    role TEXT NOT NULL,
    name TEXT NOT NULL,
    card_id INTEGER REFERENCES cards(id),
    status TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    UNIQUE(project_id, role, name)
);
"];

pub fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let version = version as usize;
    if version > MIGRATIONS.len() {
        return Err(StoreError::Invalid(format!(
            "database schema v{version} is newer than this build (v{})",
            MIGRATIONS.len()
        )));
    }
    if version < MIGRATIONS.len() {
        for sql in &MIGRATIONS[version..] {
            tx.execute_batch(sql)?;
        }
        tx.pragma_update(None, "user_version", MIGRATIONS.len() as i64)?;
    }
    tx.commit()?;
    Ok(())
}
