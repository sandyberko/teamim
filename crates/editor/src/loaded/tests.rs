use std::path::Path;

use crate::{
    App, load_image,
    loaded::{LoadedImage, drawn::Drawn, position_diacs},
    run_app,
    task::Poll,
};

#[test]
fn drawn_test() -> eyre::Result<()> {
    fn load() -> eyre::Result<LoadedImage> {
        let path = Path::new("../../assets/images/N5/007.jpg");
        let mut loaded = load_image(path)?;
        let progress_callback = |progress| eprintln!("{progress:?}");
        let (positions, misses) =
            position_diacs(&loaded.img, c"../../assets/tessdata/", progress_callback)?;
        loaded.drawing = Poll::Ready(Ok(Some(Drawn::new(positions, misses))));
        Ok(loaded)
    }
    let boot_fn = || App { img: Poll::Ready(load().map(Some)), ..App::default() };
    run_app(boot_fn)?;

    Ok(())
}
