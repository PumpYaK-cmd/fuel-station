use iced::widget::canvas::{self, Cache, Frame, Geometry, Path, Stroke, gradient};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};

use super::theme;

const CELL: f32 = 40.0;
const SWEEP_HEIGHT: f32 = 200.0;
/// Seconds for one sweep to cross the window.
const SWEEP_PERIOD: f32 = 9.0;
pub const BRACKET: f32 = 12.0;

pub struct Backdrop<'a> {
    pub t: f32,
    pub grid: &'a Cache,
}

impl<Message> canvas::Program<Message> for Backdrop<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let size = bounds.size();
        let grid = self.grid.draw(renderer, size, |frame| {
            frame.fill_rectangle(Point::ORIGIN, size, theme::BACKGROUND);
            draw_grid(frame, size);
        });

        let mut sweep = Frame::new(renderer, size);
        let y =
            (self.t % SWEEP_PERIOD) / SWEEP_PERIOD * (size.height + SWEEP_HEIGHT) - SWEEP_HEIGHT;
        let fill = gradient::Linear::new(Point::new(0.0, y), Point::new(0.0, y + SWEEP_HEIGHT))
            .add_stop(0.0, Color::TRANSPARENT)
            .add_stop(0.5, theme::SWEEP)
            .add_stop(1.0, Color::TRANSPARENT);
        sweep.fill_rectangle(
            Point::new(0.0, y),
            Size::new(size.width, SWEEP_HEIGHT),
            fill,
        );
        vec![grid, sweep.into_geometry()]
    }
}

fn draw_grid(frame: &mut Frame, size: Size) {
    let center = Point::new(size.width * 0.5, size.height * 0.4);
    let reach = (size.width * size.width + size.height * size.height).sqrt() / 2.0;
    let mut segment = |a: Point, b: Point| {
        let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let d = mid.distance(center) / reach;
        let alpha = 1.0 - ((d - 0.3) / 0.5).clamp(0.0, 1.0);
        if alpha > 0.0 {
            frame.stroke(
                &Path::line(a, b),
                Stroke::default()
                    .with_width(1.0)
                    .with_color(theme::BAR_GRID.scale_alpha(alpha)),
            );
        }
    };
    let mut x = 0.0;
    while x <= size.width {
        let mut y = 0.0;
        while y <= size.height {
            segment(Point::new(x, y), Point::new(x + CELL, y));
            segment(Point::new(x, y), Point::new(x, y + CELL));
            y += CELL;
        }
        x += CELL;
    }
}

pub struct Brackets {
    pub color: Color,
    pub alpha: f32,
    pub len: f32,
}

impl<Message> canvas::Program<Message> for Brackets {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width - 0.5, bounds.height - 0.5);
        let corners = [
            (Point::new(0.5, 0.5), 1.0, 1.0),
            (Point::new(w, 0.5), -1.0, 1.0),
            (Point::new(0.5, h), 1.0, -1.0),
            (Point::new(w, h), -1.0, -1.0),
        ];
        let path = Path::new(|p| {
            for (corner, dx, dy) in corners {
                p.move_to(Point::new(corner.x + dx * self.len, corner.y));
                p.line_to(corner);
                p.line_to(Point::new(corner.x, corner.y + dy * self.len));
            }
        });
        frame.stroke(
            &path,
            Stroke::default()
                .with_width(1.0)
                .with_color(self.color.scale_alpha(self.alpha)),
        );
        vec![frame.into_geometry()]
    }
}
