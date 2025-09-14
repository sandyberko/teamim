// spinner.rs
// A tiny, self-contained spinner widget based on Canvas + a time tick.
// Works with `iced` or `libcosmic` (which re-exports iced types).
use std::time::Instant;

use cosmic::{
    Element,
    iced::{
        Color, Length, Point, Rectangle, mouse,
        widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke},
    },
    iced_renderer::geometry,
    widget::canvas::path::Arc,
};

#[derive(Debug, Clone, Copy)]
pub struct Spinner {
    start: Instant,
    last_tick: Instant,
    size: f32,
    color: Color,
    stroke_width: f32,
    // how many radians the arc spans (e.g. 90° = FRAC_PI_2)
    arc_len: f32,
    // rotations per second
    speed_rps: f32,
}

impl Spinner {
    #[must_use]
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last_tick: now,
            size: 22.0,
            color: Color::from_rgb(0.2, 0.6, 1.0),
            stroke_width: 3.0,
            arc_len: std::f32::consts::FRAC_PI_2 * 1.5, // ~270°
            speed_rps: 1.5,
        }
    }

    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Called by your application each tick (from `time::every`) to advance the spinner.
    pub fn tick(&mut self, now: Instant) {
        self.last_tick = now;
    }
}

impl<'a, Message: 'a> From<Spinner> for Element<'a, Message> {
    fn from(val: Spinner) -> Self {
        // clone self into the Canvas program (cheap enough for this small widget)
        Canvas::new(val).width(Length::Fixed(val.size)).height(Length::Fixed(val.size)).into()
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message, Theme, Renderer> canvas::Program<Message, Theme, Renderer> for Spinner
where
    Renderer: geometry::Renderer,
{
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        // build a Frame every draw (spinner is tiny — this is fine)
        let mut frame = Frame::new(renderer, bounds.size());

        // compute rotation angle from tick timestamps
        let elapsed = self.last_tick.duration_since(self.start).as_secs_f32();
        let angle = (elapsed * self.speed_rps * std::f32::consts::TAU) % std::f32::consts::TAU;

        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius =
            (self.size.min(bounds.width).min(bounds.height) / 2.0) - (self.stroke_width / 2.0);

        let path = Path::new(|builder| {
            builder.arc(Arc {
                center,
                radius,
                start_angle: angle.into(),
                end_angle: (angle + self.arc_len).into(),
            });
        });
        frame.stroke(
            &path,
            Stroke::default()
                .with_width(self.stroke_width)
                .with_line_cap(canvas::LineCap::Round)
                .with_line_join(canvas::LineJoin::Round)
                .with_color(self.color),
        );

        vec![frame.into_geometry()]
    }
}
