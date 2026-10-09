use iced::widget::{column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};

use super::{Message, local_time, role_glyph, theme};
use crate::store::Event;

pub fn view(events: &[Event], t: f32) -> Element<'_, Message> {
    let list = column(events.iter().map(|e| {
        let color = match e.kind.as_str() {
            "report" => theme::TEXT,
            "review_result" if !e.message.ends_with("Review: clean") => theme::DANGER,
            "ship" => theme::SUCCESS,
            _ => theme::TEXT_MUTED,
        };
        row![
            row![
                text(local_time(e.created_at, "%H:%M")).color(theme::TEXT_MUTED),
                text("│").color(theme::BORDER_STRONG),
            ]
            .spacing(6),
            text(role_glyph(e.actor.role)).color(theme::ACCENT),
            text(e.card_key.as_deref().unwrap_or("")).width(70),
            text(&e.message).color(color),
        ]
        .spacing(10)
        .into()
    }))
    .spacing(3);
    let live = if (t * 0.625) % 1.0 < 0.5 { 1.0 } else { 0.25 };
    let header = row![
        text("ACTIVITY").size(11),
        text("LIVE")
            .size(10)
            .color(theme::SUCCESS.scale_alpha(live)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let list = scrollable(list)
        .direction(theme::thin())
        .style(theme::scrollbar)
        .width(Length::Fill);
    container(column![header, list].spacing(10))
        .padding(10)
        .width(Length::Fill)
        .height(170)
        .style(theme::glass)
        .into()
}
