use iced::widget::{button, column, container, row, rule, space, text};
use iced::{Color, Element, Length};

use super::theme;

#[derive(Debug, Clone)]
pub enum Message {
    Pressed,
}

const TOKENS: [(&str, Color); 12] = [
    ("background", theme::BACKGROUND),
    ("surface", theme::SURFACE),
    ("surface_hover", theme::SURFACE_HOVER),
    ("border", theme::BORDER),
    ("border_strong", theme::BORDER_STRONG),
    ("text", theme::TEXT),
    ("text_muted", theme::TEXT_MUTED),
    ("primary", theme::PRIMARY),
    ("accent", theme::ACCENT),
    ("success", theme::SUCCESS),
    ("warning", theme::WARNING),
    ("danger", theme::DANGER),
];

pub fn update(_state: &mut (), _message: Message) {}

pub fn view(_state: &()) -> Element<'_, Message> {
    let tokens = column(TOKENS.chunks(3).map(|chunk| {
        row(chunk.iter().map(|&(name, color)| token(name, color)))
            .spacing(10)
            .into()
    }))
    .spacing(10);

    let buttons = row![
        button(text("normal"))
            .on_press(Message::Pressed)
            .style(theme::button(false)),
        button(text("selected"))
            .on_press(Message::Pressed)
            .style(theme::button(true)),
    ]
    .spacing(10);

    let panel = container(column![
        text("panel"),
        text("muted text").color(theme::TEXT_MUTED),
    ])
    .padding(10)
    .style(theme::panel);

    let cards = row![
        container(text("card"))
            .padding(10)
            .style(theme::card_box(false)),
        container(text("warning card"))
            .padding(10)
            .style(theme::card_box(true)),
    ]
    .spacing(10);

    let right = column![buttons, rule::horizontal(1), panel, cards].spacing(20);

    container(row![tokens, rule::vertical(1), right].spacing(20))
        .padding(20)
        .into()
}

fn token(name: &'static str, color: Color) -> Element<'static, Message> {
    let square = container(space())
        .width(24)
        .height(24)
        .style(move |_| container::Style::default().background(color));
    row![
        square,
        column![
            text(name).color(color),
            text(color.to_string()).color(color)
        ]
    ]
    .spacing(8)
    .width(Length::Fixed(190.0))
    .into()
}
