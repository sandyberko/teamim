use image::imageops::overlay;

use crate::{
    App, FONT_SIZE, NamedImg, diac_renderer,
    img::ImgHandle,
    loaded::{
        LoadedImage,
        drawn::{Drawn, RenderedDiac, RenderedDiacPos},
    },
    run_app,
    task::Poll,
};
use {
    editor::stage,
    iced::{
        Border, Color, Element, Point, Task,
        widget::{container, container::Style, scrollable, stack},
    },
    image::ImageFormat,
    std::path::PathBuf,
    tap::prelude::*,
    teamim::test_utils,
};

#[test]
fn save() -> eyre::Result<()> {
    let mut renderer = diac_renderer::Renderer::new();
    let mut bottom = test_utils::IMAGE.clone();
    for (diac, pos) in test_utils::POSITIONS {
        let Ok(rect) = pos else { continue };
        let top = renderer.render(*diac, FONT_SIZE);
        overlay(&mut bottom, &top, rect.left.into(), rect.top.into());
    }
    bottom.save_with_format("../../../temp/saved-tests/007.png", ImageFormat::Png)?;
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
            let img = test_utils::IMAGE.clone().into();
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

#[test]
fn view() -> eyre::Result<()> {
    fn load() -> LoadedImage {
        let mut loaded = LoadedImage::new(NamedImg {
            path: test_utils::img_path!().to_owned().conv::<PathBuf>().into(),
            img: test_utils::IMAGE.clone().into(),
        });
        let mut renderer = diac_renderer::Renderer::new();
        let results = test_utils::POSITIONS
            .iter()
            .cloned()
            .map(|(diac, pos)| RenderedDiac {
                diac,
                position: match pos {
                    Ok(rect) => RenderedDiacPos::Letter(rect),
                    Err(miss) => RenderedDiacPos::Miss(miss),
                },
                img: renderer.render(diac, FONT_SIZE).into(),
            })
            .collect();
        loaded.drawing = Poll::Ready(Ok(Some(Drawn::new(results))));
        loaded
    }
    let boot_fn = || App { img: Poll::Ready(Ok(Some(load()))), ..App::default() };
    run_app(boot_fn)?;

    Ok(())
}
