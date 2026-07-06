use {
    crate::{
        tesseract_ext::{PageIteratorLevel, bounding_box::BoundingBox},
        test_utils,
    },
    eyre::WrapErr,
    image::{ImageFormat, ImageReader},
    rayon::{
        iter::{IntoParallelIterator, ParallelIterator},
        prelude,
    },
    similar::{Algorithm, TextDiff, utils::TextDiffRemapper},
    std::{
        env, fs,
        io::{self, Write},
        iter,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, AtomicU8, Ordering},
            mpsc,
        },
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

macro_rules! reprint {
    ( $( $args:tt )* ) => {{
        print!("\r\x1B[2K");
        print!( $( $args )* );
        io::stdout().flush().unwrap();
    }};
}

#[test]
fn entire_book() -> eyre::Result<()> {
    println!("pwd: {}", env::current_dir()?.display());

    let range = 1..249;

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
    let stop = AtomicBool::new(false);
    let order = Ordering::Relaxed;

    // recognize
    let (boxes, recognized_text) = range
        .into_par_iter()
        .take_any_while(|_| !stop.load(Ordering::SeqCst))
        .map_init(
            || test_utils::ctx().unwrap(),
            |ctx, page_i| -> eyre::Result<_> {
                let page_x = page_i % 10;
                let page_y = page_i / 10;

                print!("\x1B[{page_y};{page_x}H.");
                io::stdout().flush()?;

                let img_path =
                    PathBuf::from_iter(["assets", "images", "N1", &format!("{page_i:03}.jpg")]);
                if !fs::exists(&img_path)? {
                    print!("\x1B[{page_y};{page_x}H!");
                    io::stdout().flush()?;

                    if matches!(page_i, 246 | 249) {
                        stop.store(true, order);
                        return Ok(None);
                    }
                    panic!("not found: {}", img_path.display());
                }

                print!("\x1B[{page_y};{page_x}H~");
                io::stdout().flush()?;

                let img = image::open(img_path)?.to_rgba8();
                ctx.tess.set_image(&img);
                ctx.tess.recognize()?;

                let boxes = ctx
                    .tess
                    .results_iter(PageIteratorLevel::Textline)
                    .map(|bx| {
                        let value = bx.value.as_str()?.chars().next();
                        Ok(BoundingBox::new_paged(value, bx.rect, bx.page))
                    })
                    .collect::<eyre::Result<Vec<_>>>()?;
                pages_processed_count.fetch_add(1, order);

                print!("\x1B[{page_y};{page_x}H1");
                io::stdout().flush()?;

                Ok(Some((boxes, ToOwned::to_owned(ctx.tess.get_text()?.as_str()?))))
            },
        )
        .filter_map(Result::transpose)
        .collect::<eyre::Result<Box<_>>>()?
        .into_iter()
        .unzip::<_, _, Vec<_>, String>();

    print!("\x1b[2J\x1b[H");
    println!("diffing...");

    // diff
    // (ocr, ground_truth)
    let old = search::text();
    let new = recognized_text.as_str();
    let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_chars(old, new);
    println!("remapping...");
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
    let mut ops = diff.ops().iter().peekable();

    Ok(())
}
