use std::f32::consts::PI;

use iced::widget::{button, column, container, mouse_area, row, rule, space, text};
use iced::{Alignment, Element, Length};

use super::{App, Message, role_glyph, theme};

const HEIGHT: f32 = 36.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let brand = row![
        text("\u{f0298}").color(theme::ACCENT),
        text("FUEL STATION").size(11),
    ]
    .spacing(8);
    let bar = row![
        brand,
        space::horizontal(),
        agents(app),
        row![
            window_button("\u{f2d1}", Message::Minimize, false),
            window_button("\u{f2d0}", Message::ToggleMaximize, false),
            window_button("\u{f00d}", Message::Close, true),
        ],
    ]
    .spacing(24)
    .height(HEIGHT)
    .align_y(Alignment::Center);
    let bar = container(row![space().width(16), bar]).style(theme::bar);
    mouse_area(column![bar, rule::horizontal(1)])
        .on_press(Message::DragWindow)
        .on_double_click(Message::ToggleMaximize)
        .into()
}

fn agents(app: &App) -> Element<'_, Message> {
    let count = app.agents.len();
    let dot = if count > 0 {
        theme::ACCENT.scale_alpha(0.6 + 0.4 * (app.t * PI).sin())
    } else {
        theme::TEXT_MUTED
    };
    let mut strip = row![
        text("●").color(dot),
        text(format!("{count} agents")).color(theme::TEXT_MUTED),
    ]
    .spacing(14)
    .align_y(Alignment::Center);
    for agent in &app.agents {
        strip = strip.push(
            text(format!(
                "{} {} {}",
                role_glyph(agent.actor.role),
                agent.actor.name,
                agent.card_key.as_deref().unwrap_or("")
            ))
            .color(theme::ACCENT),
        );
    }
    strip.into()
}

fn window_button(glyph: &str, message: Message, close: bool) -> Element<'_, Message> {
    button(
        container(text(glyph))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(46)
    .height(Length::Fill)
    .padding(0)
    .on_press(message)
    .style(theme::window_button(close))
    .into()
}
