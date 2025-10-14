use iced::widget::image::Handle;
use std::{ffi::CStr, path::Path};
use teamim::leptonica_ext::PixBox;

use crate::{
    App, FONT_SIZE, diac_renderer,
    img::Img,
    load_image,
    loaded::{LoadedImage, drawn::Drawn},
    run_app,
    task::Poll,
    with_tctx,
};

pub(crate) const DATAPATH: &CStr = c"../../assets/tessdata/";
#[test]
fn drawn_test() -> eyre::Result<()> {
    fn load() -> eyre::Result<LoadedImage> {
        let path = Path::new("../../assets/images/N5/007.jpg");
        let mut loaded = load_image(path)?;
        let Img { width, height, pixels } = loaded.img.img.img();
        let results = PixBox::from_rgba8_with(
            &mut pixels.to_vec(),
            width.try_into()?,
            height.try_into()?,
            |img| {
                with_tctx(DATAPATH, |ctx| ctx.positions(img, |progress| eprintln!("{progress:?}")))
            },
        )???;

        loaded.drawing = Poll::Ready(Ok(Some(Drawn::new(results))));
        Ok(loaded)
    }
    let boot_fn = || App { img: Poll::Ready(load().map(Some)), ..App::default() };
    run_app(boot_fn)?;

    Ok(())
}
