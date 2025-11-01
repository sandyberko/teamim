use iced::{Element, Point, advanced};

pub mod spinner;
pub mod stage;

pub fn stage<'a, Message, Theme, Renderer>(
    children: impl IntoIterator<Item = (Element<'a, Message, Theme, Renderer>, Point)>,
) -> stage::Stage<Renderer::Handle>
where
    Renderer: advanced::image::Renderer,
{
    stage::Stage::new::<Renderer::Handle>()
}
