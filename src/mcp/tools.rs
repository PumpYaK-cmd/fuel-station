use std::fmt::Write;

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use super::Station;
use crate::store::{
    self, Actor, CardDetail, CardPatch, CardSummary, Column, Project, Store, Verdict,
};

#[derive(Deserialize, JsonSchema)]
struct BoardParams {
    /// Only this column; omit for every column.
    column: Option<Column>,
}

#[derive(Deserialize, JsonSchema)]
struct CardParams {
    /// Card id, e.g. FS-3.
    id: String,
}

#[derive(Deserialize, JsonSchema)]
struct CardAddParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Column the card starts in.
    column: Column,
    /// One-line card title.
    title: String,
    /// Card description, markdown.
    body: String,
}

#[derive(Deserialize, JsonSchema)]
struct CardUpdateParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id, e.g. FS-3.
    id: String,
    /// New title; omit to keep.
    title: Option<String>,
    /// New description, markdown; omit to keep.
    body: Option<String>,
    /// New plan, markdown; omit to keep.
    plan: Option<String>,
    /// New tier; omit to keep, empty tier clears it.
    tier: Option<String>,
    /// Whether the card needs the user's hands; omit to keep.
    human: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct CardMoveParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id, e.g. FS-3.
    id: String,
    /// Destination column.
    column: Column,
}

#[derive(Deserialize, JsonSchema)]
struct GrabParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Column to grab from; defaults to ready.
    from: Option<Column>,
}

#[derive(Deserialize, JsonSchema)]
struct CommentParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id, e.g. FS-3.
    id: String,
    /// Comment text, markdown.
    body: String,
}

#[derive(Deserialize, JsonSchema)]
struct ReportParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id, e.g. FS-3.
    id: String,
    /// One progress line.
    message: String,
}

#[derive(Deserialize, JsonSchema)]
struct SubmitReviewParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id, e.g. FS-3.
    id: String,
    /// Commit the work started from.
    base: String,
    /// Commit to review.
    head: String,
}

#[derive(Deserialize, JsonSchema)]
struct ReviewResultParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id, e.g. FS-3.
    id: String,
    /// clean keeps the card in Review; findings sends it back to Doing.
    verdict: Verdict,
    /// One line per finding; empty when clean.
    #[serde(default)]
    findings: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ShipParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id, e.g. FS-3.
    id: String,
    /// What changed, for the closing comment.
    summary: String,
    /// Commit that shipped the card.
    commit: String,
}

#[derive(Deserialize, JsonSchema)]
struct CheckinParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
    /// Card id you are working on, e.g. FS-3; omit when not on a card.
    card: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ActorParams {
    /// Who is calling: role is manager|truck|fuel|mechanic, name is your agent name.
    actor: Actor,
}

impl Station {
    fn with<T>(&self, f: impl FnOnce(&mut Store) -> store::Result<T>) -> Result<T, String> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| "store lock poisoned".to_string())?;
        f(&mut store).map_err(|e| e.to_string())
    }

    fn own(&self, id: &str) -> Result<(), String> {
        match id.rsplit_once('-') {
            Some((key, _)) if !key.eq_ignore_ascii_case(&self.project) => Err(format!(
                "{id} is not in project {}",
                self.project.to_ascii_uppercase()
            )),
            _ => Ok(()),
        }
    }
}

#[tool_router(vis = "pub(super)")]
impl Station {
    #[tool(
        description = "Show the project's verify gate, default branch, ship mode, worktree dir, bootstrap and ship command."
    )]
    async fn project_info(&self) -> Result<String, String> {
        let project = self.with(|s| s.project(&self.project))?;
        Ok(project_text(&project))
    }

    #[tool(description = "List the cards in every column, or in one column.")]
    async fn board(&self, Parameters(p): Parameters<BoardParams>) -> Result<String, String> {
        let cards = self.with(|s| s.board(&self.project, p.column))?;
        Ok(board_text(&cards, p.column))
    }

    #[tool(description = "Show a card with its plan, comments and last 20 events.")]
    async fn card_show(&self, Parameters(p): Parameters<CardParams>) -> Result<String, String> {
        self.own(&p.id)?;
        let detail = self.with(|s| s.card_show(&p.id))?;
        Ok(card_text(&detail))
    }

    #[tool(description = "Add a new card to a column.")]
    async fn card_add(&self, Parameters(p): Parameters<CardAddParams>) -> Result<String, String> {
        let card =
            self.with(|s| s.card_add(&self.project, &p.actor, p.column, &p.title, &p.body))?;
        Ok(format!(
            "{} added to {}: {}",
            card.key,
            card.column.as_str(),
            card.title
        ))
    }

    #[tool(description = "Edit a card's title, body, plan, tier or human flag.")]
    async fn card_update(
        &self,
        Parameters(p): Parameters<CardUpdateParams>,
    ) -> Result<String, String> {
        let patch = CardPatch {
            title: p.title,
            body: p.body,
            plan: p.plan,
            tier: p.tier,
            human: p.human,
        };
        self.own(&p.id)?;
        let card = self.with(|s| s.card_update(&p.id, &p.actor, &patch))?;
        Ok(format!("{} updated: {}", card.key, card.title))
    }

    #[tool(description = "Move a card to another column.")]
    async fn card_move(&self, Parameters(p): Parameters<CardMoveParams>) -> Result<String, String> {
        self.own(&p.id)?;
        let card = self.with(|s| s.card_move(&p.id, &p.actor, p.column))?;
        Ok(format!("{} -> {}", card.key, card.column.as_str()))
    }

    #[tool(
        description = "Atomically claim the top non-human card of a column (default ready) into Doing."
    )]
    async fn grab(&self, Parameters(p): Parameters<GrabParams>) -> Result<String, String> {
        let from = p.from.unwrap_or(Column::Ready);
        Ok(
            match self.with(|s| s.grab(&self.project, &p.actor, from))? {
                Some(card) => format!("{} -> Doing: {}", card.key, card.title),
                None => format!("nothing to grab in {}", from.as_str()),
            },
        )
    }

    #[tool(description = "Add a comment to a card.")]
    async fn comment(&self, Parameters(p): Parameters<CommentParams>) -> Result<String, String> {
        self.own(&p.id)?;
        let comment = self.with(|s| s.comment(&p.id, &p.actor, &p.body))?;
        Ok(format!("Commented on {}", comment.card_key))
    }

    #[tool(description = "Post a progress line to the activity feed only, not as a comment.")]
    async fn report(&self, Parameters(p): Parameters<ReportParams>) -> Result<String, String> {
        self.own(&p.id)?;
        let key = self.with(|s| {
            s.report(&p.id, &p.actor, &p.message)?;
            Ok(s.card(&p.id)?.key)
        })?;
        Ok(format!("Reported on {key}"))
    }

    #[tool(description = "Move a card from Doing to Review and record its commit range.")]
    async fn submit_review(
        &self,
        Parameters(p): Parameters<SubmitReviewParams>,
    ) -> Result<String, String> {
        self.own(&p.id)?;
        let card = self.with(|s| s.submit_review(&p.id, &p.actor, &p.base, &p.head))?;
        Ok(format!(
            "{} -> {} (base {}, head {})",
            card.key,
            card.column.as_str(),
            p.base,
            p.head
        ))
    }

    #[tool(
        description = "Record the Mechanic's verdict on a card in Review: clean or a findings list."
    )]
    async fn review_result(
        &self,
        Parameters(p): Parameters<ReviewResultParams>,
    ) -> Result<String, String> {
        self.own(&p.id)?;
        match p.verdict {
            Verdict::Clean if !p.findings.is_empty() => {
                return Err("clean verdict takes no findings".to_string());
            }
            Verdict::Findings if p.findings.is_empty() => {
                return Err("findings verdict needs at least one finding".to_string());
            }
            _ => {}
        }
        let card = self.with(|s| s.review_result(&p.id, &p.actor, p.verdict, &p.findings))?;
        let verdict = match p.verdict {
            Verdict::Clean => "clean".to_string(),
            Verdict::Findings => format!("{} findings", p.findings.len()),
        };
        Ok(format!(
            "{} {verdict} -> {}",
            card.key,
            card.column.as_str()
        ))
    }

    #[tool(description = "Move a card from Review to Done with a closing comment and the commit.")]
    async fn ship(&self, Parameters(p): Parameters<ShipParams>) -> Result<String, String> {
        self.own(&p.id)?;
        let card = self.with(|s| s.ship(&p.id, &p.actor, &p.summary, &p.commit))?;
        Ok(format!(
            "{} -> {} (commit {})",
            card.key,
            card.column.as_str(),
            p.commit
        ))
    }

    #[tool(description = "Mark yourself active in the agents strip, optionally on a card.")]
    async fn checkin(&self, Parameters(p): Parameters<CheckinParams>) -> Result<String, String> {
        let key = self.with(|s| {
            s.checkin(&self.project, &p.actor, p.card.as_deref())?;
            p.card.as_deref().map(|id| Ok(s.card(id)?.key)).transpose()
        })?;
        Ok(match key {
            Some(key) => format!("{} checked in on {key}", p.actor.name),
            None => format!("{} checked in", p.actor.name),
        })
    }

    #[tool(description = "Remove yourself from the agents strip.")]
    async fn checkout(&self, Parameters(p): Parameters<ActorParams>) -> Result<String, String> {
        self.with(|s| s.checkout(&self.project, &p.actor))?;
        Ok(format!("{} checked out", p.actor.name))
    }
}

fn time(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

fn who(actor: &Actor) -> String {
    format!("{}:{}", actor.role.as_str(), actor.name)
}

fn project_text(p: &Project) -> String {
    [
        ("key", p.key.clone()),
        ("name", p.name.clone()),
        ("path", p.path.clone().unwrap_or_default()),
        ("ship_mode", p.ship_mode.as_str().to_string()),
        ("default_branch", p.default_branch.clone()),
        ("verify_cmd", p.verify_cmd.clone()),
        ("worktree_dir", p.worktree_dir.clone()),
        ("bootstrap_cmd", p.bootstrap_cmd.clone()),
        ("ship_cmd", p.ship_cmd.clone()),
        ("created_at", time(p.created_at)),
        ("archived", p.archived.to_string()),
    ]
    .iter()
    .map(|(k, v)| format!("{k}: {v}"))
    .collect::<Vec<_>>()
    .join("\n")
}

fn board_text(cards: &[CardSummary], column: Option<Column>) -> String {
    let columns: Vec<Column> = match column {
        Some(c) => vec![c],
        None => Column::ALL.to_vec(),
    };
    let mut blocks = Vec::new();
    for col in columns {
        let in_col: Vec<&CardSummary> = cards.iter().filter(|c| c.column == col).collect();
        let mut block = format!("{} ({})", col.as_str(), in_col.len());
        for card in in_col {
            let mut parts = vec![card.key.clone(), card.title.clone()];
            if let Some(tier) = &card.tier {
                parts.push(format!("[{tier}]"));
            }
            if card.human {
                parts.push("{human}".to_string());
            }
            if let Some(holder) = &card.holder {
                parts.push(format!("@{}", who(holder)));
            }
            write!(block, "\n  {}", parts.join("  ")).unwrap();
        }
        blocks.push(block);
    }
    blocks.join("\n\n")
}

fn card_text(detail: &CardDetail) -> String {
    let card = &detail.card;
    let mut out = format!("{} [{}] {}\n", card.key, card.column.as_str(), card.title);
    let mut facts = Vec::new();
    if let Some(tier) = &card.tier {
        facts.push(format!("tier: {tier}"));
    }
    if card.human {
        facts.push("human: yes".to_string());
    }
    if let Some(base) = &card.commit_base {
        facts.push(format!("base: {base}"));
    }
    if let Some(head) = &card.commit_head {
        facts.push(format!("head: {head}"));
    }
    if !facts.is_empty() {
        writeln!(out, "{}", facts.join("  ")).unwrap();
    }
    write!(
        out,
        "\n## Body\n{}\n\n## Plan\n{}\n\n## Comments\n",
        card.body, card.plan
    )
    .unwrap();
    for comment in &detail.comments {
        write!(
            out,
            "\n{} {}\n{}\n",
            time(comment.created_at),
            who(&comment.actor),
            comment.body
        )
        .unwrap();
    }
    out.push_str("\n## Events\n");
    for event in detail.events.iter().rev() {
        writeln!(
            out,
            "{} {}  {}",
            time(event.created_at),
            who(&event.actor),
            event.message
        )
        .unwrap();
    }
    out
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::store::{NewProject, Role};

    fn station() -> (TempDir, Station) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("station.db")).unwrap();
        store
            .project_add(&NewProject {
                key: "FS".to_string(),
                name: "Fuel Station".to_string(),
                ..Default::default()
            })
            .unwrap();
        (dir, Station::new("FS", store))
    }

    fn fuel() -> Actor {
        Actor {
            role: Role::Fuel,
            name: "f1".to_string(),
        }
    }

    async fn add(station: &Station, title: &str) {
        station
            .card_add(Parameters(CardAddParams {
                actor: fuel(),
                column: Column::Ready,
                title: title.to_string(),
                body: String::new(),
            }))
            .await
            .unwrap();
    }

    async fn update(station: &Station, id: &str, tier: Option<&str>, human: Option<bool>) {
        station
            .card_update(Parameters(CardUpdateParams {
                actor: fuel(),
                id: id.to_string(),
                title: None,
                body: None,
                plan: None,
                tier: tier.map(str::to_string),
                human,
            }))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn board_lists_cards_per_column() {
        let (_dir, station) = station();
        add(&station, "first").await;
        add(&station, "second").await;
        update(&station, "FS-1", Some("2"), Some(true)).await;
        station
            .checkin(Parameters(CheckinParams {
                actor: fuel(),
                card: Some("FS-2".to_string()),
            }))
            .await
            .unwrap();

        let ready = station
            .board(Parameters(BoardParams {
                column: Some(Column::Ready),
            }))
            .await
            .unwrap();
        assert_eq!(
            ready,
            "Ready (2)\n  FS-1  first  [2]  {human}\n  FS-2  second  @fuel:f1"
        );

        let all = station
            .board(Parameters(BoardParams { column: None }))
            .await
            .unwrap();
        assert!(all.starts_with("Backlog (0)\n\nReady (2)\n"));
        assert!(all.ends_with("Done (0)"));
    }

    #[tokio::test]
    async fn card_show_header_and_tier_clear() {
        let (_dir, station) = station();
        add(&station, "first").await;
        update(&station, "FS-1", Some("2"), None).await;
        let show = |id: &str| station.card_show(Parameters(CardParams { id: id.to_string() }));

        let text = show("fs-1").await.unwrap();
        assert!(text.starts_with("FS-1 [Ready] first\ntier: 2\n\n## Body\n"));
        assert!(text.contains("fuel:f1  Added FS-1 to Ready: first\n"));

        update(&station, "FS-1", Some(""), None).await;
        let text = show("FS-1").await.unwrap();
        assert!(text.starts_with("FS-1 [Ready] first\n\n## Body\n"));

        assert_eq!(show("FS-9").await.unwrap_err(), "not found: FS-9");
        assert_eq!(show("XY-1").await.unwrap_err(), "XY-1 is not in project FS");
    }

    #[tokio::test]
    async fn grab_on_empty_column_is_not_an_error() {
        let (_dir, station) = station();
        let text = station
            .grab(Parameters(GrabParams {
                actor: fuel(),
                from: None,
            }))
            .await
            .unwrap();
        assert_eq!(text, "nothing to grab in Ready");
    }
}
