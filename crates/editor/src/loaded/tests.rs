use {
    super::{Drawn, RenderedDiac},
    crate::{
        App, FONT_SIZE, MARGIN, NamedImg, diac_renderer,
        loaded::{
            LoadedImage,
            recognize::{self, diac_style},
        },
        run_app,
        task::Poll,
    },
    image::{ImageFormat, imageops::overlay},
    std::path::PathBuf,
    tap::prelude::*,
    teamim::test_utils,
};

#[test]
fn save() -> eyre::Result<()> {
    let mut renderer = diac_renderer::Renderer::new();
    let mut bottom = test_utils::IMAGE.clone();
    for (_, diac, pos) in test_utils::POSITIONS {
        let Ok(rect) = pos else { continue };
        let top = renderer.render(*diac, FONT_SIZE, test_utils::DIAC_COLOR);
        overlay(&mut bottom, &top, rect.left.into(), rect.top.into());
    }
    bottom.save_with_format("../../../temp/saved-tests/007.png", ImageFormat::Png)?;
    Ok(())
}

#[test]
fn view() -> eyre::Result<()> {
    fn load() -> LoadedImage {
        let mut renderer = diac_renderer::Renderer::new();
        let results = test_utils::POSITIONS
            .iter()
            .cloned()
            .map(|(letter, diac, pos)| {
                RenderedDiac::new(
                    letter,
                    diac,
                    pos.map(|rect| {
                        diac_renderer::Renderer::position(letter, diac, rect, FONT_SIZE, MARGIN)
                    }),
                    renderer.render(diac, FONT_SIZE, test_utils::DIAC_COLOR).into(),
                )
            })
            .collect();
        let drawing = Poll::Ready(Ok(Some(Drawn::new(results))));
        let recognizing = Some(Poll::Ready(Ok(recognize::State {
            positions: test_utils::POSITIONS.into(),
            drawing,
            diac_style: diac_style::State::default(),
        })));
        let img = NamedImg {
            path: test_utils::img_path!().to_owned().conv::<PathBuf>().into(),
            img: test_utils::IMAGE.clone().into(),
        };
        LoadedImage { recognizing, ..LoadedImage::new(img) }
    }
    let boot_fn = || App { img: Poll::Ready(Ok(Some(load()))) };
    run_app(boot_fn)?;

    Ok(())
}
