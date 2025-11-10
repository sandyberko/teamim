//! Zoom and pan on an image.
#[cfg(test)]
mod tests;

use std::{borrow::Cow, iter};

use iced::{
    Color, ContentFit, Element, Event, Length, Pixels, Point, Radians, Rectangle, Size,
    Transformation, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        image::{self, FilterMethod, Image},
        layout, mouse, renderer,
        text::{self, Alignment, LineHeight, Paragraph, Shaping, paragraph},
        widget::tree::{self, Tree},
    },
    alignment::Vertical,
    border,
};

#[derive(Debug, Clone)]
pub struct Overlay<Handle> {
    position: Point,
    kind: OverlayKind<Handle>,
}

#[derive(Debug, Clone)]
pub enum OverlayKind<Handle> {
    Image(Handle),
    Text(Cow<'static, str>),
}

impl<Handle> Overlay<Handle> {
    pub fn image(position: Point, handle: Handle) -> Self {
        Self { position, kind: OverlayKind::Image(handle) }
    }
    pub fn text(position: Point, text: Cow<'static, str>) -> Self {
        Self { position, kind: OverlayKind::Text(text) }
    }
}

/// A frame that displays an image with the ability to zoom in/out and pan.
pub struct Stage<Message, Renderer>
where
    Renderer: image::Renderer + text::Renderer,
{
    padding: f32,
    width: Length,
    height: Length,
    min_scale: f32,
    max_scale: f32,
    scale_step: f32,
    handle: Option<Renderer::Handle>,
    filter_method: FilterMethod,
    content_fit: ContentFit,

    on_repos: Option<Box<dyn Fn(usize, Point) -> Message>>,

    font: Option<Renderer::Font>,
    overlays: Box<[Overlay<Renderer::Handle>]>,
}

impl<Message, Renderer> Stage<Message, Renderer>
where
    Renderer: image::Renderer + text::Renderer,
{
    /// Creates a new [`Viewer`] with the given [`State`].
    pub fn new(overlays: Box<[Overlay<Renderer::Handle>]>) -> Self {
        Stage {
            handle: None,
            padding: 0.0,
            width: Length::Shrink,
            height: Length::Shrink,

            min_scale: 0.25,
            max_scale: 10.0,
            scale_step: 0.10,
            filter_method: FilterMethod::default(),
            content_fit: ContentFit::default(),

            on_repos: None,

            font: None,
            overlays,
        }
    }

    /// Sets the [`FilterMethod`] of the [`Viewer`].
    pub fn filter_method(mut self, filter_method: image::FilterMethod) -> Self {
        self.filter_method = filter_method;
        self
    }

    /// Sets the [`ContentFit`] of the [`Viewer`].
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    /// Sets the padding of the [`Viewer`].
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = padding.into().0;
        self
    }

    /// Sets the width of the [`Viewer`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Viewer`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the max scale applied to the image of the [`Viewer`].
    ///
    /// Default is `10.0`
    pub fn max_scale(mut self, max_scale: f32) -> Self {
        self.max_scale = max_scale;
        self
    }

    /// Sets the min scale applied to the image of the [`Viewer`].
    ///
    /// Default is `0.25`
    pub fn min_scale(mut self, min_scale: f32) -> Self {
        self.min_scale = min_scale;
        self
    }

    /// Sets the percentage the image of the [`Viewer`] will be scaled by
    /// when zoomed in / out.
    ///
    /// Default is `0.10`
    pub fn scale_step(mut self, scale_step: f32) -> Self {
        self.scale_step = scale_step;
        self
    }

    pub fn handle(mut self, handle: Renderer::Handle) -> Self {
        self.handle = Some(handle);
        self
    }

    pub fn on_repos(mut self, on_repos: impl Fn(usize, Point) -> Message + 'static) -> Self {
        self.on_repos = Some(Box::new(on_repos));
        self
    }

    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.font = Some(font);
        self
    }
}

const MISS_CHAR_COUNT: f32 = 20.0;

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Stage<Message, Renderer>
where
    Renderer: image::Renderer + text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        let paragraphs = self
            .overlays
            .iter()
            .map(|overlay| {
                if let Some(font) = self.font
                    && let OverlayKind::Text(content) = &overlay.kind
                {
                    // TODO customize
                    let font_size = 32.0;
                    let line_height = font_size * 1.5;
                    Renderer::Paragraph::with_text(text::Text {
                        content,
                        bounds: Size::new(font_size * MISS_CHAR_COUNT, line_height),
                        size: font_size.into(),
                        line_height: LineHeight::Absolute(line_height.into()),
                        font,
                        align_x: Alignment::Right,
                        align_y: Vertical::Top,
                        shaping: Shaping::Advanced,
                        wrapping: text::Wrapping::None,
                    })
                } else {
                    Renderer::Paragraph::default()
                }
            })
            .collect();
        let state = State {
            scale: 1.0,
            starting_offset: Vector::default(),
            current_offset: Vector::default(),
            cursor_grabbed_at: None,

            repos: None,

            paragraphs,
        };
        tree::State::new(state)
    }

    fn size(&self) -> Size<Length> {
        Size { width: self.width, height: self.height }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();

        // The raw w/h of the underlying image
        let image_size =
            renderer.measure_image(self.handle.as_ref().expect("image")).unwrap_or_default();

        let image_size = Size::new(image_size.width as f32, image_size.height as f32);

        // The size to be available to the widget prior to `Shrink`ing
        let raw_size = limits.resolve(self.width, self.height, image_size);

        // The uncropped size of the image when fit to the bounds above
        let full_size = self.content_fit.fit(image_size, raw_size);

        // Shrink the widget to fit the resized image, if requested
        let final_size = Size {
            width: match self.width {
                Length::Shrink => f32::min(raw_size.width, full_size.width),
                _ => raw_size.width,
            },
            height: match self.height {
                Length::Shrink => f32::min(raw_size.height, full_size.height),
                _ => raw_size.height,
            },
        };

        layout::Node::new(final_size)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return;
                };
                let (mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. }) =
                    *delta;
                let State { scale, current_offset, .. } =
                    tree.state.downcast_mut::<State<Renderer::Paragraph>>();

                let factor = (1.0 + self.scale_step).powf(y);
                *scale *= factor;
                *current_offset = cursor_position
                    - (cursor_position - *current_offset) * Transformation::scale(factor);

                shell.request_redraw();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return;
                };
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
                let scale = Transformation::scale(state.scale);

                if let Some(on_repos) = &self.on_repos {
                    let stage_offset = bounds.position() - Point::ORIGIN;

                    if let Some((diac_idx, diac_offset)) = state.repos.take() {
                        let message =
                            on_repos(diac_idx, cursor_position - diac_offset - stage_offset);
                        shell.publish(message);
                        shell.capture_event();
                        return;
                    }

                    let over_diac = self.overlays.iter().enumerate().find_map(|(idx, overlay)| {
                        #[expect(clippy::cast_precision_loss)]
                        let size = match &overlay.kind {
                            OverlayKind::Image(handle) => {
                                let size = renderer.measure_image(handle)?;
                                Size::new(size.width as f32, size.height as f32) * scale
                            }
                            OverlayKind::Text(_) => state.paragraphs[idx].min_bounds(),
                        };
                        let diac_bounds =
                            Rectangle::new(overlay.position * scale + stage_offset, size);
                        if !diac_bounds.expand(3.0).contains(cursor_position) {
                            return None;
                        }
                        Some((idx, cursor_position - diac_bounds.position()))
                    });

                    if over_diac.is_some() {
                        state.repos = over_diac;
                        shell.request_redraw();
                        shell.capture_event();
                        return;
                    }
                }

                state.cursor_grabbed_at = Some(cursor_position);
                state.starting_offset = state.current_offset;

                shell.request_redraw();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

                if state.cursor_grabbed_at.is_some() {
                    state.cursor_grabbed_at = None;
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

                if let Some(origin) = state.cursor_grabbed_at {
                    let scaled_size = scaled_image_size(
                        renderer,
                        self.handle.as_ref().expect("image"),
                        state,
                        bounds.size(),
                        self.content_fit,
                    );
                    let hidden_width = (scaled_size.width - bounds.width / 2.0).max(0.0).round();

                    let hidden_height = (scaled_size.height - bounds.height / 2.0).max(0.0).round();

                    let delta = *position - origin;

                    let x = if bounds.width < scaled_size.width {
                        (state.starting_offset.x - delta.x).clamp(-hidden_width, hidden_width)
                    } else {
                        0.0
                    };

                    let y = if bounds.height < scaled_size.height {
                        (state.starting_offset.y - delta.y).clamp(-hidden_height, hidden_height)
                    } else {
                        0.0
                    };

                    state.current_offset = Vector::new(x, y);
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();
        let is_mouse_over = cursor.is_over(bounds);

        if state.is_cursor_grabbed() {
            mouse::Interaction::Grabbing
        } else if is_mouse_over {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let stage_bounds = layout.bounds();

        let handle = self.handle.as_ref().expect("image");
        let Size { width, height } = renderer.measure_image(handle).unwrap_or_default();
        #[expect(clippy::cast_precision_loss)]
        let orig_size = Size::new(width as f32, height as f32);
        let img_bounds =
            Rectangle::new(stage_bounds.position() + state.current_offset, orig_size * state.scale);

        let render = |renderer: &mut Renderer| {
            // background
            renderer.draw_image(
                Image {
                    handle: self.handle.clone().expect("image"),
                    border_radius: border::Radius::default(),
                    filter_method: self.filter_method,
                    rotation: Radians(0.0),
                    opacity: 1.0,
                    snap: true,
                },
                img_bounds,
                *viewport,
            );

            // overlays
            for (idx, (overlay, paragraph)) in
                self.overlays.iter().zip(&state.paragraphs).enumerate()
            {
                let overlay_offset = overlay.position - Point::ORIGIN;
                let overlay_position = img_bounds.position() + overlay_offset * state.scale;

                match &overlay.kind {
                    OverlayKind::Text(_) => {
                        renderer.fill_paragraph(
                            paragraph,
                            overlay_position,
                            Color::from_rgb8(0xff, 0, 0),
                            stage_bounds,
                        );
                    }
                    OverlayKind::Image(handle) => {
                        let Size { width, height } =
                            renderer.measure_image(handle).unwrap_or_default();
                        #[expect(clippy::cast_precision_loss)]
                        let orig_size = Size::new(width as f32, height as f32);
                        let bounds = Rectangle::new(overlay_position, orig_size * state.scale);

                        let opacity = if let Some((repos_idx, _)) = state.repos
                            && repos_idx == idx
                        {
                            0.5
                        } else {
                            1.0
                        };
                        renderer.draw_image(
                            Image {
                                handle: handle.clone(),
                                border_radius: border::Radius::default(),
                                filter_method: self.filter_method,
                                rotation: Radians(0.0),
                                opacity,
                                snap: true,
                            },
                            bounds,
                            *viewport,
                        );
                    }
                }
            }
        };

        renderer.with_layer(img_bounds, render);
    }
}

/// The local state of a [`Viewer`].
#[derive(Debug, Clone)]
pub struct State<Paragraph> {
    scale: f32,
    starting_offset: Vector,
    current_offset: Vector,
    cursor_grabbed_at: Option<Point>,

    repos: Option<(usize, Vector)>,
    paragraphs: Box<[Paragraph]>,
}

impl<Paragraph> State<Paragraph> {
    /// Returns the current offset of the [`State`], given the bounds
    /// of the [`Viewer`] and its image.
    fn offset(&self, bounds: Rectangle, image_size: Size) -> Vector {
        let hidden_width = (image_size.width - bounds.width / 2.0).max(0.0).round();

        let hidden_height = (image_size.height - bounds.height / 2.0).max(0.0).round();

        Vector::new(
            self.current_offset.x.clamp(-hidden_width, hidden_width),
            self.current_offset.y.clamp(-hidden_height, hidden_height),
        )
    }

    /// Returns if the cursor is currently grabbed by the [`Viewer`].
    pub fn is_cursor_grabbed(&self) -> bool {
        self.cursor_grabbed_at.is_some()
    }
}

impl<'a, Message, Theme, Renderer> From<Stage<Message, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + image::Renderer + text::Renderer,
    Message: 'a,
{
    fn from(viewer: Stage<Message, Renderer>) -> Element<'a, Message, Theme, Renderer> {
        Element::new(viewer)
    }
}

/// Returns the bounds of the underlying image, given the bounds of
/// the [`Viewer`]. Scaling will be applied and original aspect ratio
/// will be respected.
pub fn scaled_image_size<Renderer>(
    renderer: &Renderer,
    handle: &<Renderer as image::Renderer>::Handle,
    state: &State<Renderer::Paragraph>,
    bounds: Size,
    content_fit: ContentFit,
) -> Size
where
    Renderer: image::Renderer + text::Renderer,
{
    let Size { width, height } = renderer.measure_image(handle).unwrap_or_default();

    let image_size = Size::new(width as f32, height as f32);

    let adjusted_fit = content_fit.fit(image_size, bounds);

    Size::new(adjusted_fit.width * state.scale, adjusted_fit.height * state.scale)
}
