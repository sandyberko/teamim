// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

#[cfg(test)]
mod tests;

use derive_setters::Setters;
use iced::{
    Element, Event, Length, Point, Radians, Rectangle, Size, Transformation, Vector,
    advanced::{
        self, Clipboard, Layout, Shell, Widget,
        layout::{self, Node},
        overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    border, keyboard,
    mouse::{self},
    touch,
    widget::image::FilterMethod,
};

struct State {
    ctrl_pressed: bool,
    scale: f32,
    starting_offset: Vector,
    current_offset: Vector,
    cursor_grabbed_at: Option<Point>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ctrl_pressed: Default::default(),
            scale: 1.0,
            starting_offset: Vector::default(),
            current_offset: Vector::default(),
            cursor_grabbed_at: Option::default(),
        }
    }
}

/// Responsively generates rows and columns of widgets based on its dimmensions.
#[must_use]
#[derive(Setters)]
pub struct Stage<'a, Message, Theme, Renderer, Handle> {
    #[setters(skip)]
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    /// Where children shall be positioned.
    #[setters(skip)]
    positions: Vec<Point>,
    width: Length,
    height: Length,

    // <viewer>
    min_scale: f32,
    max_scale: f32,
    scale_step: f32,
    #[setters(strip_option)]
    handle: Option<Handle>,
    // </viewer>
    //
    #[setters(skip)]
    on_press: Option<Box<dyn Fn(Point) -> Message + 'a>>,
    #[setters(strip_option)]
    interaction: Option<mouse::Interaction>,
}

type StageItem<'a, Message, Theme, Renderer> = (Element<'a, Message, Theme, Renderer>, Point);

impl<'a, Message, Theme, Renderer, Handle> Stage<'a, Message, Theme, Renderer, Handle> {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            positions: Vec::new(),
            width: Length::Shrink,
            height: Length::Shrink,

            min_scale: 0.25,
            max_scale: 10.0,
            scale_step: 0.10,
            handle: None,

            on_press: None,
            interaction: None,
        }
    }

    pub fn with_children(
        children: impl IntoIterator<Item = StageItem<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let (children, positions): (Vec<_>, Vec<_>) = children.into_iter().unzip();
        Self { children, positions, ..Self::new() }
    }

    /// Attach a new element with custom properties
    pub fn push<W>(mut self, widget: W, position: impl Into<Point>) -> Self
    where
        W: Into<Element<'a, Message, Theme, Renderer>>,
    {
        self.children.push(widget.into());

        self.positions.push(position.into());

        self
    }

    /// The message to emit on a left button press.
    pub fn on_press(mut self, on_press: impl Fn(Point) -> Message + 'a) -> Self {
        self.on_press = Some(Box::new(on_press));
        self
    }
}

impl<Message, Theme, Renderer> Stage<'_, Message, Theme, Renderer, Renderer::Handle>
where
    Renderer: advanced::image::Renderer,
{
    fn zoom(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
        y: f32,
    ) {
        let bounds = layout.bounds();
        let Some(cursor_position) = cursor.position_over(bounds) else {
            return;
        };

        let state = tree.state.downcast_mut::<State>();
        let previous_scale = state.scale;

        if y < 0.0 && previous_scale > self.min_scale || y > 0.0 && previous_scale < self.max_scale
        {
            state.scale = (if y > 0.0 {
                state.scale * (1.0 + self.scale_step)
            } else {
                state.scale / (1.0 + self.scale_step)
            })
            .clamp(self.min_scale, self.max_scale);

            let scaled_size = {
                let size = if let Some(handle) = &self.handle
                    && let Some(Size { width, height }) = renderer.measure_image(handle)
                {
                    #[expect(clippy::cast_precision_loss)]
                    Size::new(width as f32, height as f32)
                } else {
                    let width = self
                        .positions
                        .iter()
                        .map(|v| v.x)
                        .max_by(f32::total_cmp)
                        .unwrap_or_default();
                    let height = self
                        .positions
                        .iter()
                        .map(|v| v.y)
                        .max_by(f32::total_cmp)
                        .unwrap_or_default();
                    Size::new(width, height)
                };
                size * state.scale
            };

            let factor = state.scale / previous_scale - 1.0;

            let cursor_to_center = cursor_position - bounds.center();

            let adjustment = cursor_to_center * factor + state.current_offset * factor;

            state.current_offset = Vector::new(
                if scaled_size.width > bounds.width {
                    state.current_offset.x + adjustment.x
                } else {
                    0.0
                },
                if scaled_size.height > bounds.height {
                    state.current_offset.y + adjustment.y
                } else {
                    0.0
                },
            );
        }
    }

    fn size(&mut self, renderer: &Renderer, children_size: Size) -> Size {
        self.handle
            .as_ref()
            .and_then(|handle| {
                renderer
                    .measure_image(handle)
                    .map(|Size { width, height }| Size::new(width as f32, height as f32))
            })
            .unwrap_or(children_size)
    }
}
impl<Message: Clone, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Stage<'_, Message, Theme, Renderer, Renderer::Handle>
where
    Renderer: advanced::Renderer + advanced::image::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_ref::<State>();
        eprintln!("LAYOUT {}", state.scale);
        let scale = Transformation::scale(state.scale);

        let mut children_size = Size::ZERO;

        let children = self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(&self.positions)
            .map(|((child, tree), pos)| {
                let c_layout =
                    child.as_widget_mut().layout(tree, renderer, limits).move_to(*pos * scale);

                let Rectangle { x, y, width, height } = c_layout.bounds();
                children_size = children_size.max(Size::new(x + width, y + height));

                c_layout
            })
            .collect::<Vec<_>>();
        Node::with_children(self.size(renderer, children_size), children)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.children.iter_mut().zip(&mut tree.children).zip(layout.children()).for_each(
                |((child, state), layout)| {
                    child.as_widget_mut().operate(state, layout, renderer, operation);
                },
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for ((child, state), layout) in
            self.children.iter_mut().zip(&mut tree.children).zip(layout.children())
        {
            child
                .as_widget_mut()
                .update(state, event, layout, cursor, renderer, clipboard, shell, viewport);
        }

        if shell.is_event_captured() {
            return;
        }

        if !cursor.is_over(layout.bounds()) {
            return;
        }

        // press
        if let Some(on_press) = self.on_press.as_ref()
            && let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) = event
        {
            let Some(position) = cursor.position_in(layout.bounds()) else { return };
            shell.publish((on_press)(position));
            shell.capture_event();
        }

        // zoom
        if let Event::Keyboard(keyboard::Event::ModifiersChanged(mods)) = event {
            let state = tree.state.downcast_mut::<State>();
            state.ctrl_pressed = mods.control();
            eprintln!("CONTROL {}", state.ctrl_pressed);
        }
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            let state = tree.state.downcast_ref::<State>();
            eprintln!("SCROLL {}", state.ctrl_pressed);
            if state.ctrl_pressed {
                let (mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. }) =
                    *delta;
                self.zoom(tree, event, layout, cursor, renderer, clipboard, shell, viewport, y);
                shell.capture_event();
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let content_interaction = self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, state), layout)| {
                child.as_widget().mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default();

        if content_interaction != mouse::Interaction::None {
            return content_interaction;
        }

        if let Some(interaction) = self.interaction
            && cursor.is_over(layout.bounds())
        {
            return interaction;
        }

        mouse::Interaction::None
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Some(viewport) = layout.bounds().intersection(viewport) else { return };
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<State>();
        let scale = Transformation::scale(state.scale);

        // image
        if let Some(handle) = self.handle.clone()
            && let Some(img_size) = renderer.measure_image(&handle)
        {
            let image = advanced::image::Image {
                handle,
                filter_method: FilterMethod::default(),
                rotation: Radians(0.0),
                border_radius: border::Radius::default(),
                opacity: 1.0,
                snap: true,
            };
            #[expect(clippy::cast_precision_loss)]
            let img_size = Size::new(img_size.width as f32, img_size.height as f32);
            let bounds = Rectangle::new(bounds.position(), img_size * scale);
            renderer.draw_image(image, bounds, viewport);
        }

        // children
        for ((child, state), layout) in
            self.children.iter().zip(&tree.children).zip(layout.children())
        {
            let Some(viewport) = bounds.intersection(&viewport) else { continue };
            child.as_widget().draw(state, renderer, theme, style, layout, cursor, &viewport);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(&mut self.children, tree, layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<Stage<'a, Message, Theme, Renderer, Renderer::Handle>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: advanced::Renderer + advanced::image::Renderer + 'a,
{
    fn from(flex_row: Stage<'a, Message, Theme, Renderer, Renderer::Handle>) -> Self {
        Self::new(flex_row)
    }
}
