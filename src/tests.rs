mod block_hashing;

use {
    crate::{
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
macro_rules! reprint {
    ( $( $args:tt )* ) => {{
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        write!(stdout, $( $args )* ).unwrap();
        stdout.flush().unwrap();
    }};
}

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
            let foo = recognize_entire_book()?;

            fs::create_dir_all(cache_dir)?;
            let bytes = rkyv::to_bytes::<rancor::Error>(&foo)?;
            fs::write(&cache_path, bytes)?;

            foo
        }
        Err(err) => return Err(err.into()),
    };

    print!("\x1b[2J\x1b[H");

    // diff
    let old = search::text();
    let new = text.as_str();

    let placed = block_hashing::diff_and_place(old, new, &boxes);
    Ok(())
}

fn recognize_entire_book() -> eyre::Result<(Vec<BoundingBox<char>>, String)> {
    let range = 1..=248;

    print!("\x1b[2J\x1b[H");

    io::stdout().flush()?;

    for page_i in range.clone() {
        io::stdout().flush()?;
        print!("0");
        if page_i % 10 == 0 {
            println!();
        }
    }
    io::stdout().flush()?;

    let pages_processed_count = AtomicU8::new(0);
    let order = Ordering::Relaxed;
    range
        .into_par_iter()
        .map_init(
            || test_utils::ctx().unwrap(),
            |ctx, page_i| -> eyre::Result<_> {
                let page_x = page_i % 10;
                let page_y = page_i / 10;

                reprint!("\x1B[{page_y};{page_x}H.");

                let img_path =
                    PathBuf::from_iter(["assets", "images", "N1", &format!("{page_i:03}.jpg")]);
                if !fs::exists(&img_path)? {
                    reprint!("\x1B[{page_y};{page_x}H!");

                    if matches!(page_i, 246..=248) {
                        return Ok(None);
                    }
                    panic!("not found: {}", img_path.display());
                }

                reprint!("\x1B[{page_y};{page_x}H~");

                let img = image::open(img_path)?.to_rgba8();
                ctx.tess.set_image(&img);
                ctx.tess.recognize()?;

                let text = ctx.tess.get_text()?;
                let text = text.as_str()?.replace(char::is_whitespace, "");

                // boxes
                let boxes = ctx
                    .tess
                    .results_iter(PageIteratorLevel::Symbol)
                    .map(|BoundingBox { value, rect, page }| {
                        eyre::Ok(BoundingBox {
                            value: value.as_str()?.chars().next().ok_or_eyre("empty box")?,
                            rect,
                            page,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                pages_processed_count.fetch_add(1, order);

                reprint!("\x1B[{page_y};{page_x}H1");

                Ok(Some((boxes, text)))
            },
        )
        .filter_map(Result::transpose)
        .try_reduce(
            || (Vec::new(), String::new()),
            |(mut boxes, mut text), (page_boxes, page_text)| {
                boxes.extend(page_boxes);
                text.push_str(&page_text);
                Ok((boxes, text))
            },
        )
}
