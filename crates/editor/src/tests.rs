use std::{path::Path, sync::Arc};

use crate::loaded::LoadedImage;

use super::{App, Poll, view};

#[test]
fn drawn_test() -> eyre::Result<()> {
    fn load() -> eyre::Result<LoadedImage> {
        let path = Path::new("../../assets/images/N5/007.jpg");
        let mut loaded = super::load_image(path)?;
        loaded.drawing = Poll::Ready(Ok(Some(loaded.clone().draw_teamim(
            c"../../assets/tessdata/",
            |progress| {
                eprintln!("{progress:?}");
            },
        )?)));
        Ok(loaded)
    }
    iced::application(
        || App { img: Poll::Ready(load().map(Some).map_err(Arc::new)), ..App::default() },
        App::update,
        view,
    )
    .subscription(App::subscription)
    .title("drawn test")
    .run()?;

    Ok(())
}
