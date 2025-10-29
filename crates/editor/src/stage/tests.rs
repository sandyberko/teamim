use iced::{
    Point,
    widget::{button, text},
};

#[test]
fn stage_test() -> eyre::Result<()> {
    type State = ();
    type Message = ();
    fn update(_: &mut State, _: Message) {}
    fn view(_: &'_ State) -> iced::Element<'_, Message> {
        super::Stage::<(), _, _, _>::new()
            .push(text("foo"), Point::ORIGIN)
            .push(text("bar"), (0., 100.))
            .push(text("baz"), (100., 100.))
            .push(button("Click Me").on_press(()), (0., 200.))
            .into()
    }
    iced::run(update, view)?;
    Ok(())
}
