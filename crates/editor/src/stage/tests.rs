use {
    crate::stage::Overlay,
    iced::{ContentFit, Element, Point, Task, widget::image::Handle},
    image::{Rgba, RgbaImage},
    teamim::test_utils,
};

#[test]
fn run() -> eyre::Result<()> {
    struct State {
        background: Handle,
        overlays: Box<[Overlay<Handle>]>,
    }
    type Message = ();

    fn boot() -> State {
        let background = test_utils::IMAGE.clone();
        let background =
            Handle::from_rgba(background.width(), background.height(), background.to_vec());

        let overlay = RgbaImage::from_pixel(10, 10, Rgba([0xff, 0, 0, 0xff]));
        let overlay = Handle::from_rgba(overlay.width(), overlay.height(), overlay.into_vec());

        #[expect(clippy::cast_precision_loss)]
        let overlays = (0..100)
            .flat_map(|x| (0..100).map(move |y| Point::new(100.0 * x as f32, 100.0 * y as f32)))
            .map(|pos| Overlay::new(pos, overlay.clone(), None))
            .collect();
        State { background, overlays }
    }
    fn update(_: &mut State, _: Message) -> Task<Message> {
        Task::none()
    }
    fn view(state: &State) -> Element<'_, Message> {
        super::Stage::new(state.overlays.clone())
            .handle(state.background.clone())
            .content_fit(ContentFit::None)
            .into()
    }
    iced::application(boot, update, view).run()?;
    Ok(())
}
