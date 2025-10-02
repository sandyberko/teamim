use iced::{Element, Point};

pub mod spinner;
pub mod stage;

pub fn stage<'a, Message, Theme, Renderer>(
    children: impl IntoIterator<Item = (Element<'a, Message, Theme, Renderer>, Point)>,
) -> stage::Stage<'a, Message, Theme, Renderer> {
    stage::Stage::with_children(children)
}
