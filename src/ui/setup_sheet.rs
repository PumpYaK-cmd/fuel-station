use std::path::PathBuf;

use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length, Task};

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
    busy: bool,
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
    Applied(Vec<Written>),
    Close,
}

pub fn detect(dir: PathBuf, taken: Vec<String>) -> Task<(NewProject, Facts)> {
    Task::future(async move {
        tokio::task::spawn_blocking(move || (detect::detect(&dir, &taken), detect::facts(&dir)))
            .await
            .unwrap_or_default()
    })
}

impl Sheet {
    pub fn add(form: NewProject, facts: Facts) -> Sheet {
        Sheet {
            repair: None,
            form,
            facts,
            busy: false,
            error: None,
            result: None,
        }
    }

    pub fn repair(project: &Project, found: Option<(NewProject, Facts)>) -> Sheet {
        let mut form = NewProject {
            key: project.key.clone(),
            name: project.name.clone(),
            path: project.path.clone(),
            ship_mode: project.ship_mode,
            default_branch: project.default_branch.clone(),
            verify_cmd: project.verify_cmd.clone(),
            worktree_dir: project.worktree_dir.clone(),
            bootstrap_cmd: project.bootstrap_cmd.clone(),
            ship_cmd: project.ship_cmd.clone(),
        };
        let mut facts = Facts::default();
        if let Some((found, found_facts)) = found {
            if project.default_branch.is_empty() {
                form.ship_mode = found.ship_mode;
            }
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
            facts = found_facts;
        }
        Sheet {
            repair: Some(project.key.clone()),
            form,
            facts,
            busy: false,
            error: None,
            result: None,
        }
    }

    pub fn update(
        &mut self,
        message: Message,
        store: &mut Store,
    ) -> (Option<String>, Task<Message>) {
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
                        self.busy = true;
                        let key = project.key.clone();
                        let apply = Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || {
                                    write::apply(&project, &installed_exe())
                                })
                                .await
                                .unwrap_or_else(|e| {
                                    vec![Written {
                                        path: PathBuf::new(),
                                        action: format!("error: {e}"),
                                    }]
                                })
                            },
                            Message::Applied,
                        );
                        return (Some(key), apply);
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            Message::Applied(result) => {
                if !self.busy {
                    return (None, Task::none());
                }
                self.busy = false;
                self.result = Some(result);
            }
            Message::Close => {}
        }
        (None, Task::none())
    }

    pub fn view(&self, detecting: bool) -> Element<'_, Message> {
        let mut title = match &self.repair {
            Some(key) => format!("Repair setup · {key}"),
            None => "Add Project".to_string(),
        };
        if detecting {
            title.push_str(" · inspecting folder…");
        }
        let body = match &self.result {
            Some(result) => self.view_result(result),
            None => self.view_form(),
        };
        container(column![text(title), body].spacing(20).padding(20))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::glass)
            .into()
    }

    fn view_form(&self) -> Element<'_, Message> {
        let form = &self.form;
        let mut key = text_input("KEY", &form.key).style(theme::input);
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
                .style(theme::picker)
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
        if self.busy {
            content = content.push(text("setting up…").color(theme::TEXT_MUTED));
        }
        let buttons = row![
            button(text("Set up"))
                .on_press_maybe((!self.busy).then_some(Message::Submit))
                .style(theme::button(true)),
            button(text("Cancel"))
                .on_press(Message::Close)
                .style(theme::button(false)),
        ]
        .spacing(10);
        scrollable(content.push(buttons).width(Length::Fill))
            .direction(theme::thin())
            .style(theme::scrollbar)
            .into()
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
        scrollable(column![rows, done].spacing(20))
            .direction(theme::thin())
            .style(theme::scrollbar)
            .into()
    }
}

fn input<'a>(placeholder: &'a str, value: &'a str, field: Field) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |v| Message::Edit(field, v))
        .style(theme::input)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn project(ship_mode: ShipMode, branch: &str, verify: &str) -> Project {
        Project {
            id: 1,
            key: "FS".to_string(),
            name: "Fuel".to_string(),
            path: Some("C:/fuel".to_string()),
            ship_mode,
            default_branch: branch.to_string(),
            verify_cmd: verify.to_string(),
            worktree_dir: String::new(),
            bootstrap_cmd: String::new(),
            ship_cmd: String::new(),
            created_at: 0,
            archived: false,
        }
    }

    fn found_studio() -> (NewProject, Facts) {
        let form = NewProject {
            key: "OTHER".to_string(),
            name: "other".to_string(),
            path: Some("C:/elsewhere".to_string()),
            ship_mode: ShipMode::Studio,
            default_branch: "master".to_string(),
            verify_cmd: "selene src".to_string(),
            worktree_dir: "../fuel-wt".to_string(),
            bootstrap_cmd: String::new(),
            ship_cmd: "rojo build".to_string(),
        };
        (form, Facts::default())
    }

    #[test]
    fn repair_imported_takes_detected_ship_mode_and_fills_fields() {
        let sheet = Sheet::repair(&project(ShipMode::Git, "", ""), Some(found_studio()));
        let form = &sheet.form;
        assert_eq!(form.ship_mode, ShipMode::Studio);
        assert_eq!(form.default_branch, "master");
        assert_eq!(form.verify_cmd, "selene src");
        assert_eq!(form.worktree_dir, "../fuel-wt");
        assert_eq!(form.ship_cmd, "rojo build");
        assert_eq!(form.key, "FS");
        assert_eq!(form.path.as_deref(), Some("C:/fuel"));
    }

    #[test]
    fn applied_without_pending_setup_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("station.db")).unwrap();
        let mut sheet = Sheet::add(NewProject::default(), Facts::default());
        let applied = Message::Applied(vec![Written {
            path: PathBuf::new(),
            action: "created".to_string(),
        }]);
        let _ = sheet.update(applied, &mut store);
        assert!(sheet.result.is_none());
    }

    #[test]
    fn repair_configured_keeps_ship_mode_and_set_fields() {
        let sheet = Sheet::repair(
            &project(ShipMode::Git, "main", "cargo test"),
            Some(found_studio()),
        );
        let form = &sheet.form;
        assert_eq!(form.ship_mode, ShipMode::Git);
        assert_eq!(form.default_branch, "main");
        assert_eq!(form.verify_cmd, "cargo test");
        assert_eq!(form.worktree_dir, "../fuel-wt");
    }
}
