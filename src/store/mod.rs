mod model;
mod schema;
#[cfg(test)]
mod tests;

pub use model::*;

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};

#[derive(Debug)]
pub enum StoreError {
    Sql(rusqlite::Error),
    NotFound(String),
    Invalid(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sql(e) => write!(f, "database error: {e}"),
            StoreError::NotFound(what) => write!(f, "not found: {what}"),
            StoreError::Invalid(why) => write!(f, "invalid: {why}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Sql(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sql(e)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

const LIVE_SECS: i64 = 120;

const PROJECT_SELECT: &str = "SELECT id, key, name, path, ship_mode, default_branch, verify_cmd, \
     worktree_dir, bootstrap_cmd, ship_cmd, created_at, archived FROM projects";

const CARD_SELECT: &str = "SELECT c.id, p.key, c.number, c.title, c.body, c.plan, c.col, \
     c.position, c.tier, c.human, c.commit_base, c.commit_head, c.created_at, c.updated_at, \
     c.project_id FROM cards c JOIN projects p ON p.id = c.project_id";

const EVENT_SELECT: &str = "SELECT e.id, p.key, c.number, e.actor_role, e.actor_name, e.kind, \
     e.message, e.created_at FROM events e JOIN projects p ON p.id = e.project_id \
     LEFT JOIN cards c ON c.id = e.card_id";

pub fn db_path() -> PathBuf {
    if let Some(path) = std::env::var_os("FUEL_STATION_DB") {
        return PathBuf::from(path);
    }
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    appdata.join("FuelStation").join("station.db")
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir).map_err(|e| {
                StoreError::Invalid(format!("cannot create {}: {e}", dir.display()))
            })?;
        }
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        let mode =
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StoreError::Invalid(format!(
                "journal_mode is {mode}, WAL required"
            )));
        }
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::migrate(&mut conn)?;
        Ok(Store { conn })
    }

    pub fn open_default() -> Result<Store> {
        Store::open(&db_path())
    }

    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA data_version", [], |r| r.get(0))?)
    }

    fn write<T>(&mut self, f: impl FnOnce(&Transaction) -> Result<T>) -> Result<T> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(&format!(
            "{PROJECT_SELECT} WHERE archived = 0 ORDER BY name"
        ))?;
        let projects = stmt
            .query_map([], project_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(projects)
    }

    pub fn project(&self, key: &str) -> Result<Project> {
        find_project(&self.conn, key)
    }

    pub fn project_add(&mut self, new: &NewProject) -> Result<Project> {
        let key = new.key.to_ascii_uppercase();
        if key.is_empty()
            || key.len() > 8
            || !key
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(StoreError::Invalid(format!(
                "project key {:?} must be 1-8 letters or digits",
                new.key
            )));
        }
        self.write(|tx| {
            let exists = tx
                .query_row("SELECT 1 FROM projects WHERE key = ?1", [&key], |_| Ok(()))
                .optional()?
                .is_some();
            if exists {
                return Err(StoreError::Invalid(format!("project {key} already exists")));
            }
            tx.execute(
                "INSERT INTO projects (key, name, path, ship_mode, default_branch, verify_cmd, \
                 worktree_dir, bootstrap_cmd, ship_cmd, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    key,
                    new.name,
                    new.path,
                    new.ship_mode,
                    new.default_branch,
                    new.verify_cmd,
                    new.worktree_dir,
                    new.bootstrap_cmd,
                    new.ship_cmd,
                    now()
                ],
            )?;
            let id = tx.last_insert_rowid();
            event(
                tx,
                id,
                None,
                &station(),
                "project_add",
                &format!("Added project {key}"),
            )?;
            find_project(tx, &key)
        })
    }

    pub fn project_update(&mut self, key: &str, new: &NewProject) -> Result<Project> {
        self.write(|tx| {
            let project = find_project(tx, key)?;
            if !new.key.is_empty() && !new.key.eq_ignore_ascii_case(&project.key) {
                return Err(StoreError::Invalid(format!(
                    "project key {} cannot change to {:?}",
                    project.key, new.key
                )));
            }
            tx.execute(
                "UPDATE projects SET name = ?2, path = ?3, ship_mode = ?4, default_branch = ?5, \
                 verify_cmd = ?6, worktree_dir = ?7, bootstrap_cmd = ?8, ship_cmd = ?9 \
                 WHERE id = ?1",
                params![
                    project.id,
                    new.name,
                    new.path,
                    new.ship_mode,
                    new.default_branch,
                    new.verify_cmd,
                    new.worktree_dir,
                    new.bootstrap_cmd,
                    new.ship_cmd
                ],
            )?;
            event(
                tx,
                project.id,
                None,
                &station(),
                "project_update",
                &format!("Updated project {}", project.key),
            )?;
            find_project(tx, &project.key)
        })
    }

    pub fn board(&self, key: &str, column: Option<Column>) -> Result<Vec<CardSummary>> {
        let project = find_project(&self.conn, key)?;
        let agents = self.agents(&project.key)?;
        let mut stmt = self.conn.prepare(
            "SELECT number, title, col, position, tier, human FROM cards \
             WHERE project_id = ?1 AND (?2 IS NULL OR col = ?2)",
        )?;
        let mut cards = stmt
            .query_map(params![project.id, column], |r| {
                let number: i64 = r.get(0)?;
                Ok(CardSummary {
                    key: format!("{}-{number}", project.key),
                    title: r.get(1)?,
                    column: r.get(2)?,
                    position: r.get(3)?,
                    tier: r.get(4)?,
                    human: r.get(5)?,
                    holder: None,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for card in &mut cards {
            card.holder = agents
                .iter()
                .find(|a| a.card_key.as_deref() == Some(card.key.as_str()))
                .map(|a| a.actor.clone());
        }
        cards.sort_by_key(|c| (c.column.index(), c.position));
        Ok(cards)
    }

    pub fn card(&self, card_key: &str) -> Result<Card> {
        Ok(find_card(&self.conn, card_key)?.1)
    }

    pub fn card_show(&self, card_key: &str) -> Result<CardDetail> {
        let (_, card) = find_card(&self.conn, card_key)?;
        let mut stmt = self.conn.prepare(
            "SELECT id, actor_role, actor_name, body, created_at FROM comments \
             WHERE card_id = ?1 ORDER BY created_at, id",
        )?;
        let comments = stmt
            .query_map([card.id], |r| {
                Ok(Comment {
                    id: r.get(0)?,
                    card_key: card.key.clone(),
                    actor: Actor {
                        role: r.get(1)?,
                        name: r.get(2)?,
                    },
                    body: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut stmt = self.conn.prepare(&format!(
            "{EVENT_SELECT} WHERE e.card_id = ?1 ORDER BY e.created_at DESC, e.id DESC LIMIT 20"
        ))?;
        let events = stmt
            .query_map([card.id], event_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(CardDetail {
            card,
            comments,
            events,
        })
    }

    pub fn card_add(
        &mut self,
        project_key: &str,
        actor: &Actor,
        column: Column,
        title: &str,
        body: &str,
    ) -> Result<Card> {
        self.write(|tx| {
            let project = find_project(tx, project_key)?;
            let number: i64 = tx.query_row(
                "SELECT COALESCE(MAX(number), 0) + 1 FROM cards WHERE project_id = ?1",
                [project.id],
                |r| r.get(0),
            )?;
            let position = end_of(tx, project.id, column)?;
            tx.execute(
                "INSERT INTO cards (project_id, number, title, body, col, position, \
                 created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![project.id, number, title, body, column, position, now()],
            )?;
            let id = tx.last_insert_rowid();
            event(
                tx,
                project.id,
                Some(id),
                actor,
                "card_add",
                &format!(
                    "Added {}-{number} to {}: {title}",
                    project.key,
                    column.as_str()
                ),
            )?;
            card_by_id(tx, id)
        })
    }

    pub fn card_update(
        &mut self,
        card_key: &str,
        actor: &Actor,
        patch: &CardPatch,
    ) -> Result<Card> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            tx.execute(
                "UPDATE cards SET title = COALESCE(?2, title), body = COALESCE(?3, body), \
                 plan = COALESCE(?4, plan), tier = COALESCE(?5, tier), \
                 human = COALESCE(?6, human), updated_at = ?7 WHERE id = ?1",
                params![
                    card.id,
                    patch.title,
                    patch.body,
                    patch.plan,
                    patch.tier,
                    patch.human,
                    now()
                ],
            )?;
            let fields: Vec<&str> = [
                ("title", patch.title.is_some()),
                ("body", patch.body.is_some()),
                ("plan", patch.plan.is_some()),
                ("tier", patch.tier.is_some()),
                ("human", patch.human.is_some()),
            ]
            .into_iter()
            .filter_map(|(name, set)| set.then_some(name))
            .collect();
            event(
                tx,
                project_id,
                Some(card.id),
                actor,
                "card_update",
                &format!("Updated {} ({})", card.key, fields.join(", ")),
            )?;
            card_by_id(tx, card.id)
        })
    }

    pub fn card_move(&mut self, card_key: &str, actor: &Actor, column: Column) -> Result<Card> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            move_to(tx, project_id, card.id, column)?;
            event(
                tx,
                project_id,
                Some(card.id),
                actor,
                "card_move",
                &format!(
                    "Moved {} {} -> {}",
                    card.key,
                    card.column.as_str(),
                    column.as_str()
                ),
            )?;
            card_by_id(tx, card.id)
        })
    }

    pub fn card_reorder(&mut self, card_key: &str, actor: &Actor, position: i64) -> Result<Card> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            let mut ids = tx
                .prepare(
                    "SELECT id FROM cards WHERE project_id = ?1 AND col = ?2 AND id != ?3 \
                     ORDER BY position, id",
                )?
                .query_map(params![project_id, card.column, card.id], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<i64>>>()?;
            let at = position.clamp(0, ids.len() as i64) as usize;
            ids.insert(at, card.id);
            for (i, id) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE cards SET position = ?2 WHERE id = ?1",
                    params![id, i as i64],
                )?;
            }
            tx.execute(
                "UPDATE cards SET updated_at = ?2 WHERE id = ?1",
                params![card.id, now()],
            )?;
            event(
                tx,
                project_id,
                Some(card.id),
                actor,
                "card_reorder",
                &format!(
                    "Moved {} to position {at} in {}",
                    card.key,
                    card.column.as_str()
                ),
            )?;
            card_by_id(tx, card.id)
        })
    }

    pub fn grab(&mut self, project_key: &str, actor: &Actor, from: Column) -> Result<Option<Card>> {
        self.write(|tx| {
            let project = find_project(tx, project_key)?;
            let id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM cards WHERE project_id = ?1 AND col = ?2 AND human = 0 \
                     ORDER BY position, id LIMIT 1",
                    params![project.id, from],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(id) = id else {
                return Ok(None);
            };
            move_to(tx, project.id, id, Column::Doing)?;
            let card = card_by_id(tx, id)?;
            event(
                tx,
                project.id,
                Some(id),
                actor,
                "grab",
                &format!("Grabbed {} from {}", card.key, from.as_str()),
            )?;
            Ok(Some(card))
        })
    }

    pub fn comment(&mut self, card_key: &str, actor: &Actor, body: &str) -> Result<Comment> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            let comment = insert_comment(tx, &card, actor, body)?;
            event(
                tx,
                project_id,
                Some(card.id),
                actor,
                "comment",
                &format!("Commented on {}", card.key),
            )?;
            Ok(comment)
        })
    }

    pub fn report(&mut self, card_key: &str, actor: &Actor, message: &str) -> Result<()> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            event(tx, project_id, Some(card.id), actor, "report", message)
        })
    }

    pub fn submit_review(
        &mut self,
        card_key: &str,
        actor: &Actor,
        base: &str,
        head: &str,
    ) -> Result<Card> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            if card.column != Column::Doing {
                return Err(StoreError::Invalid(format!(
                    "{} is in {}, not Doing",
                    card.key,
                    card.column.as_str()
                )));
            }
            move_to(tx, project_id, card.id, Column::Review)?;
            tx.execute(
                "UPDATE cards SET commit_base = ?2, commit_head = ?3 WHERE id = ?1",
                params![card.id, base, head],
            )?;
            event(
                tx,
                project_id,
                Some(card.id),
                actor,
                "submit_review",
                &format!("Submitted {} for review ({base}..{head})", card.key),
            )?;
            card_by_id(tx, card.id)
        })
    }

    pub fn review_result(
        &mut self,
        card_key: &str,
        actor: &Actor,
        verdict: Verdict,
        findings: &[String],
    ) -> Result<Card> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            if card.column != Column::Review {
                return Err(StoreError::Invalid(format!(
                    "{} is in {}, not Review",
                    card.key,
                    card.column.as_str()
                )));
            }
            let summary = match verdict {
                Verdict::Clean => "Review: clean".to_string(),
                Verdict::Findings => format!("Review: {} findings", findings.len()),
            };
            let mut body = summary.clone();
            if verdict == Verdict::Findings {
                for finding in findings {
                    body.push_str("\n- ");
                    body.push_str(finding);
                }
                move_to(tx, project_id, card.id, Column::Doing)?;
            }
            insert_comment(tx, &card, actor, &body)?;
            event(
                tx,
                project_id,
                Some(card.id),
                actor,
                "review_result",
                &format!("{}: {summary}", card.key),
            )?;
            card_by_id(tx, card.id)
        })
    }

    pub fn ship(
        &mut self,
        card_key: &str,
        actor: &Actor,
        summary: &str,
        commit: &str,
    ) -> Result<Card> {
        self.write(|tx| {
            let (project_id, card) = find_card(tx, card_key)?;
            if !matches!(card.column, Column::Review | Column::Doing) {
                return Err(StoreError::Invalid(format!(
                    "{} is in {}, not Review or Doing",
                    card.key,
                    card.column.as_str()
                )));
            }
            move_to(tx, project_id, card.id, Column::Done)?;
            tx.execute(
                "UPDATE cards SET commit_head = ?2 WHERE id = ?1",
                params![card.id, commit],
            )?;
            insert_comment(tx, &card, actor, &format!("{summary}\n\nCommit: {commit}"))?;
            event(
                tx,
                project_id,
                Some(card.id),
                actor,
                "ship",
                &format!("Shipped {} at {commit}", card.key),
            )?;
            card_by_id(tx, card.id)
        })
    }

    pub fn checkin(&mut self, project_key: &str, actor: &Actor, card: Option<&str>) -> Result<()> {
        self.write(|tx| {
            let project = find_project(tx, project_key)?;
            let card = match card {
                Some(card_key) => {
                    let (card_project, card) = find_card(tx, card_key)?;
                    if card_project != project.id {
                        return Err(StoreError::Invalid(format!(
                            "{} is not in project {}",
                            card.key, project.key
                        )));
                    }
                    Some(card)
                }
                None => None,
            };
            let card_id = card.as_ref().map(|c| c.id);
            tx.execute(
                "INSERT INTO agents (project_id, role, name, card_id, status, started_at, last_seen) \
                 VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?5) \
                 ON CONFLICT(project_id, role, name) DO UPDATE SET \
                 card_id = excluded.card_id, status = 'active', last_seen = excluded.last_seen",
                params![project.id, actor.role, actor.name, card_id, now()],
            )?;
            let message = match &card {
                Some(card) => format!("{} checked in on {}", actor.name, card.key),
                None => format!("{} checked in", actor.name),
            };
            event(tx, project.id, card_id, actor, "checkin", &message)
        })
    }

    pub fn checkout(&mut self, project_key: &str, actor: &Actor) -> Result<()> {
        self.write(|tx| {
            let project = find_project(tx, project_key)?;
            tx.execute(
                "UPDATE agents SET status = 'gone', last_seen = ?4 \
                 WHERE project_id = ?1 AND role = ?2 AND name = ?3",
                params![project.id, actor.role, actor.name, now()],
            )?;
            event(
                tx,
                project.id,
                None,
                actor,
                "checkout",
                &format!("{} checked out", actor.name),
            )
        })
    }

    pub fn agents(&self, project_key: &str) -> Result<Vec<Agent>> {
        let project = find_project(&self.conn, project_key)?;
        let mut stmt = self.conn.prepare(
            "SELECT a.role, a.name, c.number, a.status, a.started_at, a.last_seen \
             FROM agents a LEFT JOIN cards c ON c.id = a.card_id \
             WHERE a.project_id = ?1 AND a.status = 'active' AND a.last_seen >= ?2 \
             ORDER BY a.started_at, a.id",
        )?;
        let agents = stmt
            .query_map(params![project.id, now() - LIVE_SECS], |r| {
                let number: Option<i64> = r.get(2)?;
                Ok(Agent {
                    actor: Actor {
                        role: r.get(0)?,
                        name: r.get(1)?,
                    },
                    card_key: number.map(|n| format!("{}-{n}", project.key)),
                    status: r.get(3)?,
                    started_at: r.get(4)?,
                    last_seen: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(agents)
    }

    pub fn events(&self, project_key: &str, limit: usize) -> Result<Vec<Event>> {
        let project = find_project(&self.conn, project_key)?;
        let mut stmt = self.conn.prepare(&format!(
            "{EVENT_SELECT} WHERE e.project_id = ?1 ORDER BY e.created_at DESC, e.id DESC LIMIT ?2"
        ))?;
        let events = stmt
            .query_map(params![project.id, limit as i64], event_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(events)
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn station() -> Actor {
    Actor {
        role: Role::Manager,
        name: "station".to_string(),
    }
}

fn parse_card_key(card_key: &str) -> Result<(String, i64)> {
    let invalid = || StoreError::Invalid(format!("card id {card_key:?} is not KEY-N"));
    let (key, number) = card_key.rsplit_once('-').ok_or_else(invalid)?;
    let number: i64 = number.parse().map_err(|_| invalid())?;
    if key.is_empty() || number < 1 {
        return Err(invalid());
    }
    Ok((key.to_ascii_uppercase(), number))
}

fn project_row(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        key: r.get(1)?,
        name: r.get(2)?,
        path: r.get(3)?,
        ship_mode: r.get(4)?,
        default_branch: r.get(5)?,
        verify_cmd: r.get(6)?,
        worktree_dir: r.get(7)?,
        bootstrap_cmd: r.get(8)?,
        ship_cmd: r.get(9)?,
        created_at: r.get(10)?,
        archived: r.get(11)?,
    })
}

fn card_row(r: &Row) -> rusqlite::Result<Card> {
    let project_key: String = r.get(1)?;
    let number: i64 = r.get(2)?;
    Ok(Card {
        id: r.get(0)?,
        key: format!("{project_key}-{number}"),
        project_key,
        number,
        title: r.get(3)?,
        body: r.get(4)?,
        plan: r.get(5)?,
        column: r.get(6)?,
        position: r.get(7)?,
        tier: r.get(8)?,
        human: r.get(9)?,
        commit_base: r.get(10)?,
        commit_head: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

fn event_row(r: &Row) -> rusqlite::Result<Event> {
    let project_key: String = r.get(1)?;
    let number: Option<i64> = r.get(2)?;
    Ok(Event {
        id: r.get(0)?,
        card_key: number.map(|n| format!("{project_key}-{n}")),
        project_key,
        actor: Actor {
            role: r.get(3)?,
            name: r.get(4)?,
        },
        kind: r.get(5)?,
        message: r.get(6)?,
        created_at: r.get(7)?,
    })
}

fn find_project(conn: &Connection, key: &str) -> Result<Project> {
    let key = key.to_ascii_uppercase();
    conn.query_row(
        &format!("{PROJECT_SELECT} WHERE key = ?1"),
        [&key],
        project_row,
    )
    .optional()?
    .ok_or(StoreError::NotFound(key))
}

fn find_card(conn: &Connection, card_key: &str) -> Result<(i64, Card)> {
    let (key, number) = parse_card_key(card_key)?;
    conn.query_row(
        &format!("{CARD_SELECT} WHERE p.key = ?1 AND c.number = ?2"),
        params![key, number],
        |r| Ok((r.get(14)?, card_row(r)?)),
    )
    .optional()?
    .ok_or_else(|| StoreError::NotFound(format!("{key}-{number}")))
}

fn card_by_id(conn: &Connection, id: i64) -> Result<Card> {
    Ok(conn.query_row(&format!("{CARD_SELECT} WHERE c.id = ?1"), [id], card_row)?)
}

fn end_of(conn: &Connection, project_id: i64, column: Column) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM cards WHERE project_id = ?1 AND col = ?2",
        params![project_id, column],
        |r| r.get(0),
    )?)
}

fn move_to(conn: &Connection, project_id: i64, card_id: i64, column: Column) -> Result<()> {
    let position = end_of(conn, project_id, column)?;
    conn.execute(
        "UPDATE cards SET col = ?2, position = ?3, updated_at = ?4 WHERE id = ?1",
        params![card_id, column, position, now()],
    )?;
    Ok(())
}

fn insert_comment(conn: &Connection, card: &Card, actor: &Actor, body: &str) -> Result<Comment> {
    let created_at = now();
    conn.execute(
        "INSERT INTO comments (card_id, actor_role, actor_name, body, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![card.id, actor.role, actor.name, body, created_at],
    )?;
    Ok(Comment {
        id: conn.last_insert_rowid(),
        card_key: card.key.clone(),
        actor: actor.clone(),
        body: body.to_string(),
        created_at,
    })
}

fn event(
    conn: &Connection,
    project_id: i64,
    card_id: Option<i64>,
    actor: &Actor,
    kind: &str,
    message: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO events (project_id, card_id, actor_role, actor_name, kind, message, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![project_id, card_id, actor.role, actor.name, kind, message, now()],
    )?;
    Ok(())
}
