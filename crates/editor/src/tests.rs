use std::path::Path;

use cosmic::{
    Core, app,
    iced::executor,
    iced_widget::scrollable::{Direction, Scrollbar},
    task,
    widget::{Column, scrollable, text},
};

use crate::JobState;
use teamim::PlaceOptions;

#[test]
fn drawn_test() -> eyre::Result<()> {
    let path = Path::new("../../assets/images/N5/007.jpg");
    let mut loaded = super::load_image(path)?;
    loaded.drawing = JobState::Ready(Ok(Some(loaded.clone().draw_teamim(
        c"../../assets/tessdata/",
        PlaceOptions::default(),
        |progress| {
            eprintln!("{progress:?}");
        },
    )?)));

    app::run::<super::App>(
        app::Settings::default().any_thread(true),
        JobState::Ready(Ok(Some(loaded))),
    )?;
    Ok(())
}

#[test]
fn horizontal_scroll_rtl() -> eyre::Result<()> {
    struct App(Core);
    impl app::Application for App {
        type Executor = executor::Default;

        type Flags = ();

        type Message = ();

        const APP_ID: &'static str = "org.teamim.test.horizontal_scroll_rtl";

        fn core(&self) -> &Core {
            &self.0
        }

        fn core_mut(&mut self) -> &mut Core {
            &mut self.0
        }

        fn init(core: Core, (): Self::Flags) -> (Self, app::Task<Self::Message>) {
            (App(core), task::none())
        }

        fn view(&'_ self) -> cosmic::Element<'_, Self::Message> {
            scrollable(Column::with_children([
                text("מה לעזאזל").into(),
                text("what the hell").into(),
            ]))
            .direction(Direction::Horizontal(Scrollbar::new()))
            .into()
        }
    }
    app::run::<App>(app::Settings::default().any_thread(true), ())?;
    Ok(())
}
