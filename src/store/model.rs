use std::str::FromStr;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::StoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Manager,
    Truck,
    Fuel,
    Mechanic,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Manager => "manager",
            Role::Truck => "truck",
            Role::Fuel => "fuel",
            Role::Mechanic => "mechanic",
        }
    }
}

impl FromStr for Role {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self, StoreError> {
        match s.to_ascii_lowercase().as_str() {
            "manager" => Ok(Role::Manager),
            "truck" => Ok(Role::Truck),
            "fuel" => Ok(Role::Fuel),
            "mechanic" => Ok(Role::Mechanic),
            _ => Err(StoreError::Invalid(format!("unknown role {s:?}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Actor {
    pub role: Role,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Column {
    Backlog,
    Ready,
    Doing,
    Review,
    Done,
}

impl Column {
    pub const ALL: [Column; 5] = [
        Column::Backlog,
        Column::Ready,
        Column::Doing,
        Column::Review,
        Column::Done,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Column::Backlog => "Backlog",
            Column::Ready => "Ready",
            Column::Doing => "Doing",
            Column::Review => "Review",
            Column::Done => "Done",
        }
    }

    pub fn next(self) -> Option<Column> {
        Column::ALL.get(self.index() + 1).copied()
    }

    pub fn prev(self) -> Option<Column> {
        self.index().checked_sub(1).map(|i| Column::ALL[i])
    }

    pub(super) fn index(self) -> usize {
        self as usize
    }
}

impl FromStr for Column {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self, StoreError> {
        Column::ALL
            .into_iter()
            .find(|c| c.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| StoreError::Invalid(format!("unknown column {s:?}")))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ShipMode {
    #[default]
    Git,
    Studio,
}

impl ShipMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ShipMode::Git => "git",
            ShipMode::Studio => "studio",
        }
    }
}

impl FromStr for ShipMode {
    type Err = StoreError;

    fn from_str(s: &str) -> Result<Self, StoreError> {
        match s.to_ascii_lowercase().as_str() {
            "git" => Ok(ShipMode::Git),
            "studio" => Ok(ShipMode::Studio),
            _ => Err(StoreError::Invalid(format!("unknown ship mode {s:?}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Clean,
    Findings,
}

macro_rules! sql_text {
    ($($t:ty),*) => {$(
        impl ToSql for $t {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.as_str()))
            }
        }

        impl FromSql for $t {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                value.as_str()?.parse().map_err(FromSqlError::other)
            }
        }
    )*};
}

sql_text!(Role, Column, ShipMode);

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub path: Option<String>,
    pub ship_mode: ShipMode,
    pub default_branch: String,
    pub verify_cmd: String,
    pub worktree_dir: String,
    pub bootstrap_cmd: String,
    pub ship_cmd: String,
    pub created_at: i64,
    pub archived: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewProject {
    pub key: String,
    pub name: String,
    pub path: Option<String>,
    pub ship_mode: ShipMode,
    pub default_branch: String,
    pub verify_cmd: String,
    pub worktree_dir: String,
    pub bootstrap_cmd: String,
    pub ship_cmd: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Card {
    pub id: i64,
    pub key: String,
    pub project_key: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub plan: String,
    pub column: Column,
    pub position: i64,
    pub tier: Option<String>,
    pub human: bool,
    pub commit_base: Option<String>,
    pub commit_head: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CardSummary {
    pub key: String,
    pub title: String,
    pub column: Column,
    pub position: i64,
    pub tier: Option<String>,
    pub human: bool,
    pub holder: Option<Actor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comment {
    pub id: i64,
    pub card_key: String,
    pub actor: Actor,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: i64,
    pub project_key: String,
    pub card_key: Option<String>,
    pub actor: Actor,
    pub kind: String,
    pub message: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Agent {
    pub actor: Actor,
    pub card_key: Option<String>,
    pub status: String,
    pub started_at: i64,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CardDetail {
    pub card: Card,
    pub comments: Vec<Comment>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CardPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub plan: Option<String>,
    pub tier: Option<String>,
    pub human: Option<bool>,
}
