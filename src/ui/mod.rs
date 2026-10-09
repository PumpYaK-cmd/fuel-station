pub mod theme;

use std::time::Duration;

use iced::widget::{button, column, container, pick_list, row, rule, scrollable, space, text};
use iced::{Alignment, Element, Length, Subscription, Theme};

use crate::store::{self, CardSummary, Column, Project, Role, Store};

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
}

#[derive(Debug, Clone)]
enum Message {
    SelectProject(String),
    Tick,
    ThemeChanged(Theme),
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
    };
    app.reload(None);
    Ok(app)
}

fn update(state: &mut State, message: Message) {
    let Ok(app) = state else {
        return;
    };
    match message {
        Message::SelectProject(key) => app.reload(Some(key)),
        Message::Tick => match app.store.data_version() {
            Ok(version) if version != app.data_version => app.reload(app.selected.clone()),
            Ok(_) => app.error = None,
            Err(e) => app.error = Some(e.to_string()),
        },
        Message::ThemeChanged(theme) => app.theme = theme,
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
        Ok(_) => iced::time::every(Duration::from_millis(300)).map(|_| Message::Tick),
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

    fn try_reload(&mut self, selected: Option<String>) -> store::Result<()> {
        let version = self.store.data_version()?;
        let projects = self.store.projects()?;
        let selected = selected
            .filter(|key| projects.iter().any(|p| &p.key == key))
            .or_else(|| projects.first().map(|p| p.key.clone()));
        let board = match &selected {
            Some(key) => self.store.board(key, None)?,
            None => Vec::new(),
        };
        self.projects = projects;
        self.selected = selected;
        self.board = board;
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
        column![text(format!("{} · {}", project.key, project.name)), columns]
            .spacing(20)
            .padding(20)
            .width(Length::Fill)
            .into()
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
        let list = scrollable(column(cards.into_iter().map(card_box)).spacing(6));
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

fn card_box(card: &CardSummary) -> Element<'_, Message> {
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
    container(body)
        .padding(8)
        .width(Length::Fill)
        .style(theme::card_box(card.human))
        .into()
}

fn role_glyph(role: Role) -> char {
    match role {
        Role::Truck => '\u{f0d1}',
        Role::Fuel => '\u{f0298}',
        Role::Mechanic => '\u{f0ad}',
        Role::Manager => '\u{f007}',
    }
}
