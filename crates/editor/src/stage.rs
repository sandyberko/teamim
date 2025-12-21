//! Zoom and pan on fixed-position images and text.
#[cfg(test)]
mod tests;

use {
    iced::{
        Color, ContentFit, Element, Event, Length, Pixels, Point, Radians, Rectangle, Size,
        Transformation, Vector,
        advanced::{
            Clipboard, Layout, Shell, Widget,
            image::{self, FilterMethod, Image},
            layout, mouse, renderer,
            text::{self, Alignment, LineHeight, Paragraph, Shaping},
            widget::tree::{self, Tree},
        },
        alignment::Vertical,
        border::{self},
        keyboard,
        mouse::ScrollDelta,
    },
    std::{borrow::Cow, iter},
    tap::prelude::*,
};

#[derive(Debug, Clone)]
pub struct Overlay<Handle> {
    position: Point,
    kind: OverlayKind<Handle>,
}

impl<Handle> Overlay<Handle> {
    pub fn image(position: impl Into<Point>, handle: Handle) -> Self {
        Self { position: position.into(), kind: OverlayKind::Image(handle) }
    }

    #[must_use]
    pub fn text(position: Point, text: Cow<'static, str>) -> Self {
        Self { position, kind: OverlayKind::Text(text) }
    }

    fn bounds<Renderer>(
        &self,
        state: &State<Renderer::Paragraph>,
        layout: Layout<'_>,
        renderer: &Renderer,
        idx: usize,
    ) -> Rectangle
    where
        Renderer: image::Renderer<Handle = Handle> + text::Renderer,
    {
        let scale = Transformation::scale(state.scale);
        let stage_offset = layout.bounds().position() + state.offset - Point::ORIGIN;

        #[expect(clippy::cast_precision_loss)]
        let size = match &self.kind {
            OverlayKind::Image(handle) => {
                let size = renderer.measure_image(handle).unwrap_or_default();
                Size::new(size.width as f32, size.height as f32) * scale
            }
            OverlayKind::Text(_) => state.paragraphs[idx].min_bounds(),
        };
        Rectangle::new(self.position * scale + stage_offset, size)
    }
}

#[derive(Debug, Clone)]
pub enum OverlayKind<Handle> {
    Image(Handle),
    Text(Cow<'static, str>),
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

    on_move: Option<Box<dyn Fn(usize, Point) -> Message>>,

    font: Option<Renderer::Font>,
    font_size: Option<Pixels>,
    overlays: Box<[Overlay<Renderer::Handle>]>,
}

impl<Message, Renderer> Stage<Message, Renderer>
where
    Renderer: image::Renderer + text::Renderer,
{
    /// Creates a new [`Viewer`] with the given [`State`].
    #[must_use]
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

            on_move: None,

            font: None,
            font_size: None,
            overlays,
        }
    }

    /// Sets the [`FilterMethod`] of the [`Viewer`].
    #[must_use]
    pub fn filter_method(mut self, filter_method: image::FilterMethod) -> Self {
        self.filter_method = filter_method;
        self
    }

    /// Sets the [`ContentFit`] of the [`Viewer`].
    #[must_use]
    pub fn content_fit(mut self, content_fit: ContentFit) -> Self {
        self.content_fit = content_fit;
        self
    }

    /// Sets the padding of the [`Viewer`].
    #[must_use]
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = padding.into().0;
        self
    }

    /// Sets the width of the [`Viewer`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Viewer`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the max scale applied to the image of the [`Viewer`].
    ///
    /// Default is `10.0`
    #[must_use]
    pub fn max_scale(mut self, max_scale: f32) -> Self {
        self.max_scale = max_scale;
        self
    }

    /// Sets the min scale applied to the image of the [`Viewer`].
    ///
    /// Default is `0.25`
    #[must_use]
    pub fn min_scale(mut self, min_scale: f32) -> Self {
        self.min_scale = min_scale;
        self
    }

    /// Sets the percentage the image of the [`Viewer`] will be scaled by
    /// when zoomed in / out.
    ///
    /// Default is `0.10`
    #[must_use]
    pub fn scale_step(mut self, scale_step: f32) -> Self {
        self.scale_step = scale_step;
        self
    }

    #[must_use]
    pub fn handle(mut self, handle: Renderer::Handle) -> Self {
        self.handle = Some(handle);
        self
    }

    #[must_use]
    pub fn on_move(mut self, on_move: impl Fn(usize, Point) -> Message + 'static) -> Self {
        self.on_move = Some(Box::new(on_move));
        self
    }

    #[must_use]
    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.font = Some(font);
        self
    }

    #[must_use]
    pub fn font_size(mut self, font_size: impl Into<Pixels>) -> Self {
        self.font_size = Some(font_size.into());
        self
    }

    fn hit_overlay(
        &self,
        state: &State<Renderer::Paragraph>,
        layout: Layout<'_>,
        point: Point,
        renderer: &Renderer,
    ) -> Option<(usize, Vector)> {
        self.overlays.iter().enumerate().find_map(|(idx, overlay)| {
            let overlay_bounds = overlay.bounds(state, layout, renderer, idx);
            if !overlay_bounds.expand(3.0).contains(point) {
                return None;
            }
            Some((idx, point - overlay_bounds.position()))
        })
    }
}

const SCROLL_FACTOR: f32 = -10.0;

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Stage<Message, Renderer>
where
    Renderer: image::Renderer + text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        let state = State::<Renderer::Paragraph> {
            scale: 1.0,
            offset: Vector::default(),

            move_element: None,

            paragraphs: Box::default(),

            ctrl_pressed: false,
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
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        let image_size =
            renderer.measure_image(self.handle.as_ref().expect("image")).unwrap_or_default();

        #[expect(clippy::cast_precision_loss)]
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

        // text
        if state.paragraphs.is_empty() {
            state.paragraphs = self
                .overlays
                .iter()
                .map(|overlay| {
                    let OverlayKind::Text(content) = &overlay.kind else {
                        return Renderer::Paragraph::default();
                    };

                    let text = text::Text::<&str, _> {
                        content,
                        bounds: limits.width(Length::Shrink).height(Length::Shrink).max(),
                        // TODO customize
                        size: self.font_size.unwrap_or(renderer.default_size()),
                        line_height: LineHeight::Relative(1.5),
                        font: self.font.unwrap_or(renderer.default_font()),
                        align_x: Alignment::Right,
                        align_y: Vertical::Top,
                        shaping: Shaping::Advanced,
                        wrapping: text::Wrapping::None,
                    };
                    // HACK how do I measure text?
                    let measure_paragraph = Renderer::Paragraph::with_text(text);

                    Paragraph::with_text(text::Text {
                        bounds: measure_paragraph.min_bounds(),
                        ..text
                    })
                })
                .collect();
        }

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
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        match event {
            // detect ctrl for zoom
            Event::Keyboard(keyboard::Event::ModifiersChanged(mods)) => {
                state.ctrl_pressed = mods.control();
            }
            // zoom
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if state.ctrl_pressed => {
                let Some(cursor_position) = cursor.position_over(bounds) else {
                    return;
                };
                let delta = {
                    let (mouse::ScrollDelta::Lines { y, .. }
                    | mouse::ScrollDelta::Pixels { y, .. }) = *delta;
                    y * self.scale_step
                };
                let State { scale, offset: current_offset, .. } = state;

                let factor = (1.0 + self.scale_step).powf(delta);
                *scale *= factor;
                *current_offset = cursor_position
                    - (cursor_position - *current_offset) * Transformation::scale(factor);

                shell.request_redraw();
                shell.capture_event();
            }
            // scroll
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if !state.ctrl_pressed => {
                let delta = {
                    let (ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y }) = delta;
                    [x, y].map(|coord| coord * SCROLL_FACTOR).conv::<Vector>()
                };

                // TODO
                // let scaled_size = scaled_image_size(
                //     renderer,
                //     self.handle.as_ref().expect("image"),
                //     state,
                //     bounds.size(),
                //     self.content_fit,
                // );
                // let hidden_width = (scaled_size.width - bounds.width / 2.0).max(0.0).round();

                // let hidden_height = (scaled_size.height - bounds.height / 2.0).max(0.0).round();

                // let x = if bounds.width < scaled_size.width {
                //     (state.offset.x - delta.x).clamp(-hidden_width, hidden_width)
                // } else {
                //     0.0
                // };

                // let y = if bounds.height < scaled_size.height {
                //     (state.offset.y - delta.y).clamp(-hidden_height, hidden_height)
                // } else {
                //     0.0
                // };

                state.offset += delta;
                shell.request_redraw();
                shell.capture_event();
            }
            // initiate/commit element move
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(on_repos) = &self.on_move else { return };
                let Some(cursor_position) = cursor.position_over(bounds) else { return };

                // commit move
                if let Some(MoveElement { elem_idx, press_elem_offset, .. }) =
                    state.move_element.take()
                {
                    let stage_offset = bounds.position() - Point::ORIGIN;
                    let message = on_repos(
                        elem_idx,
                        (cursor_position - press_elem_offset - state.offset - stage_offset)
                            * Transformation::scale(state.scale).inverse(),
                    );
                    shell.publish(message);
                    shell.capture_event();
                    return;
                }

                // initiate move
                if let Some((elem_idx, press_elem_offset)) =
                    self.hit_overlay(state, layout, cursor_position, renderer)
                {
                    state.move_element = Some(MoveElement { elem_idx, press_elem_offset });
                    shell.request_redraw();
                    shell.capture_event();
                }
            }

            // cursor position for moving diac
            Event::Mouse(mouse::Event::CursorMoved { position })
                if state.move_element.is_some() && bounds.contains(*position) =>
            {
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::CursorLeft) if state.move_element.is_some() => {
                shell.request_redraw();
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
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();
        let is_mouse_over = cursor.is_over(bounds);

        if state.move_element.is_some() {
            mouse::Interaction::Crosshair
        } else if is_mouse_over {
            if let Some(pos) = cursor.position()
                && self.hit_overlay(tree.state.downcast_ref(), layout, pos, renderer).is_some()
            {
                mouse::Interaction::Move
            } else {
                mouse::Interaction::Grab
            }
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
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();

        let background_item = self.handle.as_ref().map(|handle| {
            (Overlay::image([0.0; _], handle.clone()), Renderer::Paragraph::default())
        });
        let items = iter::chain(
            background_item.as_ref().map(|(handle, paragraph)| (handle, paragraph)),
            self.overlays.iter().zip(&state.paragraphs),
        );

        for (idx, (overlay, paragraph)) in items.enumerate() {
            let mut overlay_bounds = overlay.bounds(state, layout, renderer, idx);
            let opacity = if let Some(mov) = state.move_element
                && mov.elem_idx + 1 == idx
            {
                if let Some(cursor_position) = cursor.position() {
                    let Point { x, y } = cursor_position - mov.press_elem_offset;
                    overlay_bounds.x = x;
                    overlay_bounds.y = y;
                }
                0.5
            } else {
                1.0
            };

            match &overlay.kind {
                OverlayKind::Text(_) => {
                    renderer.fill_paragraph(
                        paragraph,
                        overlay_bounds.position(),
                        Color::from_rgba(1.0, 0.0, 0.0, opacity),
                        *viewport,
                    );
                }
                OverlayKind::Image(handle) => {
                    renderer.draw_image(
                        Image {
                            handle: handle.clone(),
                            border_radius: border::Radius::default(),
                            filter_method: self.filter_method,
                            rotation: Radians(0.0),
                            opacity,
                            snap: true,
                        },
                        overlay_bounds,
                        *viewport,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MoveElement {
    elem_idx: usize,
    press_elem_offset: Vector,
}

/// The local state of a [`Viewer`].
#[derive(Debug, Clone)]
pub struct State<Paragraph> {
    scale: f32,
    offset: Vector,

    move_element: Option<MoveElement>,
    paragraphs: Box<[Paragraph]>,

    ctrl_pressed: bool,
}

impl<Paragraph> State<Paragraph> {
    /// Returns the current offset of the [`State`], given the bounds
    /// of the [`Viewer`] and its image.
    fn offset(&self, bounds: Rectangle, image_size: Size) -> Vector {
        let hidden_width = (image_size.width - bounds.width / 2.0).max(0.0).round();

        let hidden_height = (image_size.height - bounds.height / 2.0).max(0.0).round();

        Vector::new(
            self.offset.x.clamp(-hidden_width, hidden_width),
            self.offset.y.clamp(-hidden_height, hidden_height),
        )
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
