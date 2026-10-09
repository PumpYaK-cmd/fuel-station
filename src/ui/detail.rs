use iced::widget::{
    button, column, container, markdown, row, rule, scrollable, space, text, text_editor,
    text_input,
};
use iced::{Alignment, Element, Length, Theme};

use super::runs::Run;
use super::{Message, local_time, role_glyph, theme};
use crate::store::{CardDetail, Column};

#[derive(Debug, Clone)]
pub enum Leave {
    Close,
    Open(String),
    Select(String),
    Quit,
}

pub struct Detail {
    pub key: String,
    pub card: CardDetail,
    pub plan: Vec<markdown::Item>,
    pub title: String,
    pub body: text_editor::Content,
    pub comment: String,
    pub dirty: bool,
    pub leaving: Option<Leave>,
}

impl Detail {
    pub fn new(card: CardDetail) -> Detail {
        Detail {
            key: card.card.key.clone(),
            plan: markdown::parse(&card.card.plan).collect(),
            title: card.card.title.clone(),
            body: text_editor::Content::with_text(&card.card.body),
            comment: String::new(),
            dirty: false,
            leaving: None,
            card,
        }
    }

    pub fn refresh(&mut self, card: CardDetail) {
        self.plan = markdown::parse(&card.card.plan).collect();
        if !self.dirty {
            self.title.clone_from(&card.card.title);
            if self.body.text() != card.card.body {
                self.body = text_editor::Content::with_text(&card.card.body);
            }
        }
        self.card = card;
    }

    pub fn view<'a>(
        &'a self,
        app_theme: &'a Theme,
        run: Option<&'a Run>,
        can_launch: bool,
    ) -> Element<'a, Message> {
        let card = &self.card.card;
        let header = row![
            text(format!("{} · {}", card.key, card.column.as_str())),
            space::horizontal(),
            button(text("\u{f00d}"))
                .on_press(Message::CloseDetail)
                .style(theme::button(false)),
        ]
        .align_y(Alignment::Center);

        let edit = column![
            text_input("title", &self.title)
                .on_input(Message::TitleEdited)
                .style(theme::input),
            text_editor(&self.body)
                .on_action(Message::BodyAction)
                .style(theme::editor)
                .height(160),
            button(text("Save"))
                .on_press_maybe(
                    (self.dirty && !self.title.trim().is_empty()).then_some(Message::Save),
                )
                .style(theme::button(false)),
        ]
        .spacing(6);

        let plan: Element<'_, Message> = if self.plan.is_empty() {
            text("no plan").color(theme::TEXT_MUTED).into()
        } else {
            markdown::view(
                &self.plan,
                markdown::Settings::with_text_size(14, app_theme),
            )
            .map(|_| Message::LinkClicked)
        };

        let mut meta = Vec::new();
        if let Some(tier) = &card.tier {
            meta.push(format!("tier {tier}"));
        }
        if card.human {
            meta.push("HUMAN".to_string());
        }
        if card.commit_base.is_some() || card.commit_head.is_some() {
            meta.push(format!(
                "{}..{}",
                card.commit_base.as_deref().unwrap_or(""),
                card.commit_head.as_deref().unwrap_or("")
            ));
        }

        let comments = column(self.card.comments.iter().map(|c| {
            column![
                text(format!(
                    "{} {} · {}",
                    role_glyph(c.actor.role),
                    c.actor.name,
                    local_time(c.created_at, "%Y-%m-%d %H:%M")
                ))
                .color(theme::TEXT_MUTED),
                text(&c.body),
            ]
            .spacing(2)
            .into()
        }))
        .spacing(8);
        let comment_box = row![
            text_input("comment", &self.comment)
                .on_input(Message::CommentEdited)
                .on_submit(Message::AddComment)
                .style(theme::input),
            button(text("Comment"))
                .on_press_maybe((!self.comment.trim().is_empty()).then_some(Message::AddComment))
                .style(theme::button(false)),
        ]
        .spacing(6);

        let events = column(self.card.events.iter().map(|e| {
            text(format!(
                "{} {} {}",
                local_time(e.created_at, "%Y-%m-%d %H:%M"),
                e.kind,
                e.message
            ))
            .color(theme::TEXT_MUTED)
            .into()
        }))
        .spacing(3);

        let mut moves = row![].spacing(6);
        if card.column.prev().is_some() {
            moves = moves.push(move_button("← Prev", Message::Step(false)));
        }
        for col in Column::ALL.into_iter().filter(|&c| c != card.column) {
            moves = moves.push(move_button(col.as_str(), Message::MoveTo(col)));
        }
        if card.column.next().is_some() {
            moves = moves.push(move_button("Next →", Message::Step(true)));
        }

        let bar = self.leave_bar();

        let mut content = column![header, edit, rule::horizontal(1), label("PLAN"), plan];
        if !meta.is_empty() {
            content = content.push(text(meta.join(" · ")).color(theme::TEXT_MUTED));
        }
        let content = content
            .push(rule::horizontal(1))
            .push(label("COMMENTS"))
            .push(comments)
            .push(comment_box)
            .push(rule::horizontal(1))
            .push(label("EVENTS"))
            .push(events)
            .push(rule::horizontal(1))
            .push(moves.wrap())
            .push(rule::horizontal(1))
            .push(label("RUN"))
            .push(run_section(run, can_launch))
            .spacing(10)
            .padding(10);

        container(
            column![].extend(bar).push(
                scrollable(content)
                    .direction(theme::thin())
                    .style(theme::scrollbar),
            ),
        )
        .width(440)
        .height(Length::Fill)
        .style(theme::glass)
        .into()
    }

    pub fn leave_bar(&self) -> Option<Element<'_, Message>> {
        self.leaving.is_some().then(|| {
            row![
                text("Unsaved changes").color(theme::WARNING),
                button(text("Save"))
                    .on_press_maybe(
                        (!self.title.trim().is_empty()).then_some(Message::SaveAndLeave),
                    )
                    .style(theme::button(false)),
                button(text("Discard"))
                    .on_press(Message::DiscardAndLeave)
                    .style(theme::button(false)),
                button(text("Keep editing"))
                    .on_press(Message::KeepEditing)
                    .style(theme::button(false)),
            ]
            .spacing(6)
            .padding(10)
            .align_y(Alignment::Center)
            .into()
        })
    }
}

fn label(name: &str) -> Element<'_, Message> {
    text(name).color(theme::TEXT_MUTED).into()
}

fn run_section(run: Option<&Run>, can_launch: bool) -> Element<'_, Message> {
    let action = match run {
        Some(run) if run.is_running() => button(text("Stop")).on_press(Message::StopRun),
        _ => button(text("Launch Truck")).on_press_maybe(can_launch.then_some(Message::Launch)),
    };
    let mut section = column![action.style(theme::button(false))].spacing(6);
    if let Some(run) = run {
        section = section
            .push(text(run.status_text()).color(theme::TEXT_MUTED))
            .push(
                scrollable(column(run.lines.iter().map(|l| text(l).size(11).into())).spacing(2))
                    .anchor_bottom()
                    .direction(theme::thin())
                    .style(theme::scrollbar)
                    .height(200),
            );
    }
    section.into()
}

fn move_button(name: &str, message: Message) -> Element<'_, Message> {
    button(text(name))
        .on_press(message)
        .style(theme::button(false))
        .into()
}
