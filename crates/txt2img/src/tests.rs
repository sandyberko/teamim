use std::path::Path;

use ab_glyph::FontRef;
use teamim::TRAINING_TEXT;

#[test]
fn generate() -> eyre::Result<()> {
    let font = FontRef::try_from_slice(include_bytes!(
        "../../../assets/fonts/Stam_Ashkenaz_CLM_Medium.ttf"
    ))?;
    // let font = FontRef::try_from_slice(include_bytes!("../../../assets/fonts/Shlomo_Stam.ttf"))?;
    // let font = FontRef::try_from_slice(include_bytes!("../../../assets/fonts/Guttman_Stam.ttf"))?;
    // let text = "בראשית ברא אלהים את השמים ואת הארץ";
    let text = &TRAINING_TEXT[..1049];
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/test.tif");
    super::generate(&output, Vec::new(), font, text, false)?;
    Ok(())
}
