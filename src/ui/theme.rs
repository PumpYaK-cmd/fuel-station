use iced::theme::Palette;
use iced::widget::{self, container, pick_list, progress_bar, scrollable, text_editor, text_input};
use iced::{Border, Color, Font, Shadow, Theme, Vector, color};

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

pub const SURFACE_GLASS: Color = Color { a: 0.93, ..SURFACE };
pub const SURFACE_GLASS_LIT: Color = Color {
    a: 0.93,
    ..color!(0x15202D)
};
pub const SIDEBAR_GLASS: Color = Color { a: 0.80, ..SURFACE };
pub const BAR_GRID: Color = Color {
    a: 0.5,
    ..BORDER_STRONG
};
pub const SWEEP: Color = Color { a: 0.08, ..PRIMARY };

/// Cards at which a column gauge reads full.
pub const COLUMN_CAP: f32 = 20.0;

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
        let color = match (selected, status) {
            (_, widget::button::Status::Disabled) => TEXT_MUTED,
            (true, _) => ACCENT,
            (false, _) => PRIMARY,
        };
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

pub fn glass(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(SURFACE_GLASS)
        .border(Border {
            color: Color::TRANSPARENT,
            width: 1.0,
            radius: 2.0.into(),
        })
}

pub fn glass_lit(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(SURFACE_GLASS_LIT.into()),
        ..glass(theme)
    }
}

pub fn sidebar(_theme: &Theme) -> container::Style {
    container::Style::default().background(SIDEBAR_GLASS)
}

pub fn bar(_theme: &Theme) -> container::Style {
    container::Style::default().background(SURFACE)
}

pub fn card(border: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme| {
        container::Style::default()
            .background(BACKGROUND)
            .border(Border {
                color: border,
                width: 1.0,
                radius: 2.0.into(),
            })
    }
}

pub fn placeholder(theme: &Theme) -> container::Style {
    card(PRIMARY.scale_alpha(0.4))(theme).color(TEXT_MUTED)
}

pub fn ghost(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(BACKGROUND.scale_alpha(0.92).into()),
        shadow: Shadow {
            color: Color::BLACK.scale_alpha(0.5),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 18.0,
        },
        ..card(PRIMARY)(theme)
    }
}

pub fn drop_slot(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(ACCENT.scale_alpha(0.08))
        .border(Border {
            color: ACCENT.scale_alpha(0.6),
            width: 1.0,
            radius: 2.0.into(),
        })
}

pub fn fill(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style::default().background(color)
}

pub fn gauge(color: Color) -> impl Fn(&Theme) -> progress_bar::Style {
    move |_theme| progress_bar::Style {
        background: BORDER.into(),
        bar: color.into(),
        border: Border::default(),
    }
}

pub fn window_button(
    close: bool,
) -> impl Fn(&Theme, widget::button::Status) -> widget::button::Style {
    move |_theme, status| {
        let hovered = matches!(
            status,
            widget::button::Status::Hovered | widget::button::Status::Pressed
        );
        let (background, text_color) = match (hovered, close) {
            (false, _) => (None, TEXT_MUTED),
            (true, false) => (Some(SURFACE_HOVER.into()), TEXT),
            (true, true) => (Some(DANGER.into()), Color::WHITE),
        };
        widget::button::Style {
            background,
            text_color,
            ..widget::button::Style::default()
        }
    }
}

pub fn thin() -> scrollable::Direction {
    scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(4)
            .scroller_width(4)
            .spacing(4),
    )
}

pub fn scrollbar(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let active = matches!(
        status,
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        }
    );
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: if active { PRIMARY } else { BORDER_STRONG }.into(),
            border: Border::default(),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        ..scrollable::default(theme, status)
    }
}

fn field_border(focused: bool) -> Border {
    Border {
        color: if focused { PRIMARY } else { BORDER },
        width: 1.0,
        radius: 2.0.into(),
    }
}

const SELECTION: Color = Color { a: 0.3, ..PRIMARY };

pub fn input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: BACKGROUND.into(),
        border: field_border(matches!(status, text_input::Status::Focused { .. })),
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: TEXT,
        selection: SELECTION,
    }
}

pub fn editor(_theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        background: BACKGROUND.into(),
        border: field_border(matches!(status, text_editor::Status::Focused { .. })),
        placeholder: TEXT_MUTED,
        value: TEXT,
        selection: SELECTION,
    }
}

pub fn picker(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: TEXT,
        placeholder_color: TEXT_MUTED,
        handle_color: PRIMARY,
        background: BACKGROUND.into(),
        border: field_border(matches!(status, pick_list::Status::Opened { .. })),
    }
}
