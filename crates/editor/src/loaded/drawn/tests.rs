use {
    crate::{
        App, FONT_SIZE, NamedImg, diac_renderer,
        loaded::{
            LoadedImage,
            drawn::{Drawn, RenderedDiac},
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
        let mut loaded = LoadedImage::new(NamedImg {
            path: test_utils::img_path!().to_owned().conv::<PathBuf>().into(),
            img: test_utils::IMAGE.clone().into(),
        });
        let mut renderer = diac_renderer::Renderer::new();
        let results = test_utils::POSITIONS
            .iter()
            .cloned()
            .map(|(letter, diac, pos)| {
                RenderedDiac::new(
                    letter,
                    diac,
                    pos.map(|rect| renderer.position(letter, diac, rect, FONT_SIZE)),
                    renderer.render(diac, FONT_SIZE, test_utils::DIAC_COLOR).into(),
                )
            })
            .collect();
        loaded.drawing = Poll::Ready(Ok(Some(Drawn::new(results))));
        loaded
    }
    let boot_fn = || App { img: Poll::Ready(Ok(Some(load()))), ..App::default() };
    run_app(boot_fn)?;

    Ok(())
}
