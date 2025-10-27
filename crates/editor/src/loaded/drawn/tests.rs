use crate::{
    App, FONT_SIZE, NamedImg, diac_renderer,
    img::ImgHandle,
    loaded::{
        LoadedImage,
        drawn::{Drawn, overlay_diacs, render_diacs},
    },
    run_app,
    task::Poll,
};
use ::{
    editor::stage,
    iced::{
        Border, Color, Element, Point, Task,
        widget::{container, container::Style, scrollable, stack},
    },
    image::{ImageFormat, ImageReader, RgbaImage},
    std::{ffi::CStr, io::Cursor, path::PathBuf},
    tap::prelude::*,
};

const DATAPATH: &CStr = c"../../assets/tessdata/";

#[test]
fn save_test() -> eyre::Result<()> {
    let mut img = image()?;
    let results = render_diacs(&img, DATAPATH, |status| eprintln!("{status:?}"))?;
    overlay_diacs(&results, &mut img);
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
        zoom: f32,
    }
    impl DiacState {
        fn view(&self) -> Element<'_, ()> {
            scrollable(stack([
                self.img.view(self.zoom),
                red_frame(stage([(red_frame(self.diac.view(self.zoom)), Point::new(50., 50.))])),
            ]))
            .into()
        }

        #[expect(clippy::unused_self)]
        fn update(&mut self, (): ()) -> Task<()> {
            Task::none()
        }

        fn boot() -> Self {
            let buf = image().unwrap();
            let img = buf.into();

            let mut renderer = diac_renderer::Renderer::new();
            let zoom = 0.4;
            let size = FONT_SIZE * zoom;
            let diac = renderer.render('\u{0591}', size).into();
            DiacState { img, diac, zoom }
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
        let results = super::render_diacs(&loaded.img().img(), DATAPATH, |progress| {
            eprintln!("{progress:?}");
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
