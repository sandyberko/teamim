use {
    crate::diac_renderer::{self, DiacPositionOpts},
    image::imageops,
    memmap2::Mmap,
    rayon::{iter::ParallelIterator, slice::ParallelSlice},
    rkyv::rancor,
    std::{
        collections::HashMap,
        env,
        fs::{self, File},
        io::{self, ErrorKind, Write},
        path::Path,
    },
    teamim::{entire_book, glyph::GLYPHS},
};

const WORKSPACE_DIR: &str = "../../";

#[test]
fn draw_entire_book() -> eyre::Result<()> {
    let pwd = env::current_dir()?;
    let cache_dir = Path::new(WORKSPACE_DIR).join("target/cache/");
    let cache_path = cache_dir.join("entire_book_test.bin");
    let book_name = "N1";
    let book_path = Path::new(WORKSPACE_DIR).join("assets/images/").join(book_name);

    // recognize (cached)
    let pages = match File::open(&cache_path) {
        Ok(cache) => {
            eprintln!("cache hit, mmaping...");
            let bytes = unsafe { Mmap::map(&cache)? };
            eprintln!("deserializing...");
            rkyv::from_bytes::<_, rancor::Error>(&bytes)?
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            eprintln!("cache miss: {}/{}", pwd.display(), cache_path.display());
            eprintln!("recognizing...");
            let pages = entire_book::recognize(&book_path)?;

            fs::create_dir_all(cache_dir)?;
            let bytes = rkyv::to_bytes::<rancor::Error>(&pages)?;
            fs::write(&cache_path, bytes)?;

            pages
        }
        Err(err) => return Err(err.into()),
    };

    eprintln!("diffing...");
    let old = search::text();
    let new = pages.iter().map(|page| page.text.as_str()).collect::<String>();
    let boxes = pages.iter().flat_map(|page| page.boxes.as_slice()).copied().collect::<Box<[_]>>();

    let mut placed = entire_book::diff_and_place(old, &new, &boxes)
        .into_iter()
        .filter_map(|(cluster, place)| Some((cluster, place.ok()?)))
        .collect::<Box<[_]>>();
    placed.sort_unstable_by_key(|(_, place)| place.page);

    eprintln!("drawing...");
    fs::create_dir(book_name)?;
    let opts = DiacPositionOpts { font_size: 47.0, margin: 4.0 };
    // TODO keep the phf?
    let map = {
        let mut renderer = diac_renderer::Renderer::new();
        GLYPHS
            .keys()
            .copied()
            .map(|diac| (diac, renderer.render(diac, opts.font_size, [255, 0, 0])))
            .collect::<HashMap<_, _>>()
    };
    placed.par_chunk_by(|(_, x), (_, y)| x.page == y.page).for_each(|page_clusters| {
        let page_i = page_clusters[0].1.page;
        let page_name = format!("{:03}", page_i + 1);

        writeln!(io::stderr().lock(), "{page_name}...").unwrap();

        let page_file = Path::new(&page_name).with_extension("jpg");
        let mut bottom = image::open(book_path.join(&page_file)).unwrap().to_rgba8();
        for (cluster, place) in page_clusters {
            let Some(top) = map.get(&cluster.diacritic) else {
                eprint!("cannot find diacritic");
                eprint!(" {:X}", cluster.diacritic as u32);
                eprint!(" א{}", cluster.diacritic);
                eprintln!();
                continue;
            };
            let point = diac_renderer::Renderer::position(
                cluster.letter,
                cluster.diacritic,
                place.rect,
                opts,
            )
            .snap();
            imageops::overlay(&mut bottom, top, point.x.into(), point.y.into());
        }
        bottom.save(Path::new(book_name).join(&page_name).with_extension("png")).unwrap();

        writeln!(io::stderr().lock(), "{page_name}: saved!").unwrap();
    });

    Ok(())
}
