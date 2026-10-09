use iced::theme::Palette;
use iced::widget::{self, container};
use iced::{Border, Color, Font, Theme, color};

pub const BACKGROUND: Color = color!(0x0A0E14);
pub const SURFACE: Color = color!(0x0F1520);
pub const SURFACE_HOVER: Color = color!(0x151D2B);
pub const BORDER: Color = color!(0x1C2636);
pub const BORDER_STRONG: Color = color!(0x2A3A52);
pub const TEXT: Color = color!(0xC8D3E6);
pub const TEXT_MUTED: Color = color!(0x5E6E88);
pub const PRIMARY: Color = color!(0x3D8BFF);
pub const ACCENT: Color = color!(0x6CC4FF);
pub const SUCCESS: Color = color!(0x2FD3A0);
pub const WARNING: Color = color!(0xF0B44C);
pub const DANGER: Color = color!(0xFF5C6C);

pub const FONT: Font = Font::with_name("JetBrainsMono Nerd Font");

pub fn theme() -> Theme {
    Theme::custom(
        "Station Night",
        Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: PRIMARY,
            success: SUCCESS,
            warning: WARNING,
            danger: DANGER,
        },
    )
}

pub fn button(selected: bool) -> impl Fn(&Theme, widget::button::Status) -> widget::button::Style {
    move |_theme, status| {
        let color = if selected { ACCENT } else { PRIMARY };
        widget::button::Style {
            background: (status == widget::button::Status::Hovered).then(|| SURFACE_HOVER.into()),
            text_color: color,
            border: Border {
                color,
                width: 1.0,
                radius: 2.0.into(),
            },
            ..widget::button::Style::default()
        }
    }
}

pub fn panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(SURFACE)
        .border(Border {
            color: BORDER,
            width: 1.0,
            radius: 2.0.into(),
        })
}

#[allow(dead_code)]
pub fn card_box(warning: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let style = panel(theme);
        if warning {
            container::Style {
                border: Border {
                    color: WARNING,
                    ..style.border
                },
                ..style
            }
        } else {
            style
        }
    }
}
