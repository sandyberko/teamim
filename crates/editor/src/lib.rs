use iced::Point;

pub mod spinner;
pub mod stage;

pub fn stage<Handle>(children: impl IntoIterator<Item = (Point, Handle)>) -> stage::Stage<Handle> {
    stage::Stage::new(children.into_iter().collect())
}
