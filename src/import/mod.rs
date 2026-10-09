use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::store::{
    Actor, Column, ImportBoard, ImportCard, NewProject, Role, ShipMode, Store, derive_key,
};

#[derive(Debug, Default)]
pub struct ImportReport {
    pub imported: Vec<(String, String, usize)>,
    pub skipped: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct BoardFile {
    name: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    closed: bool,
}

#[derive(Deserialize)]
struct ListFile {
    id: String,
    name: String,
    #[serde(default)]
    pos: f64,
}

#[derive(Deserialize)]
struct CardFile {
    #[serde(rename = "idList")]
    id_list: String,
    name: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    pos: f64,
    #[serde(default)]
    closed: bool,
    #[serde(rename = "dateCreated")]
    date_created: Option<String>,
    #[serde(default)]
    comments: Vec<CommentFile>,
}

#[derive(Deserialize)]
struct CommentFile {
    date: Option<String>,
    #[serde(rename = "memberCreator")]
    member: MemberFile,
    data: CommentData,
}

#[derive(Deserialize)]
struct MemberFile {
    #[serde(rename = "fullName", default)]
    full_name: String,
}

#[derive(Deserialize)]
struct CommentData {
    #[serde(default)]
    text: String,
}

pub fn trello_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".trello")
}

pub fn import_all(store: &mut Store, trello_dir: &Path) -> Result<ImportReport, String> {
    let entries =
        std::fs::read_dir(trello_dir).map_err(|e| format!("{}: {e}", trello_dir.display()))?;
    let mut report = ImportReport::default();
    let mut boards = Vec::new();
    for entry in entries {
        let dir = entry.map_err(|e| e.to_string())?.path();
        if !dir.is_dir() {
            continue;
        }
        match read_json::<BoardFile>(&dir.join("board.json")) {
            Ok(board) => boards.push((dir, board)),
            Err(e) => {
                let label = dir.file_name().unwrap_or_default().to_string_lossy();
                report.skipped.push((label.into_owned(), e));
            }
        }
    }
    boards.sort_by(|a, b| a.1.name.cmp(&b.1.name));

    let projects = store.projects().map_err(|e| e.to_string())?;
    let mut taken: Vec<String> = projects.iter().map(|p| p.key.clone()).collect();
    for (dir, file) in boards {
        if file.closed {
            report.skipped.push((file.name, "closed".to_string()));
            continue;
        }
        if projects.iter().any(|p| p.name == file.name) {
            report
                .skipped
                .push((file.name, "already imported".to_string()));
            continue;
        }
        let name = file.name.clone();
        let mut board = read_board(&dir, file)?;
        board.project.key = derive_key(&name, &taken);
        let cards = board.cards.len();
        let project = store.import_board(&board).map_err(|e| e.to_string())?;
        taken.push(project.key.clone());
        report.imported.push((project.key, name, cards));
    }
    Ok(report)
}

fn read_board(dir: &Path, board: BoardFile) -> Result<ImportBoard, String> {
    let lists: Vec<ListFile> = read_json(&dir.join("lists.json"))?;
    let cards_dir = dir.join("cards");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&cards_dir)
        .map_err(|e| format!("{}: {e}", cards_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    let mut sorted: Vec<(f64, f64, ImportCard)> = Vec::new();
    for path in paths {
        let card: CardFile = read_json(&path)?;
        if card.closed {
            continue;
        }
        let list = lists.iter().find(|l| l.id == card.id_list);
        let column = list.map_or(Column::Backlog, |l| column_for(&l.name));
        let list_pos = list.map_or(f64::MAX, |l| l.pos);
        let created_at = card
            .date_created
            .as_deref()
            .and_then(parse_time)
            .unwrap_or_else(now);
        let mut comments: Vec<_> = card.comments.iter().map(comment).collect();
        comments.sort_by_key(|c| c.2);
        sorted.push((
            list_pos,
            card.pos,
            ImportCard {
                column,
                human: card.name.starts_with("[HUMAN]"),
                title: card.name,
                body: card.desc,
                created_at,
                comments,
            },
        ));
    }
    sorted.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(Ordering::Equal)
            .then(a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
    });

    Ok(ImportBoard {
        project: NewProject {
            name: board.name,
            path: extract_path(&board.desc),
            ship_mode: ShipMode::Git,
            ..Default::default()
        },
        cards: sorted.into_iter().map(|(_, _, card)| card).collect(),
    })
}

fn comment(c: &CommentFile) -> (Actor, String, i64) {
    let actor = Actor {
        role: Role::Manager,
        name: c.member.full_name.clone(),
    };
    let created_at = c.date.as_deref().and_then(parse_time).unwrap_or_else(now);
    (actor, c.data.text.clone(), created_at)
}

fn column_for(list_name: &str) -> Column {
    [Column::Backlog, Column::Ready, Column::Doing, Column::Done]
        .into_iter()
        .find(|c| c.as_str().eq_ignore_ascii_case(list_name.trim()))
        .unwrap_or(Column::Backlog)
}

fn extract_path(desc: &str) -> Option<String> {
    let bytes = desc.as_bytes();
    let start = (0..bytes.len().saturating_sub(2)).find(|&i| {
        bytes[i].is_ascii_alphabetic()
            && bytes[i + 1] == b':'
            && matches!(bytes[i + 2], b'\\' | b'/')
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
    })?;
    let line = desc[start..].lines().next()?;
    let end = [" (", ". ", "; ", " - "]
        .iter()
        .filter_map(|sep| line.find(sep))
        .min()
        .unwrap_or(line.len());
    let path = line[..end].trim().trim_end_matches('.').trim();
    Some(path.to_string())
}

fn parse_time(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s).ok().map(|t| t.timestamp())
}

fn now() -> i64 {
    Utc::now().timestamp()
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
