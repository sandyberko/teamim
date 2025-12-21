use {
    crate::stage::Overlay,
    iced::advanced::{image, text},
    std::sync::LazyLock,
    swash::FontRef,
};

pub mod spinner;
pub mod stage;

pub const GUTTMAN: &[u8] = include_bytes!("../../../assets/fonts/Guttman_Stam.ttf");
pub static FONT: LazyLock<FontRef<'static>> =
    LazyLock::new(|| FontRef::from_index(GUTTMAN, 0).expect("invalid font data"));

pub fn stage<Message, Renderer>(
    children: impl IntoIterator<Item = Overlay<Renderer::Handle>>,
) -> stage::Stage<Message, Renderer>
where
    Renderer: image::Renderer + text::Renderer,
{
    stage::Stage::new(children.into_iter().collect())
}
