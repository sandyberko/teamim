use iced::Point;

pub mod spinner;
pub mod stage;

pub fn stage<Message, Handle>(
    children: impl IntoIterator<Item = (Point, Handle)>,
) -> stage::Stage<Message, Handle> {
    stage::Stage::new(children.into_iter().collect())
}
