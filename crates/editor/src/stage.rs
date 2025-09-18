// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

#[cfg(test)]
mod tests;

use cosmic::iced::Point;
use cosmic::iced_core;

use cosmic::iced_core::layout::Node;
use cosmic::{Element, Renderer};
use derive_setters::Setters;
use iced_core::event::{self, Event};
use iced_core::widget::{Operation, Tree};
use iced_core::{
    Clipboard, Layout, Length, Rectangle, Shell, Vector, Widget, layout, mouse, overlay, renderer,
};

/// Responsively generates rows and columns of widgets based on its dimmensions.
#[must_use]
#[derive(Setters)]
pub struct Stage<'a, Message> {
    #[setters(skip)]
    children: Vec<Element<'a, Message>>,
    /// Where children shall be positioned.
    #[setters(skip)]
    positions: Vec<Point>,
    /// Sets the width of the grid.
    width: Length,
    /// Sets the height of the grid.
    height: Length,
}

impl<Message> Default for Stage<'_, Message> {
    fn default() -> Self {
        Self::new()
    }
}

type StageItem<'a, Message> = (Element<'a, Message>, Point);

impl<'a, Message> Stage<'a, Message> {
    pub const fn new() -> Self {
        Self {
            children: Vec::new(),
            positions: Vec::new(),
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    pub fn with_children(children: impl IntoIterator<Item = StageItem<'a, Message>>) -> Self {
        let (children, positions): (Vec<_>, Vec<_>) = children.into_iter().unzip();
        Self { children, positions, ..Self::new() }
    }

    /// Attach a new element with custom properties
    pub fn push<W>(mut self, widget: W, position: Point) -> Self
    where
        W: Into<Element<'a, Message>>,
    {
        self.children.push(widget.into());

        self.positions.push(position);

        self
    }
}

impl<'a, Message> FromIterator<StageItem<'a, Message>> for Stage<'a, Message> {
    fn from_iter<T: IntoIterator<Item = StageItem<'a, Message>>>(iter: T) -> Self {
        Self::with_children(iter)
    }
}

impl<Message: 'static + Clone> Widget<Message, cosmic::Theme, Renderer> for Stage<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.children.as_mut_slice());
    }

    fn size(&self) -> iced_core::Size<Length> {
        iced_core::Size::new(self.width, self.height)
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        Node::with_children(
            // [TODO]
            (400., 2000.).into(),
            self.children
                .iter()
                .zip(&mut tree.children)
                .zip(&self.positions)
                .map(|((child, tree), pos)| {
                    child.as_widget().layout(tree, renderer, limits).move_to(*pos)
                })
                .collect(),
        )
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        operation.container(None, layout.bounds(), &mut |operation| {
            self.children.iter().zip(&mut tree.children).zip(layout.children()).for_each(
                |((child, state), c_layout)| {
                    child.as_widget().operate(
                        state,
                        c_layout.with_virtual_offset(layout.virtual_offset()),
                        renderer,
                        operation,
                    );
                },
            );
        });
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((child, state), c_layout)| {
                child.as_widget_mut().on_event(
                    state,
                    event.clone(),
                    c_layout.with_virtual_offset(layout.virtual_offset()),
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                )
            })
            .fold(event::Status::Ignored, event::Status::merge)
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, state), c_layout)| {
                child.as_widget().mouse_interaction(
                    state,
                    c_layout.with_virtual_offset(layout.virtual_offset()),
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &cosmic::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((child, state), c_layout) in
            self.children.iter().zip(&tree.children).zip(layout.children())
        {
            child.as_widget().draw(
                state,
                renderer,
                theme,
                style,
                c_layout.with_virtual_offset(layout.virtual_offset()),
                cursor,
                viewport,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, cosmic::Theme, Renderer>> {
        overlay::from_children(&mut self.children, tree, layout, renderer, translation)
    }

    fn drag_destinations(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        dnd_rectangles: &mut iced_core::clipboard::DndDestinationRectangles,
    ) {
        for ((e, c_layout), state) in
            self.children.iter().zip(layout.children()).zip(state.children.iter())
        {
            e.as_widget().drag_destinations(
                state,
                c_layout.with_virtual_offset(layout.virtual_offset()),
                renderer,
                dnd_rectangles,
            );
        }
    }
}

impl<'a, Message: 'static + Clone> From<Stage<'a, Message>> for Element<'a, Message> {
    fn from(flex_row: Stage<'a, Message>) -> Self {
        Self::new(flex_row)
    }
}
