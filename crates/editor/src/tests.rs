use std::path::Path;

use iced::{Settings, Task};

use super::{App, JobState, view};
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
    let app = App { img: JobState::Ready(Ok(Some(loaded))), ..App::default() };
    iced::application("drawn_test", App::update, view)
        .subscription(App::subscription)
        .settings(Settings { any_thread: true, ..Default::default() })
        .run_with(|| (app, Task::none()))?;

    Ok(())
}
