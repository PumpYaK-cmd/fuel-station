mod activity;
mod detail;
mod fx;
mod setup_sheet;
pub mod theme;
mod titlebar;

use std::collections::HashMap;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::keyboard::{self, Key, key::Named};
use iced::theme::palette::mix;
use iced::widget::{
    button, canvas, column, container, mouse_area, pick_list, progress_bar, row, rule, scrollable,
    space, stack, text, text_editor,
};
use iced::{Alignment, Element, Length, Size, Subscription, Task, Theme, mouse, window};

use crate::import::{self, ImportReport};
use crate::setup::agents;
use crate::store::{
    self, Actor, Agent, CardPatch, CardSummary, Column, Event, Project, Role, Store, StoreError,
};
use detail::{Detail, Leave};
use setup_sheet::Sheet;

pub fn run() {
    let result = iced::application(boot, update, view)
        .title("Fuel Station")
        .subscription(subscription)
        .theme(app_theme)
        .font(include_bytes!("../../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf").as_slice())
        .default_font(theme::FONT)
        .window(window::Settings {
            min_size: Some(Size::new(960.0, 600.0)),
            decorations: false,
            ..window::Settings::default()
        })
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
    read_failed: bool,
    theme: Theme,
    themes: Vec<Theme>,
    detail: Option<Detail>,
    events: Vec<Event>,
    agents: Vec<Agent>,
    ticks: u32,
    sheet: Option<Sheet>,
    can_import: bool,
    import: Option<Result<ImportReport, String>>,
    window: Option<window::Id>,
    booted: Instant,
    t: f32,
    hovered: Option<String>,
    appeared: HashMap<String, Instant>,
    reloaded: Instant,
    grid: canvas::Cache,
}

#[derive(Debug, Clone)]
enum Message {
    SelectProject(String),
    Tick,
    ThemeChanged(Theme),
    OpenCard(String),
    CloseDetail,
    Escape,
    TitleEdited(String),
    BodyAction(text_editor::Action),
    Save,
    SaveAndLeave,
    DiscardAndLeave,
    KeepEditing,
    CommentEdited(String),
    AddComment,
    MoveTo(Column),
    Step(bool),
    LinkClicked,
    AddProject,
    PickFolder(String),
    Picked(Option<String>, Option<PathBuf>),
    Repair(String),
    Sheet(setup_sheet::Message),
    Import,
    CloseImport,
    WindowId(Option<window::Id>),
    Frame(Instant),
    DragWindow,
    Minimize,
    ToggleMaximize,
    Close,
    HoverCard(String),
    UnhoverCard(String),
}

fn boot() -> (State, Task<Message>) {
    (open(), window::latest().map(Message::WindowId))
}

fn open() -> State {
    let store = Store::open_default()
        .map_err(|e| format!("cannot open {}: {e}", store::db_path().display()))?;
    let mut app = App {
        store,
        projects: Vec::new(),
        selected: None,
        board: Vec::new(),
        data_version: 0,
        error: None,
        read_failed: false,
        theme: theme::theme(),
        themes: std::iter::once(theme::theme())
            .chain(Theme::ALL.iter().cloned())
            .collect(),
        detail: None,
        events: Vec::new(),
        agents: Vec::new(),
        ticks: 0,
        sheet: None,
        can_import: import::trello_dir().is_dir(),
        import: None,
        window: None,
        booted: Instant::now(),
        t: 0.0,
        hovered: None,
        appeared: HashMap::new(),
        reloaded: Instant::now(),
        grid: canvas::Cache::new(),
    };
    app.reload(None);
    if let Err(e) = agents::install_agents(&agents::agents_dir()) {
        app.error = Some(format!("agent files: {e}"));
    }
    Ok(app)
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    let Ok(app) = state else {
        return Task::none();
    };
    match message {
        Message::SelectProject(key) => {
            if app.detail.as_ref().is_some_and(|d| d.dirty) && app.selected.as_ref() == Some(&key) {
                return Task::none();
            }
            request_leave(app, Leave::Select(key));
        }
        Message::Tick => {
            app.ticks = app.ticks.wrapping_add(1);
            match app.store.data_version() {
                Ok(version) if version != app.data_version || app.ticks % 17 == 0 => {
                    app.reload(app.selected.clone())
                }
                Ok(_) => app.clear_read_error(),
                Err(e) => app.read_error(e),
            }
        }
        Message::ThemeChanged(theme) => app.theme = theme,
        Message::OpenCard(key) => {
            if app.detail.as_ref().is_some_and(|d| d.key == key) {
                return Task::none();
            }
            request_leave(app, Leave::Open(key));
        }
        Message::CloseDetail => request_leave(app, Leave::Close),
        Message::Escape => match &mut app.detail {
            Some(d) if d.leaving.is_some() => d.leaving = None,
            _ => request_leave(app, Leave::Close),
        },
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
            save(app);
        }
        Message::SaveAndLeave => {
            if save(app)
                && let Some(leaving) = app.detail.as_mut().and_then(|d| d.leaving.take())
            {
                leave(app, leaving);
            }
        }
        Message::DiscardAndLeave => {
            if let Some(d) = &mut app.detail {
                d.dirty = false;
                if let Some(leaving) = d.leaving.take() {
                    leave(app, leaving);
                }
            }
        }
        Message::KeepEditing => {
            if let Some(d) = &mut app.detail {
                d.leaving = None;
            }
        }
        Message::CommentEdited(comment) => {
            if let Some(d) = &mut app.detail {
                d.comment = comment;
            }
        }
        Message::AddComment => {
            let Some(d) = &mut app.detail else {
                return Task::none();
            };
            if d.comment.trim().is_empty() {
                return Task::none();
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
                return Task::none();
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
        Message::AddProject => return pick_folder(None),
        Message::PickFolder(key) => return pick_folder(Some(key)),
        Message::Picked(key, Some(dir)) => {
            let taken: Vec<String> = match app.store.project_names() {
                Ok(names) => names.into_iter().map(|(key, _)| key).collect(),
                Err(e) => {
                    app.error = Some(e.to_string());
                    return Task::none();
                }
            };
            app.sheet = match key {
                None => Some(Sheet::add(&dir, &taken)),
                Some(key) => app
                    .projects
                    .iter()
                    .find(|p| p.key == key)
                    .map(|p| Sheet::repair_at(p, &dir, &taken)),
            };
        }
        Message::Picked(_, None) => {}
        Message::Repair(key) => {
            app.sheet = app
                .projects
                .iter()
                .find(|p| p.key == key)
                .map(Sheet::repair);
        }
        Message::Sheet(setup_sheet::Message::Close) => app.sheet = None,
        Message::Sheet(message) => {
            if let Some(key) = app
                .sheet
                .as_mut()
                .and_then(|sheet| sheet.update(message, &mut app.store))
            {
                app.reload(Some(key));
            }
        }
        Message::Import => {
            app.import = Some(import::import_all(&mut app.store, &import::trello_dir()));
            app.reload(None);
        }
        Message::CloseImport => app.import = None,
        Message::WindowId(id) => app.window = id,
        Message::Frame(now) => app.t = now.duration_since(app.booted).as_secs_f32(),
        Message::DragWindow => return app.window.map_or_else(Task::none, window::drag),
        Message::Minimize => {
            return app
                .window
                .map_or_else(Task::none, |id| window::minimize(id, true));
        }
        Message::ToggleMaximize => {
            return app.window.map_or_else(Task::none, window::toggle_maximize);
        }
        Message::Close => return app.window.map_or_else(Task::none, window::close),
        Message::HoverCard(key) => app.hovered = Some(key),
        Message::UnhoverCard(key) => {
            if app.hovered.as_ref() == Some(&key) {
                app.hovered = None;
            }
        }
    }
    Task::none()
}

fn save(app: &mut App) -> bool {
    let Some(d) = &mut app.detail else {
        return true;
    };
    if d.title.trim().is_empty() {
        return false;
    }
    let body = d.body.text();
    let patch = CardPatch {
        title: (d.title != d.card.card.title).then(|| d.title.clone()),
        body: (body != d.card.card.body).then_some(body),
        ..CardPatch::default()
    };
    if patch.title.is_none() && patch.body.is_none() {
        d.dirty = false;
        return true;
    }
    let result = app.store.card_update(&d.key, &manager(), &patch);
    let saved = result.is_ok();
    if saved {
        d.dirty = false;
    }
    app.after_write(result);
    saved
}

fn request_leave(app: &mut App, target: Leave) {
    match &mut app.detail {
        Some(d) if d.dirty => d.leaving = Some(target),
        _ => leave(app, target),
    }
}

fn leave(app: &mut App, target: Leave) {
    match target {
        Leave::Close => app.detail = None,
        Leave::Open(key) => match app.store.card_show(&key) {
            Ok(card) => app.detail = Some(Detail::new(card)),
            Err(e) => app.error = Some(e.to_string()),
        },
        Leave::Select(key) => {
            app.sheet = None;
            app.import = None;
            app.reload(Some(key));
        }
    }
}

fn pick_folder(key: Option<String>) -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .pick_folder()
                .await
                .map(|dir| dir.path().to_path_buf())
        },
        move |dir| Message::Picked(key.clone(), dir),
    )
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
            let frames = window::frames().map(Message::Frame);
            if app.detail.is_none() {
                return Subscription::batch([tick, frames]);
            }
            let keys = keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed {
                    key, repeat: false, ..
                } => match key {
                    Key::Named(Named::ArrowLeft) => Some(Message::Step(false)),
                    Key::Named(Named::ArrowRight) => Some(Message::Step(true)),
                    Key::Named(Named::Escape) => Some(Message::Escape),
                    _ => None,
                },
                _ => None,
            });
            Subscription::batch([tick, frames, keys])
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
            self.read_error(e);
        }
    }

    fn read_error(&mut self, e: StoreError) {
        self.error = Some(e.to_string());
        self.read_failed = true;
    }

    fn clear_read_error(&mut self) {
        if self.read_failed {
            self.error = None;
            self.read_failed = false;
        }
    }

    fn after_write<T>(&mut self, result: store::Result<T>) {
        match result {
            Ok(_) => {
                self.error = None;
                self.reload(self.selected.clone());
            }
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
        let now = Instant::now();
        if selected != self.selected {
            self.appeared.clear();
            self.reloaded = now;
        } else {
            for card in &board {
                if !self.board.iter().any(|old| old.key == card.key) {
                    self.appeared.insert(card.key.clone(), now);
                }
            }
        }
        self.appeared
            .retain(|_, at| now.duration_since(*at) < Duration::from_secs(1));
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
        self.clear_read_error();
        Ok(())
    }

    fn view(&self) -> Element<'_, Message> {
        let backdrop = canvas(fx::Backdrop {
            t: self.t,
            grid: &self.grid,
        })
        .width(Length::Fill)
        .height(Length::Fill);
        let body =
            row![self.view_sidebar(), rule::vertical(1), self.view_main()].height(Length::Fill);
        stack![
            backdrop,
            column![
                titlebar::view(self),
                body,
                rule::horizontal(1),
                self.view_footer()
            ]
        ]
        .into()
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
        let add = button(text("+ Add Project"))
            .width(Length::Fill)
            .on_press(Message::AddProject)
            .style(theme::button(false));
        container(
            column![
                scrollable(projects)
                    .direction(theme::thin())
                    .style(theme::scrollbar)
                    .height(Length::Fill),
                add
            ]
            .spacing(20),
        )
        .padding([20, 16])
        .width(200)
        .height(Length::Fill)
        .style(theme::sidebar)
        .into()
    }

    fn view_main(&self) -> Element<'_, Message> {
        if let Some(sheet) = &self.sheet {
            return sheet.view().map(Message::Sheet);
        }
        if let Some(import) = &self.import {
            return view_import(import);
        }
        if self.projects.is_empty() && self.can_import {
            return container(
                button(text("Import trello boards"))
                    .on_press(Message::Import)
                    .style(theme::button(false)),
            )
            .padding(20)
            .into();
        }
        self.view_board()
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
        let action = match project.path {
            Some(_) => button(text("Repair setup")).on_press(Message::Repair(project.key.clone())),
            None => button(text("Pick folder")).on_press(Message::PickFolder(project.key.clone())),
        };
        let header = row![
            text(format!("{} · {}", project.key, project.name)),
            space::horizontal(),
            action.style(theme::button(false)),
        ]
        .spacing(20)
        .align_y(Alignment::Center);
        let mut main = row![column![columns, activity::view(&self.events, self.t)].spacing(10)]
            .spacing(10)
            .height(Length::Fill);
        if let Some(detail) = &self.detail {
            main = main.push(detail.view(&self.theme));
        }
        column![header, main]
            .spacing(14)
            .padding([16, 20])
            .width(Length::Fill)
            .into()
    }

    fn view_column(&self, col: Column) -> Element<'_, Message> {
        let cards: Vec<&CardSummary> = self.board.iter().filter(|c| c.column == col).collect();
        let (name_color, gauge_color) = match col {
            Column::Review => (theme::WARNING, theme::WARNING),
            Column::Done => (theme::SUCCESS, theme::SUCCESS),
            _ => (theme::TEXT, theme::PRIMARY),
        };
        let grow = 1.0 - (1.0 - self.reloaded.elapsed().as_secs_f32().min(1.0)).powi(3);
        let fill = (cards.len() as f32).min(theme::COLUMN_CAP) / theme::COLUMN_CAP * grow;
        let header = row![
            text(col.as_str().to_uppercase()).size(11).color(name_color),
            text(cards.len().to_string())
                .size(11)
                .color(theme::TEXT_MUTED),
            progress_bar(0.0..=1.0, fill)
                .girth(3)
                .style(theme::gauge(gauge_color)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let open = self.detail.as_ref().map(|d| d.key.as_str());
        let list = scrollable(
            column(cards.into_iter().map(|card| {
                let flash = self
                    .appeared
                    .get(&card.key)
                    .map(|at| at.elapsed().as_secs_f32() / FLASH_SECS)
                    .filter(|age| *age < 1.0);
                card_box(
                    card,
                    open == Some(card.key.as_str()),
                    self.hovered.as_deref() == Some(card.key.as_str()),
                    flash,
                )
            }))
            .spacing(6),
        )
        .direction(theme::thin())
        .style(theme::scrollbar);
        let panel = container(column![header, rule::horizontal(1), list].spacing(10))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::glass);
        let brackets = match col {
            Column::Doing => fx::Brackets {
                color: theme::PRIMARY,
                alpha: 0.7 + 0.3 * (self.t * PI).sin(),
            },
            _ => fx::Brackets {
                color: theme::BORDER_STRONG,
                alpha: 1.0,
            },
        };
        stack![
            panel,
            canvas(brackets).width(Length::Fill).height(Length::Fill)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_footer(&self) -> Element<'_, Message> {
        let mut status = row![
            text(format!(
                "{} · data_version {}",
                store::db_path().display(),
                self.data_version
            ))
            .size(11)
            .color(theme::TEXT_MUTED)
        ]
        .spacing(20)
        .align_y(Alignment::Center);
        if let Some(e) = &self.error {
            status = status.push(text(e).size(11).color(theme::DANGER));
        }
        container(
            status.push(space::horizontal()).push(
                pick_list(
                    self.themes.as_slice(),
                    Some(&self.theme),
                    Message::ThemeChanged,
                )
                .text_size(11)
                .style(theme::picker),
            ),
        )
        .padding([8, 20])
        .style(theme::bar)
        .into()
    }
}

fn view_import(import: &Result<ImportReport, String>) -> Element<'_, Message> {
    let lines: Element<'_, Message> = match import {
        Ok(report) => column(
            report
                .imported
                .iter()
                .map(|(key, name, cards)| {
                    text(format!("imported {name} as {key}: {cards} cards"))
                        .color(theme::SUCCESS)
                        .into()
                })
                .chain(report.skipped.iter().map(|(name, reason)| {
                    text(format!("skipped {name}: {reason}"))
                        .color(theme::WARNING)
                        .into()
                })),
        )
        .spacing(6)
        .into(),
        Err(e) => text(e).color(theme::DANGER).into(),
    };
    let done = button(text("Done"))
        .on_press(Message::CloseImport)
        .style(theme::button(false));
    column![text("Import trello boards"), scrollable(lines), done]
        .spacing(20)
        .padding(20)
        .width(Length::Fill)
        .into()
}

const FLASH_SECS: f32 = 0.6;

fn card_box(
    card: &CardSummary,
    selected: bool,
    hovered: bool,
    flash: Option<f32>,
) -> Element<'_, Message> {
    let mut top = row![text(&card.key).color(theme::TEXT_MUTED)];
    if let Some(tier) = &card.tier {
        top = top
            .push(space::horizontal())
            .push(text(format!("[{tier}]")));
    }
    let mut body = column![top, text(&card.title)].spacing(4);
    if let Some(holder) = &card.holder {
        body = body.push(
            text(format!("{} {}", role_glyph(holder.role), holder.name))
                .size(11)
                .color(theme::ACCENT),
        );
    }
    let bar = if selected {
        theme::ACCENT
    } else if card.human {
        theme::WARNING
    } else if let Some(age) = flash {
        mix(theme::ACCENT, theme::BORDER_STRONG, age)
    } else if hovered {
        theme::PRIMARY
    } else {
        theme::BORDER_STRONG
    };
    let border = if selected {
        theme::ACCENT
    } else if card.human {
        theme::WARNING
    } else {
        theme::BORDER
    };
    let content = row![
        container(space())
            .width(2)
            .height(Length::Fill)
            .style(theme::fill(bar)),
        container(body).padding(8).width(Length::Fill),
    ]
    .height(Length::Shrink);
    mouse_area(
        container(content)
            .width(Length::Fill)
            .style(theme::card(border)),
    )
    .on_press(Message::OpenCard(card.key.clone()))
    .on_enter(Message::HoverCard(card.key.clone()))
    .on_exit(Message::UnhoverCard(card.key.clone()))
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
