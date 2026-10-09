mod activity;
mod detail;
pub mod theme;

use std::time::Duration;

use iced::keyboard::{self, Key, key::Named};
use iced::widget::{
    button, column, container, mouse_area, pick_list, row, rule, scrollable, space, text,
    text_editor,
};
use iced::{Alignment, Element, Length, Subscription, Theme, mouse};

use crate::setup::agents;
use crate::store::{
    self, Actor, Agent, CardPatch, CardSummary, Column, Event, Project, Role, Store, StoreError,
};
use detail::Detail;

pub fn run() {
    let result = iced::application(boot, update, view)
        .title("Fuel Station")
        .subscription(subscription)
        .theme(app_theme)
        .font(include_bytes!("../../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf").as_slice())
        .default_font(theme::FONT)
        .run();
    if let Err(e) = result {
        eprintln!("fuel-station: {e}");
    }
}

type State = Result<App, String>;

struct App {
    store: Store,
    projects: Vec<Project>,
    selected: Option<String>,
    board: Vec<CardSummary>,
    data_version: i64,
    error: Option<String>,
    theme: Theme,
    themes: Vec<Theme>,
    detail: Option<Detail>,
    events: Vec<Event>,
    agents: Vec<Agent>,
    ticks: u32,
}

#[derive(Debug, Clone)]
enum Message {
    SelectProject(String),
    Tick,
    ThemeChanged(Theme),
    OpenCard(String),
    CloseDetail,
    TitleEdited(String),
    BodyAction(text_editor::Action),
    Save,
    CommentEdited(String),
    AddComment,
    MoveTo(Column),
    Step(bool),
    LinkClicked,
}

fn boot() -> State {
    let store = Store::open_default()
        .map_err(|e| format!("cannot open {}: {e}", store::db_path().display()))?;
    let mut app = App {
        store,
        projects: Vec::new(),
        selected: None,
        board: Vec::new(),
        data_version: 0,
        error: None,
        theme: theme::theme(),
        themes: std::iter::once(theme::theme())
            .chain(Theme::ALL.iter().cloned())
            .collect(),
        detail: None,
        events: Vec::new(),
        agents: Vec::new(),
        ticks: 0,
    };
    app.reload(None);
    if let Err(e) = agents::install_agents(&agents::agents_dir()) {
        app.error = Some(format!("agent files: {e}"));
    }
    Ok(app)
}

fn update(state: &mut State, message: Message) {
    let Ok(app) = state else {
        return;
    };
    match message {
        Message::SelectProject(key) => app.reload(Some(key)),
        Message::Tick => {
            app.ticks = app.ticks.wrapping_add(1);
            match app.store.data_version() {
                Ok(version) if version != app.data_version || app.ticks % 17 == 0 => {
                    app.reload(app.selected.clone())
                }
                Ok(_) => app.error = None,
                Err(e) => app.error = Some(e.to_string()),
            }
        }
        Message::ThemeChanged(theme) => app.theme = theme,
        Message::OpenCard(key) => match app.store.card_show(&key) {
            Ok(card) => app.detail = Some(Detail::new(card)),
            Err(e) => app.error = Some(e.to_string()),
        },
        Message::CloseDetail => app.detail = None,
        Message::TitleEdited(title) => {
            if let Some(d) = &mut app.detail {
                d.title = title;
                d.dirty = true;
            }
        }
        Message::BodyAction(action) => {
            if let Some(d) = &mut app.detail {
                d.dirty |= action.is_edit();
                d.body.perform(action);
            }
        }
        Message::Save => {
            let Some(d) = &mut app.detail else {
                return;
            };
            let patch = CardPatch {
                title: Some(d.title.clone()),
                body: Some(d.body.text()),
                ..CardPatch::default()
            };
            let result = app.store.card_update(&d.key, &manager(), &patch);
            if result.is_ok() {
                d.dirty = false;
            }
            app.after_write(result);
        }
        Message::CommentEdited(comment) => {
            if let Some(d) = &mut app.detail {
                d.comment = comment;
            }
        }
        Message::AddComment => {
            let Some(d) = &mut app.detail else {
                return;
            };
            if d.comment.trim().is_empty() {
                return;
            }
            let result = app.store.comment(&d.key, &manager(), d.comment.trim());
            if result.is_ok() {
                d.comment.clear();
            }
            app.after_write(result);
        }
        Message::MoveTo(column) => {
            if let Some(d) = &app.detail {
                let result = app.store.card_move(&d.key, &manager(), column);
                app.after_write(result);
            }
        }
        Message::Step(forward) => {
            let Some(d) = &app.detail else {
                return;
            };
            let column = d.card.card.column;
            let target = if forward {
                column.next()
            } else {
                column.prev()
            };
            if let Some(target) = target {
                let result = app.store.card_move(&d.key, &manager(), target);
                app.after_write(result);
            }
        }
        Message::LinkClicked => {}
    }
}

fn manager() -> Actor {
    Actor {
        role: Role::Manager,
        name: "manager".to_string(),
    }
}

fn view(state: &State) -> Element<'_, Message> {
    match state {
        Ok(app) => app.view(),
        Err(e) => container(text(e).color(theme::DANGER)).padding(20).into(),
    }
}

fn subscription(state: &State) -> Subscription<Message> {
    match state {
        Ok(app) => {
            let tick = iced::time::every(Duration::from_millis(300)).map(|_| Message::Tick);
            if app.detail.is_none() {
                return tick;
            }
            let keys = keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, .. } => match key {
                    Key::Named(Named::ArrowLeft) => Some(Message::Step(false)),
                    Key::Named(Named::ArrowRight) => Some(Message::Step(true)),
                    Key::Named(Named::Escape) => Some(Message::CloseDetail),
                    _ => None,
                },
                _ => None,
            });
            Subscription::batch([tick, keys])
        }
        Err(_) => Subscription::none(),
    }
}

fn app_theme(state: &State) -> Theme {
    match state {
        Ok(app) => app.theme.clone(),
        Err(_) => theme::theme(),
    }
}

impl App {
    fn reload(&mut self, selected: Option<String>) {
        if let Err(e) = self.try_reload(selected) {
            self.error = Some(e.to_string());
        }
    }

    fn after_write<T>(&mut self, result: store::Result<T>) {
        match result {
            Ok(_) => self.reload(self.selected.clone()),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn try_reload(&mut self, selected: Option<String>) -> store::Result<()> {
        let version = self.store.data_version()?;
        let projects = self.store.projects()?;
        let selected = selected
            .filter(|key| projects.iter().any(|p| &p.key == key))
            .or_else(|| projects.first().map(|p| p.key.clone()));
        let (board, events, agents) = match &selected {
            Some(key) => (
                self.store.board(key, None)?,
                self.store.events(key, 100)?,
                self.store.agents(key)?,
            ),
            None => (Vec::new(), Vec::new(), Vec::new()),
        };
        let open = match &self.detail {
            Some(d) if selected.as_ref() == Some(&d.card.card.project_key) => {
                match self.store.card_show(&d.key) {
                    Ok(card) => Some(card),
                    Err(StoreError::NotFound(_)) => None,
                    Err(e) => return Err(e),
                }
            }
            _ => None,
        };
        self.projects = projects;
        self.selected = selected;
        self.board = board;
        self.events = events;
        self.agents = agents;
        match (open, &mut self.detail) {
            (Some(card), Some(d)) => d.refresh(card),
            _ => self.detail = None,
        }
        self.data_version = version;
        self.error = None;
        Ok(())
    }

    fn view(&self) -> Element<'_, Message> {
        let body =
            row![self.view_sidebar(), rule::vertical(1), self.view_board()].height(Length::Fill);
        column![body, rule::horizontal(1), self.view_footer()].into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let projects: Element<'_, Message> = if self.projects.is_empty() {
            text("no projects").color(theme::TEXT_MUTED).into()
        } else {
            column(self.projects.iter().map(|p| {
                let selected = self.selected.as_ref() == Some(&p.key);
                button(text(&p.name))
                    .width(Length::Fill)
                    .on_press(Message::SelectProject(p.key.clone()))
                    .style(theme::button(selected))
                    .into()
            }))
            .spacing(6)
            .into()
        };
        column![text("FUEL STATION"), scrollable(projects)]
            .spacing(20)
            .padding(20)
            .width(200)
            .into()
    }

    fn view_board(&self) -> Element<'_, Message> {
        let Some(project) = self
            .selected
            .as_ref()
            .and_then(|key| self.projects.iter().find(|p| &p.key == key))
        else {
            return space().into();
        };
        let columns = row(Column::ALL.iter().map(|&col| self.view_column(col)))
            .spacing(10)
            .height(Length::Fill);
        let header = row![
            text(format!("{} · {}", project.key, project.name)),
            space::horizontal(),
            self.view_agents(),
        ]
        .align_y(Alignment::Center);
        let mut main = row![column![columns, activity::view(&self.events)].spacing(10)]
            .spacing(10)
            .height(Length::Fill);
        if let Some(detail) = &self.detail {
            main = main.push(detail.view(&self.theme));
        }
        column![header, main]
            .spacing(20)
            .padding(20)
            .width(Length::Fill)
            .into()
    }

    fn view_agents(&self) -> Element<'_, Message> {
        let count = self.agents.len();
        let dot = if count > 0 {
            theme::ACCENT
        } else {
            theme::TEXT_MUTED
        };
        let mut strip = row![
            text("●").color(dot),
            text(format!("{count} agents")).color(theme::TEXT_MUTED),
        ]
        .spacing(6);
        for agent in &self.agents {
            strip = strip.push(
                text(format!(
                    "  {} {} {}",
                    role_glyph(agent.actor.role),
                    agent.actor.name,
                    agent.card_key.as_deref().unwrap_or("")
                ))
                .color(theme::ACCENT),
            );
        }
        strip.into()
    }

    fn view_column(&self, col: Column) -> Element<'_, Message> {
        let cards: Vec<&CardSummary> = self.board.iter().filter(|c| c.column == col).collect();
        let name = text(col.as_str().to_uppercase());
        let name = match col {
            Column::Review => name.color(theme::WARNING),
            Column::Done => name.color(theme::SUCCESS),
            _ => name,
        };
        let header = row![name, text(cards.len().to_string()).color(theme::TEXT_MUTED)].spacing(8);
        let open = self.detail.as_ref().map(|d| d.key.as_str());
        let list = scrollable(
            column(
                cards
                    .into_iter()
                    .map(|card| card_box(card, open == Some(card.key.as_str()))),
            )
            .spacing(6),
        );
        container(column![header, list].spacing(10))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::panel)
            .into()
    }

    fn view_footer(&self) -> Element<'_, Message> {
        let mut status = row![
            text(format!(
                "{} · data_version {}",
                store::db_path().display(),
                self.data_version
            ))
            .color(theme::TEXT_MUTED)
        ]
        .spacing(20)
        .align_y(Alignment::Center);
        if let Some(e) = &self.error {
            status = status.push(text(e).color(theme::DANGER));
        }
        container(status.push(space::horizontal()).push(pick_list(
            self.themes.as_slice(),
            Some(&self.theme),
            Message::ThemeChanged,
        )))
        .padding([10, 20])
        .into()
    }
}

fn card_box(card: &CardSummary, selected: bool) -> Element<'_, Message> {
    let mut top = row![text(&card.key).color(theme::TEXT_MUTED)];
    if let Some(tier) = &card.tier {
        top = top
            .push(space::horizontal())
            .push(text(format!("[{tier}]")));
    }
    let mut body = column![top, text(&card.title)].spacing(4);
    if let Some(holder) = &card.holder {
        body = body.push(
            text(format!("{} {}", role_glyph(holder.role), holder.name)).color(theme::ACCENT),
        );
    }
    mouse_area(
        container(body)
            .padding(8)
            .width(Length::Fill)
            .style(theme::card_box(card.human, selected)),
    )
    .on_press(Message::OpenCard(card.key.clone()))
    .interaction(mouse::Interaction::Pointer)
    .into()
}

fn local_time(secs: i64, format: &str) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|t| t.with_timezone(&chrono::Local).format(format).to_string())
        .unwrap_or_default()
}

fn role_glyph(role: Role) -> char {
    match role {
        Role::Truck => '\u{f0d1}',
        Role::Fuel => '\u{f0298}',
        Role::Mechanic => '\u{f0ad}',
        Role::Manager => '\u{f007}',
    }
}
