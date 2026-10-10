mod activity;
mod detail;
mod fx;
mod notify;
mod runs;
mod setup_sheet;
pub mod theme;
mod titlebar;

use std::collections::{BTreeSet, HashMap};
use std::f32::consts::PI;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use iced::keyboard::{self, Key, key::Named};
use iced::theme::palette::mix;
use iced::widget::{
    button, canvas, column, container, float, mouse_area, operation, pick_list, pin, progress_bar,
    row, rule, scrollable, space, stack, text, text_editor, text_input,
};
use iced::{
    Alignment, Color, Element, Length, Padding, Point, Size, Subscription, Task, Theme, Vector,
    mouse, window,
};

use crate::import::{self, ImportReport};
use crate::setup::detect::Facts;
use crate::setup::{agents, skills};
use crate::store::{
    self, Actor, Agent, CardPatch, CardSummary, Column, Event, NewProject, Project, Role, Store,
    StoreError,
};
use detail::{Detail, Leave};
use notify::TrayAction;
use runs::{Run, RunEvent};
use setup_sheet::Sheet;

const APP_ICON: &[u8] = include_bytes!("../../assets/icon/fuel-station-64.rgba");
const APP_ICON_SIZE: u32 = 64;

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
            icon: window::icon::from_rgba(APP_ICON.to_vec(), APP_ICON_SIZE, APP_ICON_SIZE).ok(),
            ..window::Settings::default()
        })
        .run();
    if let Err(e) = result {
        report_fatal(&e.to_string());
    }
}

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| report_fatal(&info.to_string())));
}

fn crash_log() -> PathBuf {
    crate::setup::install::app_dir()
        .unwrap_or_default()
        .join("crash.log")
}

fn report_fatal(info: &str) {
    let path = crash_log();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{secs} {}", info.replace('\n', " "));
    }
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Fuel Station")
        .set_description(format!("{info}\nlogged to {}", path.display()))
        .show();
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
    detecting: Option<u64>,
    picking: Option<u64>,
    next_detect: u64,
    can_import: bool,
    import: Option<Result<ImportReport, String>>,
    window: Option<window::Id>,
    booted: Instant,
    t: f32,
    hovered: Option<String>,
    appeared: HashMap<String, Instant>,
    reloaded: Instant,
    grid: canvas::Cache,
    tray: Option<notify::Tray>,
    focused: bool,
    last_event: i64,
    search: String,
    filter: Filter,
    drag: Option<Drag>,
    runs: HashMap<String, Run>,
    next_run: u64,
    landed: Option<(String, Instant)>,
}

type Slot = (Column, Option<String>);

struct Drag {
    key: String,
    over: Option<Column>,
    over_at: Instant,
    moved: bool,
    cursor: Point,
    lifted: Option<Instant>,
    slot: Option<Slot>,
    slot_at: Instant,
    closing: Vec<(Slot, Instant, f32)>,
}

impl Drag {
    fn lift(&mut self) {
        if !self.moved {
            let now = Instant::now();
            self.moved = true;
            self.lifted = Some(now);
            self.over_at = now;
        }
    }

    fn set_over(&mut self, column: Column) {
        if self.over != Some(column) {
            self.over = Some(column);
            self.over_at = Instant::now();
        }
    }

    fn set_slot(&mut self, slot: Option<Slot>) {
        if self.slot == slot {
            return;
        }
        let now = Instant::now();
        let open = ease_out(self.slot_at.elapsed().as_secs_f32() / GAP_SECS);
        let closing = self
            .closing
            .iter()
            .filter(|(old, _, _)| Some(old) == slot.as_ref())
            .map(|(_, at, from)| from * (1.0 - ease_out(at.elapsed().as_secs_f32() / GAP_SECS)))
            .sum::<f32>();
        self.closing.retain(|(old, at, _)| {
            Some(old) != slot.as_ref() && at.elapsed().as_secs_f32() < GAP_SECS
        });
        if let Some(old) = self.slot.take() {
            self.closing.push((old, now, open));
        }
        self.slot = slot;
        let head_start = GAP_SECS * (1.0 - (1.0 - closing).cbrt());
        self.slot_at = now
            .checked_sub(Duration::from_secs_f32(head_start))
            .unwrap_or(now);
    }

    fn gap(&self, column: Column, before: Option<&str>) -> f32 {
        let is = |slot: &Slot| slot.0 == column && slot.1.as_deref() == before;
        let opening = match &self.slot {
            Some(slot) if is(slot) => ease_out(self.slot_at.elapsed().as_secs_f32() / GAP_SECS),
            _ => 0.0,
        };
        let closing = self
            .closing
            .iter()
            .filter(|(slot, _, _)| is(slot))
            .map(|(_, at, from)| from * (1.0 - ease_out(at.elapsed().as_secs_f32() / GAP_SECS)))
            .sum::<f32>();
        DROP_GAP * (opening + closing)
    }
}

const SEARCH_ID: &str = "search";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Filter {
    All,
    Human,
    Held,
    Tier(String),
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Filter::All => f.write_str("all"),
            Filter::Human => f.write_str("human"),
            Filter::Held => f.write_str("held"),
            Filter::Tier(tier) => write!(f, "tier {tier}"),
        }
    }
}

fn card_matches(card: &CardSummary, search: &str, filter: &Filter) -> bool {
    let search = search.trim().to_lowercase();
    let found = search.is_empty()
        || card.key.to_lowercase().contains(&search)
        || card.title.to_lowercase().contains(&search);
    found
        && match filter {
            Filter::All => true,
            Filter::Human => card.human,
            Filter::Held => card.holder.is_some(),
            Filter::Tier(tier) => card.tier.as_ref() == Some(tier),
        }
}

#[derive(Debug, Clone)]
enum Message {
    SelectProject(String),
    Tick,
    ThemeChanged(Theme),
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
    ToggleHuman,
    LinkClicked,
    AddProject,
    PickFolder(String),
    Picked(u64, Option<String>, Option<PathBuf>),
    Repair(String),
    Detected(u64, Option<String>, Box<NewProject>, Facts),
    Sheet(setup_sheet::Message),
    Import,
    CloseImport,
    WindowId(Option<window::Id>),
    Frame(Instant),
    DragWindow,
    Minimize,
    ToggleMaximize,
    Close,
    Press(String),
    EnterCard(String),
    ExitCard(String),
    EnterColumn(Column),
    EnterTail(Column),
    DropOn(Column),
    DragMove(Point),
    CancelDrag,
    Focus(bool),
    Search(String),
    FilterChanged(Filter),
    FocusSearch,
    Launch,
    StopRun,
    RunEvent(String, u64, RunEvent),
}

fn boot() -> (State, Task<Message>) {
    let mut state = open();
    if let Ok(app) = &mut state {
        #[cfg(windows)]
        if let Err(e) = notify::register_app_id() {
            app.error.get_or_insert_with(|| format!("toast id: {e}"));
        }
        match notify::tray() {
            Ok(tray) => app.tray = Some(tray),
            Err(e) => {
                app.error.get_or_insert_with(|| format!("tray icon: {e}"));
            }
        }
    }
    (state, window::latest().map(Message::WindowId))
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
        detecting: None,
        picking: None,
        next_detect: 0,
        can_import: import::trello_dir().is_dir(),
        import: None,
        window: None,
        booted: Instant::now(),
        t: 0.0,
        hovered: None,
        appeared: HashMap::new(),
        reloaded: Instant::now(),
        grid: canvas::Cache::new(),
        tray: None,
        focused: true,
        last_event: 0,
        search: String::new(),
        filter: Filter::All,
        drag: None,
        runs: HashMap::new(),
        next_run: 0,
        landed: None,
    };
    app.reload(None);
    match app.store.last_event_id() {
        Ok(id) => app.last_event = id,
        Err(e) => app.read_error(e),
    }
    let installed = [
        ("agent files", agents::install_agents(&agents::agents_dir())),
        ("skills", skills::install_skills(&skills::skills_dir())),
    ];
    let problems: Vec<String> = installed
        .into_iter()
        .flat_map(|(label, result)| match result {
            Err(e) => vec![format!("{label}: {e}")],
            Ok(actions) => actions
                .into_iter()
                .filter(|(_, action)| action.starts_with("skipped"))
                .map(|(file, _)| format!("{file} not replaced: it has no station marker"))
                .collect(),
        })
        .collect();
    if !problems.is_empty() {
        app.error = Some(problems.join("; "));
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
            app.sheet = None;
            app.import = None;
            app.cancel_pending();
            return request_leave(app, Leave::Select(key));
        }
        Message::Tick => {
            app.ticks = app.ticks.wrapping_add(1);
            match app.store.data_version() {
                Ok(version) if version != app.data_version || app.ticks % 17 == 0 => {
                    app.reload(app.selected.clone());
                    app.notify_events();
                }
                Ok(_) => app.clear_read_error(),
                Err(e) => app.read_error(e),
            }
            return app.tray_task();
        }
        Message::ThemeChanged(theme) => app.theme = theme,
        Message::CloseDetail => return request_leave(app, Leave::Close),
        Message::Escape => match &mut app.detail {
            Some(d) if d.leaving.is_some() => d.leaving = None,
            _ => return request_leave(app, Leave::Close),
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
            if save(app)
                && let Some(d) = &mut app.detail
            {
                d.leaving = None;
            }
        }
        Message::SaveAndLeave => {
            if save(app)
                && let Some(leaving) = app.detail.as_mut().and_then(|d| d.leaving.take())
            {
                return leave(app, leaving);
            }
        }
        Message::DiscardAndLeave => {
            if let Some(d) = &mut app.detail {
                d.dirty = false;
                if let Some(leaving) = d.leaving.take() {
                    return leave(app, leaving);
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
        Message::ToggleHuman => {
            if let Some(d) = &app.detail {
                let patch = CardPatch {
                    human: Some(!d.card.card.human),
                    ..CardPatch::default()
                };
                let result = app.store.card_update(&d.key, &manager(), &patch);
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
        Message::AddProject => return start_pick(app, None),
        Message::PickFolder(key) => return start_pick(app, Some(key)),
        Message::Picked(id, key, Some(dir)) => {
            if app.picking != Some(id) {
                return Task::none();
            }
            app.picking = None;
            let taken: Vec<String> = match app.store.project_names() {
                Ok(names) => names.into_iter().map(|(key, _)| key).collect(),
                Err(e) => {
                    app.error = Some(e.to_string());
                    return Task::none();
                }
            };
            return start_detect(app, key, dir, taken);
        }
        Message::Picked(id, _, None) => {
            if app.picking == Some(id) {
                app.picking = None;
            }
        }
        Message::Repair(key) => {
            let Some(project) = app.projects.iter().find(|p| p.key == key) else {
                return Task::none();
            };
            match project.path.as_deref().filter(|p| !p.is_empty()) {
                Some(path) => {
                    let taken: Vec<String> = app.projects.iter().map(|p| p.key.clone()).collect();
                    let path = PathBuf::from(path);
                    return start_detect(app, Some(key), path, taken);
                }
                None => {
                    app.detecting = None;
                    app.picking = None;
                    app.sheet = Some(Sheet::repair(project, None));
                }
            }
        }
        Message::Detected(id, key, form, facts) => {
            if app.detecting != Some(id) {
                return Task::none();
            }
            app.detecting = None;
            app.sheet = match key {
                None => Some(Sheet::add(*form, facts)),
                Some(key) => app
                    .projects
                    .iter()
                    .find(|p| p.key == key)
                    .map(|p| Sheet::repair(p, Some((*form, facts)))),
            };
        }
        Message::Sheet(setup_sheet::Message::Close) => {
            app.sheet = None;
            app.cancel_pending();
        }
        Message::Sheet(message) => {
            let Some(sheet) = app.sheet.as_mut() else {
                return Task::none();
            };
            let (key, task) = sheet.update(message, &mut app.store);
            let task = task.map(Message::Sheet);
            match key {
                Some(key) if app.selected.as_ref() != Some(&key) => {
                    app.reload(app.selected.clone());
                    return Task::batch([request_leave(app, Leave::Select(key)), task]);
                }
                Some(key) => app.reload(Some(key)),
                None => {}
            }
            return task;
        }
        Message::Import => {
            app.cancel_pending();
            app.import = Some(import::import_all(&mut app.store, &import::trello_dir()));
            app.reload(None);
        }
        Message::CloseImport => app.import = None,
        Message::WindowId(id) => app.window = id,
        Message::Frame(now) => {
            app.t = now.duration_since(app.booted).as_secs_f32();
            if app
                .landed
                .as_ref()
                .is_some_and(|(_, at)| now.duration_since(*at).as_secs_f32() >= LAND_SECS)
            {
                app.landed = None;
            }
        }
        Message::DragWindow => return app.window.map_or_else(Task::none, window::drag),
        Message::Minimize => {
            return app
                .window
                .map_or_else(Task::none, |id| window::minimize(id, true));
        }
        Message::ToggleMaximize => {
            return app.window.map_or_else(Task::none, window::toggle_maximize);
        }
        Message::Close => return request_leave(app, Leave::Quit),
        Message::Press(key) => {
            if let Some(card) = app.board.iter().find(|c| c.key == key) {
                let now = Instant::now();
                app.drag = Some(Drag {
                    key,
                    over: Some(card.column),
                    over_at: now,
                    moved: false,
                    cursor: Point::ORIGIN,
                    lifted: None,
                    slot: None,
                    slot_at: now,
                    closing: Vec::new(),
                });
            }
        }
        Message::EnterCard(key) => {
            let column = app.board.iter().find(|c| c.key == key).map(|c| c.column);
            if let (Some(drag), Some(column)) = (&mut app.drag, column) {
                if key != drag.key {
                    drag.lift();
                }
                drag.set_over(column);
                drag.set_slot(Some((column, Some(key.clone()))));
            }
            app.hovered = Some(key);
        }
        Message::ExitCard(key) => {
            if app.hovered.as_ref() == Some(&key) {
                app.hovered = None;
            }
        }
        Message::EnterColumn(col) => {
            if let Some(drag) = &mut app.drag
                && drag.over != Some(col)
            {
                drag.set_over(col);
                drag.lift();
                if drag.slot.as_ref().map(|slot| slot.0) != Some(col) {
                    drag.set_slot(Some((col, None)));
                }
            }
        }
        Message::EnterTail(col) => {
            if let Some(drag) = &mut app.drag {
                drag.set_slot(Some((col, None)));
            }
        }
        Message::DropOn(col) => {
            let Some(drag) = app.drag.take() else {
                return Task::none();
            };
            if !drag.moved {
                if app.detail.as_ref().is_some_and(|d| d.key == drag.key) {
                    return Task::none();
                }
                return request_leave(app, Leave::Open(drag.key));
            }
            let before = match drag.slot {
                Some((slot_col, before)) if slot_col == col => before,
                _ => None,
            };
            let cards: Vec<&CardSummary> = app.board.iter().filter(|c| c.column == col).collect();
            let index = drop_index(&cards, &drag.key, before.as_deref());
            if cards.iter().position(|c| c.key == drag.key) == Some(index) {
                return Task::none();
            }
            let result = app
                .store
                .card_place(&drag.key, &manager(), col, index as i64);
            let placed = result.is_ok();
            app.after_write(result);
            if placed {
                let now = Instant::now();
                app.appeared.insert(drag.key.clone(), now);
                app.landed = Some((drag.key, now));
            }
        }
        Message::DragMove(position) => {
            if let Some(drag) = &mut app.drag {
                drag.cursor = position;
            }
        }
        Message::CancelDrag => app.drag = None,
        Message::Focus(focused) => app.focused = focused,
        Message::Search(search) => app.search = search,
        Message::FilterChanged(filter) => app.filter = filter,
        Message::FocusSearch => return operation::focus(SEARCH_ID),
        Message::Launch => {
            let Some(d) = &app.detail else {
                return Task::none();
            };
            let Ok(dir) = app.launch_dir() else {
                return Task::none();
            };
            let key = d.key.clone();
            let result = app.store.report(&key, &manager(), "launched Truck");
            app.after_write(result);
            let id = app.next_run;
            app.next_run += 1;
            let (task, run) = runs::start(dir, key.clone(), id);
            app.runs.insert(key, run);
            return task;
        }
        Message::StopRun => {
            let Some(d) = &app.detail else {
                return Task::none();
            };
            let key = d.key.clone();
            if let Some(run) = app.runs.get_mut(&key)
                && run.is_running()
            {
                run.stop();
                let result = app.store.report(&key, &manager(), "Truck run stopped");
                app.after_write(result);
            }
        }
        Message::RunEvent(key, id, event) => {
            if let Some(report) = app
                .runs
                .get_mut(&key)
                .filter(|run| run.id == id)
                .and_then(|run| run.apply(event))
            {
                let result = app.store.report(&key, &manager(), &report);
                app.after_write(result);
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

fn request_leave(app: &mut App, target: Leave) -> Task<Message> {
    match &mut app.detail {
        Some(d) if d.dirty => {
            if !matches!(d.leaving, Some(Leave::Quit)) {
                d.leaving = Some(target);
            }
            Task::none()
        }
        _ => leave(app, target),
    }
}

fn leave(app: &mut App, target: Leave) -> Task<Message> {
    match target {
        Leave::Close => app.detail = None,
        Leave::Open(key) => match app.store.card_show(&key) {
            Ok(card) => app.detail = Some(Detail::new(card)),
            Err(e) => app.error = Some(e.to_string()),
        },
        Leave::Select(key) => app.reload(Some(key)),
        Leave::Quit => return app.window.map_or_else(Task::none, window::close),
    }
    Task::none()
}

fn start_detect(
    app: &mut App,
    key: Option<String>,
    dir: PathBuf,
    taken: Vec<String>,
) -> Task<Message> {
    let id = app.next_detect;
    app.next_detect += 1;
    app.detecting = Some(id);
    app.picking = None;
    setup_sheet::detect(dir, taken)
        .map(move |(form, facts)| Message::Detected(id, key.clone(), Box::new(form), facts))
}

fn start_pick(app: &mut App, key: Option<String>) -> Task<Message> {
    let id = app.next_detect;
    app.next_detect += 1;
    app.cancel_pending();
    app.picking = Some(id);
    pick_folder(id, key)
}

fn pick_folder(id: u64, key: Option<String>) -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .pick_folder()
                .await
                .map(|dir| dir.path().to_path_buf())
        },
        move |dir| Message::Picked(id, key.clone(), dir),
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
            let focus = window::events().filter_map(|(_, event)| match event {
                window::Event::Focused => Some(Message::Focus(true)),
                window::Event::Unfocused => Some(Message::Focus(false)),
                _ => None,
            });
            let find = keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    repeat: false,
                    ..
                } if modifiers.command() && key.to_latin(physical_key) == Some('f') => {
                    Some(Message::FocusSearch)
                }
                _ => None,
            });
            let mut subs = vec![tick, frames, focus, find];
            if app.detail.is_some() {
                subs.push(keyboard::listen().filter_map(|event| match event {
                    keyboard::Event::KeyPressed {
                        key, repeat: false, ..
                    } => match key {
                        Key::Named(Named::ArrowLeft) => Some(Message::Step(false)),
                        Key::Named(Named::ArrowRight) => Some(Message::Step(true)),
                        Key::Named(Named::Escape) => Some(Message::Escape),
                        _ => None,
                    },
                    _ => None,
                }));
            } else if app.sheet.is_none() && app.import.is_none() {
                subs.push(iced::event::listen_with(|event, _, _| match event {
                    iced::Event::Keyboard(keyboard::Event::KeyPressed {
                        key: Key::Named(Named::Escape),
                        repeat: false,
                        ..
                    }) => Some(Message::Search(String::new())),
                    _ => None,
                }));
            }
            if app.drag.is_some() {
                // Column releases are not captured, so this also fires after DropOn; DropOn wins because widget messages run before subscription output.
                subs.push(iced::event::listen_with(|event, status, _| {
                    match (event, status) {
                        (
                            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                            iced::event::Status::Ignored,
                        ) => Some(Message::CancelDrag),
                        _ => None,
                    }
                }));
                subs.push(iced::event::listen_with(|event, _, _| match event {
                    iced::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                        Some(Message::DragMove(position))
                    }
                    _ => None,
                }));
            }
            Subscription::batch(subs)
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

    fn notify_events(&mut self) {
        let events = match self.store.events_after(self.last_event) {
            Ok(events) => events,
            Err(e) => return self.read_error(e),
        };
        let Some(last) = events.last() else {
            return;
        };
        self.last_event = last.id;
        if self.focused {
            return;
        }
        for e in &events {
            if let Some(body) = notify::toast_text(e) {
                let title = self
                    .projects
                    .iter()
                    .find(|p| p.key == e.project_key)
                    .map_or(e.project_key.as_str(), |p| p.name.as_str());
                notify::toast(title, &body);
            }
        }
    }

    fn tray_task(&mut self) -> Task<Message> {
        let (Some(tray), Some(id)) = (&self.tray, self.window) else {
            return Task::none();
        };
        let show = || window::minimize(id, false).chain(window::gain_focus(id));
        let tasks: Vec<Task<Message>> = notify::tray_messages(tray)
            .into_iter()
            .map(|action| match action {
                TrayAction::Show => show(),
                TrayAction::Quit => {
                    let quit = request_leave(self, Leave::Quit);
                    if self.detail.as_ref().is_some_and(|d| d.leaving.is_some()) {
                        show()
                    } else {
                        quit
                    }
                }
            })
            .collect();
        Task::batch(tasks)
    }

    fn launch_dir(&self) -> Result<PathBuf, String> {
        let card = &self
            .detail
            .as_ref()
            .ok_or_else(|| "no card open".to_string())?
            .card
            .card;
        if card.human {
            return Err("human card".to_string());
        }
        if card.column == Column::Done {
            return Err("card is done".to_string());
        }
        if self.runs.get(&card.key).is_some_and(Run::is_running) {
            return Err("already running".to_string());
        }
        if let Some(holder) = self
            .board
            .iter()
            .find(|c| c.key == card.key)
            .and_then(|c| c.holder.as_ref())
        {
            return Err(format!(
                "held by {} {}",
                role_glyph(holder.role),
                holder.name
            ));
        }
        if let Some(other) = self.runs.iter().find_map(|(key, run)| {
            (run.is_running()
                && key.rsplit_once('-').map(|(project, _)| project)
                    == Some(card.project_key.as_str()))
            .then_some(key)
        }) {
            return Err(format!("{other} is running in this project"));
        }
        self.projects
            .iter()
            .find(|p| p.key == card.project_key)
            .and_then(|p| p.path.as_deref())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| "project has no folder".to_string())
    }

    fn cancel_pending(&mut self) {
        self.detecting = None;
        self.picking = None;
    }

    fn with_leave_bar<'a>(&'a self, content: Element<'a, Message>) -> Element<'a, Message> {
        match self.detail.as_ref().and_then(Detail::leave_bar) {
            Some(bar) => column![bar, content].into(),
            None => content,
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
                self.notify_events();
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
        let layers = stack![
            backdrop,
            column![
                titlebar::view(self),
                body,
                rule::horizontal(1),
                self.view_footer()
            ]
        ];
        match self.view_ghost() {
            Some(ghost) => layers.push(ghost).into(),
            None => layers.into(),
        }
    }

    fn view_ghost(&self) -> Option<Element<'_, Message>> {
        let drag = self.drag.as_ref().filter(|d| d.moved)?;
        let card = self.board.iter().find(|c| c.key == drag.key)?;
        let lift = drag
            .lifted
            .map_or(1.0, |at| at.elapsed().as_secs_f32() / LIFT_SECS);
        let ghost = container(card_face(
            card,
            theme::PRIMARY,
            self.runs.get(&card.key).is_some_and(Run::is_running),
        ))
        .width(GHOST_WIDTH)
        .style(theme::ghost);
        Some(
            pin(float(ghost).scale(1.0 + 0.04 * ease_out(lift)))
                .position(drag.cursor - GHOST_OFFSET)
                .into(),
        )
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
            return self.with_leave_bar(sheet.view(self.detecting.is_some()).map(Message::Sheet));
        }
        if let Some(import) = &self.import {
            return self.with_leave_bar(view_import(import));
        }
        if self.detecting.is_some() {
            return self.with_leave_bar(
                container(text("inspecting folder…").color(theme::TEXT_MUTED))
                    .padding(20)
                    .into(),
            );
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
            text_input("search", &self.search)
                .id(SEARCH_ID)
                .on_input(Message::Search)
                .style(theme::input)
                .width(220),
            pick_list(
                self.filter_options(),
                Some(&self.filter),
                Message::FilterChanged
            )
            .style(theme::picker),
            action.style(theme::button(false)),
        ]
        .spacing(20)
        .align_y(Alignment::Center);
        let mut main = row![column![columns, activity::view(&self.events, self.t)].spacing(10)]
            .spacing(10)
            .height(Length::Fill);
        if let Some(detail) = &self.detail {
            main = main.push(detail.view(
                &self.theme,
                self.runs.get(&detail.key),
                self.launch_dir().err(),
            ));
        }
        column![header, main]
            .spacing(14)
            .padding([16, 20])
            .width(Length::Fill)
            .into()
    }

    fn filter_options(&self) -> Vec<Filter> {
        let tiers: BTreeSet<&String> = self.board.iter().filter_map(|c| c.tier.as_ref()).collect();
        let mut options = vec![Filter::All, Filter::Human, Filter::Held];
        options.extend(tiers.into_iter().cloned().map(Filter::Tier));
        if !options.contains(&self.filter) {
            options.push(self.filter.clone());
        }
        options
    }

    fn view_column(&self, col: Column) -> Element<'_, Message> {
        let in_column: Vec<&CardSummary> = self.board.iter().filter(|c| c.column == col).collect();
        let cards: Vec<&CardSummary> = in_column
            .iter()
            .copied()
            .filter(|c| card_matches(c, &self.search, &self.filter))
            .collect();
        let (name_color, gauge_color) = match col {
            Column::Review => (theme::WARNING, theme::WARNING),
            Column::Done => (theme::SUCCESS, theme::SUCCESS),
            _ => (theme::TEXT, theme::PRIMARY),
        };
        let grow = ease_out(self.reloaded.elapsed().as_secs_f32());
        let fill = (in_column.len() as f32).min(theme::COLUMN_CAP) / theme::COLUMN_CAP * grow;
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
        let moving = self.drag.as_ref().filter(|d| d.moved);
        let target = moving.filter(|d| d.over == Some(col));
        let list = column(cards.into_iter().map(|card| {
            let flash = self
                .appeared
                .get(&card.key)
                .map(|at| at.elapsed().as_secs_f32() / FLASH_SECS)
                .filter(|age| *age < 1.0);
            let dragged = moving.is_some_and(|d| d.key == card.key);
            let gap = moving
                .filter(|d| d.key != card.key)
                .map_or(0.0, |d| d.gap(col, Some(&card.key)));
            let boxed = card_box(
                card,
                open == Some(card.key.as_str()),
                self.hovered.as_deref() == Some(card.key.as_str()),
                flash,
                moving.is_some(),
                dragged,
                self.runs.get(&card.key).is_some_and(Run::is_running),
                gap,
            );
            match &self.landed {
                Some((key, at)) if *key == card.key => {
                    let settle = 1.0 - ease_out(at.elapsed().as_secs_f32() / LAND_SECS);
                    float(boxed).scale(1.0 + 0.05 * settle).into()
                }
                _ => boxed,
            }
        }));
        let tail_gap = moving.map_or(0.0, |d| d.gap(col, None));
        let mut tail = column![];
        if tail_gap > 0.0 {
            tail = tail.push(gap_box(tail_gap));
        }
        let tail = mouse_area(container(tail).width(Length::Fill).height(DROP_GAP))
            .on_enter(Message::EnterTail(col));
        let list = scrollable(list.push(tail))
            .direction(theme::thin())
            .style(theme::scrollbar);
        let panel = container(column![header, rule::horizontal(1), list].spacing(10))
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(if target.is_some() {
                theme::glass_lit
            } else {
                theme::glass
            });
        let brackets = match (col, target) {
            (_, Some(drag)) => fx::Brackets {
                color: theme::ACCENT,
                alpha: 0.75 + 0.25 * (self.t * 2.0 * PI).sin(),
                len: fx::BRACKET + 8.0 * ease_out(drag.over_at.elapsed().as_secs_f32() / GAP_SECS),
            },
            (Column::Doing, None) => fx::Brackets {
                color: theme::PRIMARY,
                alpha: 0.7 + 0.3 * (self.t * PI).sin(),
                len: fx::BRACKET,
            },
            _ => fx::Brackets {
                color: theme::BORDER_STRONG,
                alpha: 1.0,
                len: fx::BRACKET,
            },
        };
        let area = mouse_area(
            stack![
                panel,
                canvas(brackets).width(Length::Fill).height(Length::Fill)
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .on_enter(Message::EnterColumn(col))
        .on_release(Message::DropOn(col));
        match moving {
            Some(_) => area.interaction(mouse::Interaction::Grabbing).into(),
            None => area.into(),
        }
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
/// px
const DROP_GAP: f32 = 44.0;
const GAP_SECS: f32 = 0.15;
const LIFT_SECS: f32 = 0.12;
const LAND_SECS: f32 = 0.25;
/// px
const GHOST_WIDTH: f32 = 260.0;
/// Cursor position inside the ghost, px.
const GHOST_OFFSET: Vector = Vector::new(24.0, 14.0);

fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}

fn card_face(card: &CardSummary, bar: Color, running: bool) -> Element<'_, Message> {
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
    } else if running {
        body = body.push(text("▶ truck").size(11).color(theme::ACCENT));
    }
    row![
        container(space())
            .width(2)
            .height(Length::Fill)
            .style(theme::fill(bar)),
        container(body).padding(8).width(Length::Fill),
    ]
    .height(Length::Shrink)
    .into()
}

fn gap_box<'a>(height: f32) -> Element<'a, Message> {
    container(space())
        .width(Length::Fill)
        .height(height)
        .style(theme::drop_slot)
        .into()
}

#[allow(clippy::too_many_arguments)]
fn card_box(
    card: &CardSummary,
    selected: bool,
    hovered: bool,
    flash: Option<f32>,
    dragging: bool,
    dragged: bool,
    running: bool,
    gap: f32,
) -> Element<'_, Message> {
    let bar = if dragged {
        theme::BORDER
    } else if selected {
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
    let face = container(card_face(card, bar, running)).width(Length::Fill);
    let face = if dragged {
        face.style(theme::placeholder)
    } else {
        face.style(theme::card(border))
    };
    let mut boxed = column![];
    if gap > 0.0 {
        boxed = boxed.push(gap_box(gap));
    }
    mouse_area(container(boxed.push(face)).padding(Padding::ZERO.bottom(6)))
        .on_press(Message::Press(card.key.clone()))
        .on_enter(Message::EnterCard(card.key.clone()))
        .on_exit(Message::ExitCard(card.key.clone()))
        .interaction(if dragging {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Pointer
        })
        .into()
}

fn drop_index(cards: &[&CardSummary], dragged: &str, before: Option<&str>) -> usize {
    let mut index = 0;
    for card in cards {
        if Some(card.key.as_str()) == before {
            return index;
        }
        if card.key != dragged {
            index += 1;
        }
    }
    index
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

#[cfg(test)]
mod tests {
    use super::*;

    fn card(key: &str, title: &str) -> CardSummary {
        CardSummary {
            key: key.to_string(),
            title: title.to_string(),
            column: Column::Backlog,
            position: 0,
            tier: None,
            human: false,
            holder: None,
        }
    }

    #[test]
    fn matches_key_and_title_ignoring_case() {
        let c = card("FS-12", "Fix the Gauge");
        assert!(card_matches(&c, "fs-1", &Filter::All));
        assert!(card_matches(&c, "GAUGE", &Filter::All));
        assert!(card_matches(&c, "", &Filter::All));
        assert!(!card_matches(&c, "toast", &Filter::All));
    }

    #[test]
    fn human_filter() {
        let mut c = card("FS-1", "a");
        assert!(!card_matches(&c, "", &Filter::Human));
        c.human = true;
        assert!(card_matches(&c, "", &Filter::Human));
    }

    #[test]
    fn held_filter() {
        let mut c = card("FS-1", "a");
        assert!(!card_matches(&c, "", &Filter::Held));
        c.holder = Some(manager());
        assert!(card_matches(&c, "", &Filter::Held));
    }

    #[test]
    fn tier_filter() {
        let mut c = card("FS-1", "a");
        assert!(!card_matches(&c, "", &Filter::Tier("T1".into())));
        c.tier = Some("T1".into());
        assert!(card_matches(&c, "", &Filter::Tier("T1".into())));
        assert!(!card_matches(&c, "", &Filter::Tier("T2".into())));
    }

    #[test]
    fn search_and_filter_combine() {
        let mut c = card("FS-1", "search box");
        c.human = true;
        assert!(card_matches(&c, "search", &Filter::Human));
        assert!(!card_matches(&c, "other", &Filter::Human));
        assert!(!card_matches(&c, "search", &Filter::Held));
    }

    fn index(keys: &[&str], dragged: &str, before: Option<&str>) -> usize {
        let cards: Vec<CardSummary> = keys
            .iter()
            .enumerate()
            .map(|(i, key)| CardSummary {
                key: key.to_string(),
                title: String::new(),
                column: Column::Ready,
                position: i as i64,
                tier: None,
                human: false,
                holder: None,
            })
            .collect();
        let refs: Vec<&CardSummary> = cards.iter().collect();
        drop_index(&refs, dragged, before)
    }

    #[test]
    fn drop_before_first() {
        assert_eq!(index(&["A", "B", "C"], "X", Some("A")), 0);
    }

    #[test]
    fn drop_before_middle() {
        assert_eq!(index(&["A", "B", "C"], "X", Some("B")), 1);
    }

    #[test]
    fn drop_appends() {
        assert_eq!(index(&["A", "B", "C"], "X", None), 3);
    }

    #[test]
    fn drop_excludes_dragged_card() {
        assert_eq!(index(&["A", "B", "C"], "A", Some("C")), 1);
        assert_eq!(index(&["A", "B", "C"], "A", None), 2);
    }

    #[test]
    fn drop_before_dragged_keeps_its_place() {
        assert_eq!(index(&["A", "B", "C"], "B", Some("B")), 1);
    }

    #[test]
    fn quick_slot_changes_keep_every_closing_gap() {
        let now = Instant::now();
        let mut drag = Drag {
            key: "X".into(),
            over: Some(Column::Ready),
            over_at: now,
            moved: true,
            cursor: Point::ORIGIN,
            lifted: Some(now),
            slot: Some((Column::Ready, Some("A".into()))),
            slot_at: now - Duration::from_secs(1),
            closing: Vec::new(),
        };
        drag.set_slot(Some((Column::Ready, Some("B".into()))));
        drag.set_slot(Some((Column::Ready, Some("C".into()))));
        assert!(drag.gap(Column::Ready, Some("A")) > 0.0);
        assert_eq!(drag.closing.len(), 2);
    }
}
