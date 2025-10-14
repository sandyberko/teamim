use std::io::Cursor;

use editor::stage;
use iced::{
    Element, Point, Task,
    widget::{image::Handle, stack},
};
use image::{ImageFormat, ImageReader, RgbaImage};
use teamim::leptonica_ext::PixBox;

use crate::{
    FONT_SIZE, diac_renderer,
    img::ImgHandle,
    loaded::{
        self, Transform,
        drawn::{Drawn, render},
    },
    with_tctx,
};

#[test]
fn save_test() -> eyre::Result<()> {
    let mut buf = image()?;
    let width = buf.width().try_into()?;
    let height = buf.height().try_into()?;
    let results = PixBox::from_rgba8_with(&mut buf, width, height, |img| {
        with_tctx(loaded::tests::DATAPATH, |ctx| {
            ctx.positions(img, |status| eprintln!("{status:?}"))
        })
    })???;
    render(&results, Transform::default(), &mut buf);
    buf.save_with_format("../../../temp/saved-tests/007.png", ImageFormat::Png)?;
    Ok(())
}

#[test]
fn diac_view() -> eyre::Result<()> {
    #[derive(Clone)]
    struct DiacState {
        img: ImgHandle,
        diac: ImgHandle,
        opts: Transform,
    }
    impl DiacState {
        fn view(&self) -> Element<'_, ()> {
            stack([
                self.img.view(self.opts.zoom),
                stage([(self.diac.view(self.opts.zoom), Point::new(50., 50.))]).into(),
            ])
            .into()
        }
        fn update(&mut self, (): ()) -> Task<()> {
            Task::none()
        }

        fn boot() -> Self {
            let buf = image().unwrap();
            let mut renderer = diac_renderer::Renderer::new();
            let opts = Transform::default();
            let diac = renderer.render('\u{0591}', FONT_SIZE * opts.zoom).into();
            let img = buf.into();
            DiacState { img, diac, opts }
        }
    }

    iced::application(DiacState::boot, DiacState::update, DiacState::view).run();
    Ok(())
}

fn image() -> eyre::Result<RgbaImage> {
    let bytes = include_bytes!("../../../../../assets/images/N5/007.jpg");
    let buf = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg).decode()?.to_rgba8();
    Ok(buf)
}
