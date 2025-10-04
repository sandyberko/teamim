use std::{path::Path, sync::Arc};

use crate::{loaded::LoadedImage, task::Poll};

use super::{App, TryPoll, view};

#[test]
fn drawn_test() -> eyre::Result<()> {
    fn load() -> eyre::Result<LoadedImage> {
        let path = Path::new("../../assets/images/N5/007.jpg");
        let mut loaded = super::load_image(path)?;
        let drawn = loaded.img.clone().draw_teamim(c"../../assets/tessdata/", |progress| {
            eprintln!("{progress:?}");
        })?;
        loaded.drawing = Poll::Ready(Ok(Some(drawn)));
        Ok(loaded)
    }
    iced::application(
        || App { img: Poll::Ready(load().map(Some)), ..App::default() },
        App::update,
        view,
    )
    .subscription(App::subscription)
    .title("drawn test")
    .run()?;

    Ok(())
}
