use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length};

use super::{Message, local_time, role_glyph, theme};
use crate::store::Event;

pub fn view(events: &[Event]) -> Element<'_, Message> {
    let list = column(events.iter().map(|e| {
        let color = match e.kind.as_str() {
            "report" => theme::TEXT,
            "review_result" if e.message.contains("findings") => theme::DANGER,
            "ship" => theme::SUCCESS,
            _ => theme::TEXT_MUTED,
        };
        row![
            text(local_time(e.created_at, "%H:%M")).color(theme::TEXT_MUTED),
            text(role_glyph(e.actor.role)).color(theme::ACCENT),
            text(e.card_key.as_deref().unwrap_or("")).width(70),
            text(&e.message).color(color),
        ]
        .spacing(10)
        .into()
    }))
    .spacing(3);
    container(column![text("ACTIVITY"), scrollable(list).width(Length::Fill)].spacing(10))
        .padding(10)
        .width(Length::Fill)
        .height(170)
        .style(theme::panel)
        .into()
}
