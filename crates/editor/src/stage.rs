// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

#[cfg(test)]
mod tests;

use derive_setters::Setters;
use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{self, Node},
        overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse, touch,
};

/// Responsively generates rows and columns of widgets based on its dimmensions.
#[must_use]
#[derive(Setters)]
pub struct Stage<'a, Message, Theme, Renderer> {
    #[setters(skip)]
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    /// Where children shall be positioned.
    #[setters(skip)]
    positions: Vec<Point>,
    width: Length,
    height: Length,

    #[setters(skip)]
    on_press: Option<Box<dyn Fn(Point) -> Message + 'a>>,
    #[setters(strip_option)]
    interaction: Option<mouse::Interaction>,
}

impl<Message, Theme, Renderer> Default for Stage<'_, Message, Theme, Renderer> {
    fn default() -> Self {
        Self::new()
    }
}

type StageItem<'a, Message, Theme, Renderer> = (Element<'a, Message, Theme, Renderer>, Point);

impl<'a, Message, Theme, Renderer> Stage<'a, Message, Theme, Renderer> {
    pub const fn new() -> Self {
        Self {
            children: Vec::new(),
            positions: Vec::new(),
            width: Length::Shrink,
            height: Length::Shrink,

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

impl<'a, Message, Theme, Renderer> FromIterator<StageItem<'a, Message, Theme, Renderer>>
    for Stage<'a, Message, Theme, Renderer>
{
    fn from_iter<T: IntoIterator<Item = StageItem<'a, Message, Theme, Renderer>>>(iter: T) -> Self {
        Self::with_children(iter)
    }
}

impl<Message: 'static + Clone, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Stage<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
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
        let mut bounds = Rectangle::with_size(Size::ZERO);
        let children = self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(&self.positions)
            .map(|((child, tree), pos)| {
                let c_layout = child.as_widget_mut().layout(tree, renderer, limits).move_to(*pos);
                bounds = bounds.union(&c_layout.bounds());
                c_layout
            })
            .collect();
        Node::with_children(bounds.size(), children)
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

        if let (
            Some(on_press),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }),
        ) = (self.on_press.as_ref(), event)
        {
            let Some(position) = cursor.position_in(layout.bounds()) else { return };
            shell.publish((on_press)(position));
            shell.capture_event();
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
        for ((child, state), layout) in self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .filter(|(_, layout)| layout.bounds().intersects(&viewport))
        {
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

impl<'a, Message, Theme, Renderer> From<Stage<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'static,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(flex_row: Stage<'a, Message, Theme, Renderer>) -> Self {
        Self::new(flex_row)
    }
}
