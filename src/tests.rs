use eyre::WrapErr;

use crate::test_utils;

#[test]
fn diactrit_map() -> eyre::Result<()> {
    color_eyre::install()?;

    let map = super::build_diacrit_map()?;
    for (i, char) in search::text().chars().enumerate().take(100) {
        eprint!("{i}:\t{char}\t");
        if let Some((diacritic, letter)) = map.get(&i) {
            eprint!("[{letter}{diacritic}]");
        } else {
            eprint!("[ ]");
        }
        eprintln!();
    }
    Ok(())
}

#[test]
fn positions() -> eyre::Result<()> {
    let mut ctx = test_utils::ctx()?;
    let got = ctx
        .positions(&test_utils::IMAGE, |progress| eprintln!("{progress:?}"))
        .wrap_err("place error")?;
    assert_eq!(test_utils::POSITIONS, got);
    Ok(())
}
