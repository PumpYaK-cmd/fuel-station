use std::fs;

use serde_json::json;
use tempfile::TempDir;

use super::*;

fn write(path: &Path, value: serde_json::Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
}

struct TestCard<'a> {
    id: &'a str,
    list: &'a str,
    name: &'a str,
    pos: f64,
    closed: bool,
    comments: &'a [(&'a str, &'a str)],
}

fn card(dir: &Path, c: TestCard) {
    let comments: Vec<_> = c
        .comments
        .iter()
        .map(|(date, text)| {
            json!({"date": date, "memberCreator": {"fullName": "gabri"}, "data": {"text": text}})
        })
        .collect();
    write(
        &dir.join("cards").join(format!("{}.json", c.id)),
        json!({
            "idList": c.list,
            "name": c.name,
            "desc": format!("body of {}", c.name),
            "pos": c.pos,
            "closed": c.closed,
            "dateCreated": "2026-08-27T15:11:46.217275+00:00",
            "comments": comments,
        }),
    );
}

fn simple(dir: &Path, id: &str, list: &str, name: &str, pos: f64) {
    card(
        dir,
        TestCard {
            id,
            list,
            name,
            pos,
            closed: false,
            comments: &[],
        },
    );
}

fn board(root: &Path, id: &str, name: &str, desc: &str, closed: bool) -> PathBuf {
    let dir = root.join(id);
    write(
        &dir.join("board.json"),
        json!({"name": name, "desc": desc, "closed": closed}),
    );
    write(
        &dir.join("lists.json"),
        json!([
            {"id": "b", "name": "Backlog", "pos": 3.0, "closed": false},
            {"id": "d", "name": "doing", "pos": 2.0, "closed": false},
            {"id": "x", "name": "Done", "pos": 1.0, "closed": true},
            {"id": "o", "name": "Ideas", "pos": 4.0, "closed": false},
        ]),
    );
    dir
}

fn fake_trello() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    let dir = board(
        root.path(),
        "1",
        "fuel-station",
        "Rust app. Repo: C:\\Dev\\Fuel Station",
        false,
    );
    simple(&dir, "a", "b", "second backlog", 20.0);
    simple(&dir, "b", "b", "first backlog", 10.0);
    simple(&dir, "c", "o", "from ideas", 5.0);
    simple(&dir, "d", "x", "[HUMAN] click it", 1.0);
    card(
        &dir,
        TestCard {
            id: "e",
            list: "d",
            name: "gone",
            pos: 1.0,
            closed: true,
            comments: &[],
        },
    );
    card(
        &dir,
        TestCard {
            id: "f",
            list: "d",
            name: "doing with comments",
            pos: 1.0,
            closed: false,
            comments: &[
                ("2026-08-28T10:00:00+00:00", "later"),
                ("2026-08-27T10:00:00+00:00", "earlier"),
            ],
        },
    );
    let closed = board(root.path(), "2", "old-board", "", true);
    simple(&closed, "a", "b", "never seen", 1.0);
    root
}

#[test]
fn imports_open_boards_and_skips_closed() {
    let root = fake_trello();
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("station.db")).unwrap();
    let report = import_all(&mut store, root.path()).unwrap();
    assert_eq!(
        report.imported,
        vec![("FS".to_string(), "fuel-station".to_string(), 5)]
    );
    assert_eq!(
        report.skipped,
        vec![("old-board".to_string(), "closed".to_string())]
    );

    let project = store.project("FS").unwrap();
    assert_eq!(project.path.as_deref(), Some("C:\\Dev\\Fuel Station"));
    let board = store.board("FS", None).unwrap();
    let rows: Vec<_> = board
        .iter()
        .map(|c| {
            (
                c.key.as_str(),
                c.title.as_str(),
                c.column,
                c.position,
                c.human,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("FS-3", "first backlog", Column::Backlog, 0, false),
            ("FS-4", "second backlog", Column::Backlog, 1, false),
            ("FS-5", "from ideas", Column::Backlog, 2, false),
            ("FS-2", "doing with comments", Column::Doing, 0, false),
            ("FS-1", "[HUMAN] click it", Column::Done, 0, true),
        ]
    );
    let detail = store.card_show("FS-2").unwrap();
    let comments: Vec<_> = detail.comments.iter().map(|c| c.body.as_str()).collect();
    assert_eq!(comments, ["earlier", "later"]);
    assert_eq!(detail.comments[0].actor.role, Role::Manager);
    assert_eq!(detail.comments[0].actor.name, "gabri");
    assert_eq!(detail.comments[0].created_at, 1_787_824_800);
    assert_eq!(detail.card.created_at, 1_787_843_506);

    let again = import_all(&mut store, root.path()).unwrap();
    assert!(again.imported.is_empty());
    assert!(
        again
            .skipped
            .contains(&("fuel-station".to_string(), "already imported".to_string()))
    );
}

#[test]
fn colliding_keys_get_a_number() {
    let root = tempfile::tempdir().unwrap();
    for (id, name) in [("1", "yak-aviation"), ("2", "yak-automotive")] {
        let dir = board(root.path(), id, name, "", false);
        simple(&dir, "a", "b", "one", 1.0);
    }
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("station.db")).unwrap();
    let report = import_all(&mut store, root.path()).unwrap();
    let keys: Vec<_> = report.imported.iter().map(|i| i.0.as_str()).collect();
    assert_eq!(keys, ["YA", "YA2"]);
    assert_eq!(report.imported[0].1, "yak-automotive");
}

#[test]
fn path_extraction() {
    let cases = [
        (
            "Roblox game. Repo: C:\\Users\\gabri\\Documents\\Development\\Shototsu",
            Some("C:\\Users\\gabri\\Documents\\Development\\Shototsu"),
        ),
        (
            "Forge. Repo: C:\\Users\\gabri\\Documents\\Development\\Forge\\LOAD (source mirror of ServerStorage.LOAD; ships via ClankHub)",
            Some("C:\\Users\\gabri\\Documents\\Development\\Forge\\LOAD"),
        ),
        (
            "MSFS-based flight model, dev aircraft F35B. C:\\Users\\gabri\\Documents\\Development\\YaK Aviation 2",
            Some("C:\\Users\\gabri\\Documents\\Development\\YaK Aviation 2"),
        ),
        (
            "YaK Automotive. Repo: C:\\Users\\gabri\\Documents\\Development\\YaK Automotive. Spec in docs/, gates in docs/09.",
            Some("C:\\Users\\gabri\\Documents\\Development\\YaK Automotive"),
        ),
        (
            "Kitsu Pen app (Windows + Android). C:/Users/gabri/Documents/Development2/Kitsu Pen",
            Some("C:/Users/gabri/Documents/Development2/Kitsu Pen"),
        ),
        (
            "Fuel Station: Rust app. Repo: C:\\Users\\gabri\\Documents\\Development2\\Fuel Station",
            Some("C:\\Users\\gabri\\Documents\\Development2\\Fuel Station"),
        ),
        ("no path here", None),
    ];
    for (desc, want) in cases {
        assert_eq!(extract_path(desc).as_deref(), want, "{desc}");
    }
}
