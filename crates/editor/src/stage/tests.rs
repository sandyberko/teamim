use iced::{Point, widget::text};

#[test]
fn main() -> eyre::Result<()> {
    type State = ();
    type Message = ();
    fn update(_: &mut State, _: Message) {}
    fn view(_: &'_ State) -> iced::Element<'_, Message> {
        super::Stage::<(), _, _>::new()
            .push(text("foo"), Point::ORIGIN)
            .push(text("bar"), (0., 100.).into())
            .push(text("baz"), (100., 100.).into())
            .into()
    }
    iced::run(update, view)?;
    Ok(())
}
