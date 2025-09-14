use cosmic::{
    Element, Renderer, Theme,
    iced::{Length, Rectangle, Size},
    iced_core::{
        Layout,
        layout::{Limits, Node},
        mouse::Cursor,
        renderer::Style,
        widget::Tree,
    },
    widget::Widget,
};

pub struct Btn<Message>(pub Element<'static, Message>);
impl<Message> Widget<Message, Theme, Renderer> for Btn<Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.0)]
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.0.as_widget().layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.0.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            &viewport.intersection(&layout.bounds()).unwrap_or_default(),
        );
    }
}
