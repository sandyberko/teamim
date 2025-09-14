use cosmic::{Core, Element, app, executor, iced::Point, widget::text};

fn main() -> eyre::Result<()> {
    struct App {
        core: Core,
    }
    impl cosmic::Application for App {
        type Executor = executor::Default;
        type Flags = ();
        type Message = ();
        const APP_ID: &'static str = "org.teamim.test";

        fn init(core: Core, _flags: Self::Flags) -> (Self, app::Task<Self::Message>) {
            (Self { core }, app::Task::none())
        }

        fn core(&self) -> &Core {
            &self.core
        }
        fn core_mut(&mut self) -> &mut Core {
            &mut self.core
        }

        fn view(&'_ self) -> Element<'_, Self::Message> {
            editor::stage::Stage::new()
                .push(text("foo"), Point::ORIGIN)
                .push(text("bar"), (0., 100.).into())
                .push(text("baz"), (100., 100.).into())
                .into()
        }
    }
    app::run::<App>(app::Settings::default(), ())?;
    Ok(())
}
