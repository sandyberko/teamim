use std::io::Cursor;

use image::{ImageFormat, ImageReader};
use teamim::leptonica_ext::PixBox;

use crate::{
    loaded::{self, Transform, drawn::render},
    with_tctx,
};

#[test]
fn save_test() -> eyre::Result<()> {
    let bytes = include_bytes!("../../../../../assets/images/N5/007.jpg");
    let mut buf =
        ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg).decode()?.to_rgba8();
    let width = buf.width().try_into()?;
    let height = buf.height().try_into()?;
    let (positions, _) = PixBox::from_rgba8_with(&mut buf, width, height, |img| {
        with_tctx(loaded::tests::DATAPATH, |ctx| {
            ctx.positions(img, |status| eprintln!("{status:?}"))
        })
    })???;
    render(&positions, Transform::default(), &mut buf);
    buf.save_with_format("../../../temp/saved-tests/007.png", ImageFormat::Png)?;
    Ok(())
}
