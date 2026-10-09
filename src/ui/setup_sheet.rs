use std::path::Path;

use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

use super::theme;
use crate::setup::detect::{self, Facts};
use crate::setup::install::installed_exe;
use crate::setup::write::{self, Written};
use crate::store::{NewProject, Project, ShipMode, Store};

const SHIP_MODES: [&str; 2] = ["git", "studio"];

pub struct Sheet {
    repair: Option<String>,
    form: NewProject,
    facts: Facts,
    error: Option<String>,
    result: Option<Vec<Written>>,
}

#[derive(Debug, Clone, Copy)]
pub enum Field {
    Name,
    Key,
    Path,
    Branch,
    Verify,
    Worktree,
    Bootstrap,
    Ship,
}

#[derive(Debug, Clone)]
pub enum Message {
    Edit(Field, String),
    ShipMode(ShipMode),
    Submit,
    Close,
}

impl Sheet {
    pub fn add(dir: &Path, taken_keys: &[String]) -> Sheet {
        Sheet {
            repair: None,
            form: detect::detect(dir, taken_keys),
            facts: detect::facts(dir),
            error: None,
            result: None,
        }
    }

    pub fn repair(project: &Project) -> Sheet {
        let facts = project
            .path
            .as_deref()
            .map(|p| detect::facts(Path::new(p)))
            .unwrap_or_default();
        Sheet {
            repair: Some(project.key.clone()),
            form: NewProject {
                key: project.key.clone(),
                name: project.name.clone(),
                path: project.path.clone(),
                ship_mode: project.ship_mode,
                default_branch: project.default_branch.clone(),
                verify_cmd: project.verify_cmd.clone(),
                worktree_dir: project.worktree_dir.clone(),
                bootstrap_cmd: project.bootstrap_cmd.clone(),
                ship_cmd: project.ship_cmd.clone(),
            },
            facts,
            error: None,
            result: None,
        }
    }

    pub fn repair_at(project: &Project, dir: &Path, taken_keys: &[String]) -> Sheet {
        let mut sheet = Sheet::repair(project);
        let found = detect::detect(dir, taken_keys);
        let form = &mut sheet.form;
        if form.path.as_deref().is_none_or(str::is_empty) {
            form.path = found.path;
        }
        for (field, value) in [
            (&mut form.default_branch, found.default_branch),
            (&mut form.verify_cmd, found.verify_cmd),
            (&mut form.worktree_dir, found.worktree_dir),
            (&mut form.bootstrap_cmd, found.bootstrap_cmd),
            (&mut form.ship_cmd, found.ship_cmd),
        ] {
            if field.is_empty() {
                *field = value;
            }
        }
        sheet.facts = detect::facts(dir);
        sheet
    }

    pub fn update(&mut self, message: Message, store: &mut Store) -> Option<String> {
        match message {
            Message::Edit(field, value) => {
                let form = &mut self.form;
                match field {
                    Field::Name => form.name = value,
                    Field::Key => form.key = value,
                    Field::Path => form.path = (!value.is_empty()).then_some(value),
                    Field::Branch => form.default_branch = value,
                    Field::Verify => form.verify_cmd = value,
                    Field::Worktree => form.worktree_dir = value,
                    Field::Bootstrap => form.bootstrap_cmd = value,
                    Field::Ship => form.ship_cmd = value,
                }
            }
            Message::ShipMode(mode) => self.form.ship_mode = mode,
            Message::Submit => {
                let saved = match &self.repair {
                    Some(key) => store.project_update(key, &self.form),
                    None => store.project_add(&self.form),
                };
                match saved {
                    Ok(project) => {
                        self.error = None;
                        self.result = Some(write::apply(&project, &installed_exe()));
                        return Some(project.key);
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            Message::Close => {}
        }
        None
    }

    pub fn view(&self) -> Element<'_, Message> {
        let title = match &self.repair {
            Some(key) => format!("Repair setup · {key}"),
            None => "Add Project".to_string(),
        };
        let body = match &self.result {
            Some(result) => self.view_result(result),
            None => self.view_form(),
        };
        column![text(title), body]
            .spacing(20)
            .padding(20)
            .width(Length::Fill)
            .into()
    }

    fn view_form(&self) -> Element<'_, Message> {
        let form = &self.form;
        let mut key = text_input("KEY", &form.key);
        if self.repair.is_none() {
            key = key.on_input(|v| Message::Edit(Field::Key, v));
        }
        let fields = column![
            labelled("Name", input("name", &form.name, Field::Name)),
            labelled("Key", key.into()),
            labelled(
                "Path",
                input("folder", form.path.as_deref().unwrap_or(""), Field::Path)
            ),
            labelled(
                "Ship mode",
                pick_list(SHIP_MODES, Some(form.ship_mode.as_str()), |s| {
                    Message::ShipMode(s.parse().unwrap_or_default())
                })
                .into()
            ),
            labelled("Branch", input("main", &form.default_branch, Field::Branch)),
            labelled(
                "Verify gate",
                input("command", &form.verify_cmd, Field::Verify)
            ),
            labelled(
                "Worktree dir",
                input("../name-wt", &form.worktree_dir, Field::Worktree)
            ),
            labelled(
                "Bootstrap",
                input("command", &form.bootstrap_cmd, Field::Bootstrap)
            ),
            labelled(
                "Ship command",
                input("command", &form.ship_cmd, Field::Ship)
            ),
        ]
        .spacing(8);
        let facts = &self.facts;
        let facts = column![
            fact("git repo", if facts.is_git { "yes" } else { "no" }),
            fact("remote", facts.remote.as_deref().unwrap_or("none")),
            fact(
                "CLAUDE.md",
                if facts.has_claude_md {
                    "present"
                } else {
                    "missing"
                }
            ),
        ]
        .spacing(4);
        let mut content = column![fields, facts].spacing(20);
        if let Some(e) = &self.error {
            content = content.push(text(e).color(theme::DANGER));
        }
        let buttons = row![
            button(text("Set up"))
                .on_press(Message::Submit)
                .style(theme::button(true)),
            button(text("Cancel"))
                .on_press(Message::Close)
                .style(theme::button(false)),
        ]
        .spacing(10);
        scrollable(content.push(buttons).width(Length::Fill)).into()
    }

    fn view_result<'a>(&'a self, result: &'a [Written]) -> Element<'a, Message> {
        let rows = column(result.iter().map(|w| {
            let action = text(&w.action);
            let action = if w.action.starts_with("error") {
                action.color(theme::DANGER)
            } else if w.action == "unchanged" {
                action.color(theme::TEXT_MUTED)
            } else {
                action.color(theme::SUCCESS)
            };
            row![
                text(w.path.display().to_string()).width(Length::Fill),
                action
            ]
            .spacing(20)
            .into()
        }))
        .spacing(6);
        let done = button(text("Done"))
            .on_press(Message::Close)
            .style(theme::button(false));
        scrollable(column![rows, done].spacing(20)).into()
    }
}

fn input<'a>(placeholder: &'a str, value: &'a str, field: Field) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |v| Message::Edit(field, v))
        .into()
}

fn labelled<'a>(label: &'a str, field: Element<'a, Message>) -> Element<'a, Message> {
    row![text(label).width(140), field].spacing(10).into()
}

fn fact<'a>(label: &'a str, value: &'a str) -> Element<'a, Message> {
    row![text(label).width(140).color(theme::TEXT_MUTED), text(value)]
        .spacing(10)
        .into()
}
