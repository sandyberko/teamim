use std::{
    ffi::CStr,
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

use ::tap::prelude::*;
use editor::stage;
use iced::{
    Border, Color, Element, Point, Task,
    widget::{container, container::Style, image::Handle, scrollable, stack},
};
use image::{ImageBuffer, ImageFormat, ImageReader, Rgba, RgbaImage};
use teamim::DiacResultKind;

use crate::{
    App, FONT_SIZE, NamedImg, diac_renderer,
    img::ImgHandle,
    load_image,
    loaded::{
        self, LoadedImage, Transform,
        drawn::{Drawn, RenderedDiac, render},
    },
    run_app,
    task::Poll,
    with_tctx,
};
const DATAPATH: &CStr = c"../../assets/tessdata/";

#[test]
fn save_test() -> eyre::Result<()> {
    let mut img = image()?;
    let results =
        with_tctx(DATAPATH, |ctx| ctx.positions(&img, |status| eprintln!("{status:?}")))??;
    render(
        results.iter().filter_map(|(diac, res)| {
            if let DiacResultKind::Pos(rect) = res { Some((*diac, *rect)) } else { None }
        }),
        Transform::default(),
        &mut img,
    );
    img.save_with_format("../../../temp/saved-tests/007.png", ImageFormat::Png)?;
    Ok(())
}
fn red_frame<'a, Message: 'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .style(|_| {
            Style::default().border(Border::default().color(Color::from_rgb8(0xFF, 0, 0)).width(2))
        })
        .into()
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
            scrollable(stack([
                self.img.view(self.opts.zoom),
                red_frame(stage([(
                    red_frame(self.diac.view(self.opts.zoom)),
                    Point::new(50., 50.),
                )])),
            ]))
            .into()
        }

        fn update(&mut self, (): ()) -> Task<()> {
            Task::none()
        }

        fn boot() -> Self {
            let buf = image().unwrap();
            let img = buf.into();

            let mut renderer = diac_renderer::Renderer::new();
            let opts = Transform::default();
            let size = FONT_SIZE * opts.zoom;
            let diac = renderer.render('\u{0591}', size).into();
            DiacState { img, diac, opts }
        }
    }

    iced::application(DiacState::boot, DiacState::update, DiacState::view).run()?;
    Ok(())
}

macro_rules! img_path {
    () => {
        "../../../../../assets/images/N5/007.jpg"
    };
}
fn image() -> eyre::Result<RgbaImage> {
    let bytes = include_bytes!(img_path!());
    let buf = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg).decode()?.to_rgba8();
    Ok(buf)
}

#[test]
fn view() -> eyre::Result<()> {
    fn load() -> eyre::Result<LoadedImage> {
        let mut loaded = LoadedImage::new(NamedImg {
            path: img_path!().to_owned().conv::<PathBuf>().into(),
            img: image()?.into(),
        });
        let results = super::posit_diacs(&loaded.img().img(), DATAPATH, 0.4, |progress| {
            eprintln!("{progress:?}")
        })?;
        loaded.drawing = Poll::Ready(Ok(Some(Drawn::new(results))));
        Ok(loaded)
    }
    let boot_fn = || App {
        img: Poll::Ready(load().map_err(|err| err.to_string()).map(Some)),
        ..App::default()
    };
    run_app(boot_fn)?;

    Ok(())
}
