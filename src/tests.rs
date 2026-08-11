use {
    crate::{
        entire_book,
        tesseract_ext::{PageIteratorLevel, bounding_box::BoundingBox},
        test_utils,
    },
    eyre::{OptionExt, WrapErr},
    memmap2::Mmap,
    rayon::iter::{IntoParallelIterator, ParallelIterator},
    rkyv::rancor,
    std::{
        env,
        fs::{self, File},
        io::{self, ErrorKind, Write},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU8, Ordering},
    },
};

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

// Keep a full terminal update together when rayon workers report progress.
#[test]
fn entire_book() -> eyre::Result<()> {
    let cache_dir = Path::new("./target/cache/");
    let cache_path = cache_dir.join("entire_book_test.bin");

    // recognize (cached)
    let (boxes, text) = match File::open(&cache_path) {
        Ok(cache) => {
            println!("mmaping...");
            let bytes = unsafe { Mmap::map(&cache)? };
            println!("deserializing...");
            rkyv::from_bytes::<_, rancor::Error>(&bytes)?
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let foo = entire_book::recognize(&PathBuf::from_iter(["assets", "images", "N1"]))?;

            fs::create_dir_all(cache_dir)?;
            let bytes = rkyv::to_bytes::<rancor::Error>(&foo)?;
            fs::write(&cache_path, bytes)?;

            foo
        }
        Err(err) => return Err(err.into()),
    };

    // diff
    let old = search::text();
    let new = text.as_str();

    let placed = entire_book::diff_and_place(old, new, &boxes);

    Ok(())
}
