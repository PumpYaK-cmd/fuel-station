use std::collections::HashSet;
use std::sync::Barrier;
use std::thread;

use tempfile::TempDir;

use super::*;

fn temp() -> (TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("station.db")).unwrap();
    (dir, store)
}

fn truck() -> Actor {
    Actor {
        role: Role::Truck,
        name: "truck".to_string(),
    }
}

fn with_project(store: &mut Store) -> Project {
    store
        .project_add(&NewProject {
            key: "fs".to_string(),
            name: "Fuel Station".to_string(),
            ..Default::default()
        })
        .unwrap()
}

fn add(store: &mut Store, column: Column, title: &str) -> Card {
    store.card_add("FS", &truck(), column, title, "").unwrap()
}

fn event_count(store: &Store) -> usize {
    store.events("FS", 10_000).unwrap().len()
}

#[test]
fn project_names_include_archived() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    store
        .project_add(&NewProject {
            key: "old".to_string(),
            name: "Old".to_string(),
            ..Default::default()
        })
        .unwrap();
    store
        .conn
        .execute("UPDATE projects SET archived = 1 WHERE key = 'OLD'", [])
        .unwrap();
    let listed: Vec<_> = store
        .projects()
        .unwrap()
        .into_iter()
        .map(|p| p.key)
        .collect();
    assert_eq!(listed, ["FS"]);
    assert_eq!(
        store.project_names().unwrap(),
        [
            ("FS".to_string(), "Fuel Station".to_string()),
            ("OLD".to_string(), "Old".to_string()),
        ]
    );
}

#[test]
fn migrations_are_idempotent_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("station.db");
    let mut store = Store::open(&path).unwrap();
    with_project(&mut store);
    drop(store);
    let store = Store::open(&path).unwrap();
    let version: i64 = store
        .conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, schema::MIGRATIONS.len() as i64);
    assert_eq!(store.projects().unwrap().len(), 1);
    store
        .conn
        .pragma_update(None, "user_version", 99_i64)
        .unwrap();
    drop(store);
    assert!(matches!(Store::open(&path), Err(StoreError::Invalid(_))));
}

#[test]
fn project_key_is_uppercased_and_validated() {
    let (_dir, mut store) = temp();
    let project = with_project(&mut store);
    assert_eq!(project.key, "FS");
    assert_eq!(project.ship_mode, ShipMode::Git);
    assert_eq!(store.project("fs").unwrap().name, "Fuel Station");
    for bad in ["", "TOOLONGKEY", "F-S", "FS"] {
        let err = store.project_add(&NewProject {
            key: bad.to_string(),
            ..Default::default()
        });
        assert!(matches!(err, Err(StoreError::Invalid(_))), "{bad:?}");
    }
    let updated = store
        .project_update(
            "FS",
            &NewProject {
                name: "Renamed".to_string(),
                ship_mode: ShipMode::Studio,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.ship_mode, ShipMode::Studio);
    let rekey = store.project_update(
        "FS",
        &NewProject {
            key: "XX".to_string(),
            ..Default::default()
        },
    );
    assert!(matches!(rekey, Err(StoreError::Invalid(_))));
    assert!(matches!(
        store.project("NOPE"),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn card_add_numbers_and_positions() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    let a = add(&mut store, Column::Ready, "a");
    let b = add(&mut store, Column::Ready, "b");
    let c = add(&mut store, Column::Backlog, "c");
    assert_eq!((a.key.as_str(), a.position), ("FS-1", 0));
    assert_eq!((b.key.as_str(), b.position), ("FS-2", 1));
    assert_eq!((c.key.as_str(), c.position), ("FS-3", 0));
    let board = store.board("FS", None).unwrap();
    let keys: Vec<&str> = board.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(keys, ["FS-3", "FS-1", "FS-2"]);
    assert_eq!(store.board("FS", Some(Column::Ready)).unwrap().len(), 2);
}

#[test]
fn card_keys_parse() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    add(&mut store, Column::Ready, "a");
    assert_eq!(store.card("fs-1").unwrap().title, "a");
    for bad in ["FS", "FS-", "-1", "FS-x", "FS-0"] {
        assert!(
            matches!(store.card(bad), Err(StoreError::Invalid(_))),
            "{bad:?}"
        );
    }
    match store.card("FS-9") {
        Err(StoreError::NotFound(key)) => assert_eq!(key, "FS-9"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn move_appends_to_end() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    add(&mut store, Column::Doing, "a");
    add(&mut store, Column::Doing, "b");
    let c = add(&mut store, Column::Ready, "c");
    let moved = store.card_move(&c.key, &truck(), Column::Doing).unwrap();
    assert_eq!(moved.column, Column::Doing);
    assert_eq!(moved.position, 2);
}

#[test]
fn reorder_renumbers_column() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    for title in ["a", "b", "c", "d"] {
        add(&mut store, Column::Ready, title);
    }
    store.card_reorder("FS-4", &truck(), 1).unwrap();
    store.card_reorder("FS-1", &truck(), 99).unwrap();
    let board = store.board("FS", Some(Column::Ready)).unwrap();
    let order: Vec<(&str, i64)> = board.iter().map(|c| (c.key.as_str(), c.position)).collect();
    assert_eq!(order, [("FS-4", 0), ("FS-2", 1), ("FS-3", 2), ("FS-1", 3)]);
}

#[test]
fn card_update_patches_fields() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    add(&mut store, Column::Ready, "a");
    let card = store
        .card_update(
            "FS-1",
            &truck(),
            &CardPatch {
                plan: Some("plan".to_string()),
                tier: Some("2".to_string()),
                human: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(card.title, "a");
    assert_eq!(card.plan, "plan");
    assert_eq!(card.tier.as_deref(), Some("2"));
    assert!(card.human);
}

#[test]
fn card_update_empty_tier_clears_it() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    add(&mut store, Column::Ready, "a");
    let patch = |tier: Option<&str>| CardPatch {
        tier: tier.map(str::to_string),
        ..Default::default()
    };
    store
        .card_update("FS-1", &truck(), &patch(Some("2")))
        .unwrap();
    let card = store.card_update("FS-1", &truck(), &patch(None)).unwrap();
    assert_eq!(card.tier.as_deref(), Some("2"));
    let card = store
        .card_update("FS-1", &truck(), &patch(Some("")))
        .unwrap();
    assert_eq!(card.tier, None);
}

#[test]
fn grab_takes_top_non_human() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    assert!(store.grab("FS", &truck(), Column::Ready).unwrap().is_none());
    add(&mut store, Column::Ready, "human");
    add(&mut store, Column::Ready, "machine");
    add(&mut store, Column::Ready, "later");
    store
        .card_update(
            "FS-1",
            &truck(),
            &CardPatch {
                human: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    let grabbed = store.grab("FS", &truck(), Column::Ready).unwrap().unwrap();
    assert_eq!(grabbed.key, "FS-2");
    assert_eq!(grabbed.column, Column::Doing);
    let grabbed = store.grab("FS", &truck(), Column::Ready).unwrap().unwrap();
    assert_eq!(grabbed.key, "FS-3");
    assert!(store.grab("FS", &truck(), Column::Ready).unwrap().is_none());
    assert_eq!(store.card("FS-1").unwrap().column, Column::Ready);
}

fn race(mut store: Store, barrier: &Barrier, name: &str) -> Vec<String> {
    let actor = Actor {
        role: Role::Fuel,
        name: name.to_string(),
    };
    barrier.wait();
    let mut keys = Vec::new();
    while let Some(card) = store.grab("FS", &actor, Column::Ready).unwrap() {
        keys.push(card.key);
    }
    keys
}

#[test]
fn racing_grabs_take_each_card_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("station.db");
    let mut store = Store::open(&path).unwrap();
    with_project(&mut store);
    for i in 0..20 {
        add(&mut store, Column::Ready, &format!("card {i}"));
    }
    let a = Store::open(&path).unwrap();
    let b = Store::open(&path).unwrap();
    let barrier = Barrier::new(2);
    let (a, b) = thread::scope(|s| {
        let a = s.spawn(|| race(a, &barrier, "a"));
        let b = s.spawn(|| race(b, &barrier, "b"));
        (a.join().unwrap(), b.join().unwrap())
    });
    let all: HashSet<String> = a.iter().chain(&b).cloned().collect();
    assert_eq!(a.len() + b.len(), 20);
    assert_eq!(all.len(), 20);
    assert_eq!(store.board("FS", Some(Column::Doing)).unwrap().len(), 20);
}

#[test]
fn review_flow_transitions() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    add(&mut store, Column::Ready, "a");
    let mechanic = Actor {
        role: Role::Mechanic,
        name: "mechanic".to_string(),
    };
    assert!(matches!(
        store.submit_review("FS-1", &truck(), "base", "head"),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.review_result("FS-1", &mechanic, Verdict::Clean, &[]),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.ship("FS-1", &truck(), "done", "abc"),
        Err(StoreError::Invalid(_))
    ));
    store.card_move("FS-1", &truck(), Column::Doing).unwrap();
    let card = store
        .submit_review("FS-1", &truck(), "base", "head")
        .unwrap();
    assert_eq!(card.column, Column::Review);
    assert_eq!(card.commit_base.as_deref(), Some("base"));
    assert_eq!(card.commit_head.as_deref(), Some("head"));
    let findings = vec!["one".to_string(), "two".to_string()];
    let card = store
        .review_result("FS-1", &mechanic, Verdict::Findings, &findings)
        .unwrap();
    assert_eq!(card.column, Column::Doing);
    store
        .submit_review("FS-1", &truck(), "base", "head2")
        .unwrap();
    let card = store
        .review_result("FS-1", &mechanic, Verdict::Clean, &[])
        .unwrap();
    assert_eq!(card.column, Column::Review);
    let card = store
        .ship("FS-1", &truck(), "shipped it", "abc123")
        .unwrap();
    assert_eq!(card.column, Column::Done);
    assert_eq!(card.commit_head.as_deref(), Some("abc123"));
    let detail = store.card_show("FS-1").unwrap();
    let bodies: Vec<&str> = detail.comments.iter().map(|c| c.body.as_str()).collect();
    assert_eq!(
        bodies,
        [
            "Review: 2 findings\n- one\n- two",
            "Review: clean",
            "shipped it\n\nCommit: abc123"
        ]
    );
    assert_eq!(detail.events[0].kind, "ship");
}

#[test]
fn every_mutation_writes_an_event() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    let actor = truck();
    let step = |store: &mut Store, kind: &'static str, f: &dyn Fn(&mut Store)| {
        let before = event_count(store);
        f(store);
        assert_eq!(event_count(store), before + 1, "{kind}");
        assert_eq!(store.events("FS", 1).unwrap()[0].kind, kind);
    };
    step(&mut store, "project_update", &|s| {
        s.project_update("FS", &NewProject::default()).unwrap();
    });
    step(&mut store, "card_add", &|s| {
        s.card_add("FS", &actor, Column::Ready, "a", "").unwrap();
    });
    step(&mut store, "card_add", &|s| {
        s.card_add("FS", &actor, Column::Ready, "b", "").unwrap();
    });
    step(&mut store, "card_update", &|s| {
        s.card_update("FS-1", &actor, &CardPatch::default())
            .unwrap();
    });
    step(&mut store, "card_reorder", &|s| {
        s.card_reorder("FS-2", &actor, 0).unwrap();
    });
    step(&mut store, "grab", &|s| {
        s.grab("FS", &actor, Column::Ready).unwrap().unwrap();
    });
    step(&mut store, "card_move", &|s| {
        s.card_move("FS-1", &actor, Column::Doing).unwrap();
    });
    step(&mut store, "comment", &|s| {
        s.comment("FS-1", &actor, "hi").unwrap();
    });
    step(&mut store, "report", &|s| {
        s.report("FS-1", &actor, "halfway").unwrap();
    });
    step(&mut store, "submit_review", &|s| {
        s.submit_review("FS-1", &actor, "a", "b").unwrap();
    });
    step(&mut store, "review_result", &|s| {
        s.review_result("FS-1", &actor, Verdict::Clean, &[])
            .unwrap();
    });
    step(&mut store, "ship", &|s| {
        s.ship("FS-1", &actor, "done", "b").unwrap();
    });
    step(&mut store, "checkin", &|s| {
        s.checkin("FS", &actor, Some("FS-2")).unwrap();
    });
    step(&mut store, "checkout", &|s| {
        s.checkout("FS", &actor).unwrap();
    });
    let event = &store.events("FS", 1).unwrap()[0];
    assert_eq!(event.actor, actor);
    let project_event = store.events("FS", 100).unwrap().pop().unwrap();
    assert_eq!(project_event.kind, "project_add");
    assert_eq!(project_event.actor.name, "station");
    assert_eq!(project_event.card_key, None);
}

#[test]
fn agents_liveness_window() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    add(&mut store, Column::Doing, "a");
    let fuel = Actor {
        role: Role::Fuel,
        name: "fuel-1".to_string(),
    };
    store.checkin("FS", &fuel, Some("FS-1")).unwrap();
    store.checkin("FS", &truck(), None).unwrap();
    let agents = store.agents("FS").unwrap();
    assert_eq!(agents.len(), 2);
    let board = store.board("FS", None).unwrap();
    assert_eq!(board[0].holder.as_ref(), Some(&fuel));

    store
        .conn
        .execute(
            "UPDATE agents SET last_seen = last_seen - 200 WHERE name = 'fuel-1'",
            [],
        )
        .unwrap();
    let agents = store.agents("FS").unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].actor, truck());
    assert!(store.board("FS", None).unwrap()[0].holder.is_none());

    store.checkin("FS", &fuel, Some("FS-1")).unwrap();
    assert_eq!(store.agents("FS").unwrap().len(), 2);
    store.checkout("FS", &truck()).unwrap();
    let agents = store.agents("FS").unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].card_key.as_deref(), Some("FS-1"));
}

#[test]
fn data_version_changes_after_other_connection_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("station.db");
    let watcher = Store::open(&path).unwrap();
    let mut writer = Store::open(&path).unwrap();
    let before = watcher.data_version().unwrap();
    with_project(&mut writer);
    assert_ne!(watcher.data_version().unwrap(), before);
}

#[test]
fn events_after_spans_projects_in_order() {
    let (_dir, mut store) = temp();
    with_project(&mut store);
    store
        .project_add(&NewProject {
            key: "ot".to_string(),
            name: "Other".to_string(),
            ..Default::default()
        })
        .unwrap();
    let mark = store.last_event_id().unwrap();
    add(&mut store, Column::Ready, "a");
    store
        .card_add("OT", &truck(), Column::Ready, "b", "")
        .unwrap();
    store.card_move("FS-1", &truck(), Column::Doing).unwrap();
    let events = store.events_after(mark).unwrap();
    let seen: Vec<(&str, &str)> = events
        .iter()
        .map(|e| (e.project_key.as_str(), e.kind.as_str()))
        .collect();
    assert_eq!(
        seen,
        [("FS", "card_add"), ("OT", "card_add"), ("FS", "card_move")]
    );
    assert!(events.windows(2).all(|w| w[0].id < w[1].id));
    assert_eq!(store.last_event_id().unwrap(), events[2].id);
    assert!(store.events_after(events[2].id).unwrap().is_empty());
}

#[test]
fn columns_parse_and_step() {
    assert_eq!("doing".parse::<Column>().unwrap(), Column::Doing);
    assert!("nope".parse::<Column>().is_err());
    assert_eq!(Column::Backlog.next(), Some(Column::Ready));
    assert_eq!(Column::Done.next(), None);
    assert_eq!(Column::Backlog.prev(), None);
    assert_eq!(Column::Done.prev(), Some(Column::Review));
}

#[test]
fn import_board_round_trip() {
    let (_dir, mut store) = temp();
    let manager = Actor {
        role: Role::Manager,
        name: "gabri".to_string(),
    };
    let card = |column, title: &str, comments: Vec<(Actor, String, i64)>| ImportCard {
        column,
        title: title.to_string(),
        body: format!("{title} body"),
        human: title.starts_with("[HUMAN]"),
        created_at: 1_000,
        comments,
    };
    let board = ImportBoard {
        project: NewProject {
            key: "im".to_string(),
            name: "Imported".to_string(),
            ..Default::default()
        },
        cards: vec![
            card(Column::Done, "old", vec![]),
            card(
                Column::Backlog,
                "[HUMAN] a",
                vec![
                    (manager.clone(), "one".to_string(), 50),
                    (manager.clone(), "two".to_string(), 60),
                ],
            ),
            card(Column::Backlog, "b", vec![]),
        ],
    };
    let project = store.import_board(&board).unwrap();
    assert_eq!(project.key, "IM");
    let rows: Vec<_> = store
        .board("IM", None)
        .unwrap()
        .into_iter()
        .map(|c| (c.key, c.column, c.position, c.human))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("IM-2".to_string(), Column::Backlog, 0, true),
            ("IM-3".to_string(), Column::Backlog, 1, false),
            ("IM-1".to_string(), Column::Done, 0, false),
        ]
    );
    let detail = store.card_show("IM-2").unwrap();
    assert_eq!(detail.card.body, "[HUMAN] a body");
    assert_eq!(detail.card.created_at, 1_000);
    assert_eq!(detail.comments.len(), 2);
    assert_eq!(detail.comments[1].body, "two");
    assert_eq!(detail.comments[1].created_at, 60);
    assert_eq!(detail.comments[1].actor, manager);
    let events = store.events("IM", 10).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "import");
    assert_eq!(events[0].message, "imported 3 cards from trello");
    assert!(store.import_board(&board).is_err());
}

#[test]
fn import_board_numbers_by_creation() {
    let (_dir, mut store) = temp();
    let card = |column, title: &str, created_at| ImportCard {
        column,
        title: title.to_string(),
        body: String::new(),
        human: false,
        created_at,
        comments: vec![],
    };
    let board = ImportBoard {
        project: NewProject {
            key: "im".to_string(),
            name: "Imported".to_string(),
            ..Default::default()
        },
        cards: vec![
            card(Column::Done, "newest", 300),
            card(Column::Backlog, "oldest", 100),
            card(Column::Ready, "middle", 200),
        ],
    };
    store.import_board(&board).unwrap();
    let rows: Vec<_> = store
        .board("IM", None)
        .unwrap()
        .into_iter()
        .map(|c| (c.key, c.title, c.column, c.position))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("IM-1".to_string(), "oldest".to_string(), Column::Backlog, 0),
            ("IM-2".to_string(), "middle".to_string(), Column::Ready, 0),
            ("IM-3".to_string(), "newest".to_string(), Column::Done, 0),
        ]
    );
}

#[test]
fn derive_key_rules() {
    let none: Vec<String> = vec![];
    assert_eq!(derive_key("fuel-station", &none), "FS");
    assert_eq!(derive_key("Fuel Station", &none), "FS");
    assert_eq!(derive_key("shototsu", &none), "SHOT");
    assert_eq!(derive_key("yak-aviation", &none), "YA");
    assert_eq!(
        derive_key("one-two-three-four-five-six-seven-eight-nine", &none),
        "OTTFFSSE"
    );
    let taken = vec!["YA".to_string()];
    assert_eq!(derive_key("yak-automotive", &taken), "YA2");
    let taken = vec!["YA".to_string(), "YA2".to_string()];
    assert_eq!(derive_key("yak-aviation", &taken), "YA3");
    assert_eq!(derive_key("!!!", &none), "P");
}
