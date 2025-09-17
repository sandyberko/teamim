use std::path::Path;

use cosmic::app;
use teamim::PlaceOptions;

use crate::JobState;

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
