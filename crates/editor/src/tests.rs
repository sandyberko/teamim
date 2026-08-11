use {
    memmap2::Mmap,
    rkyv::rancor,
    std::{
        env,
        fs::{self, File},
        io::ErrorKind,
        path::{Path, PathBuf},
    },
    teamim::entire_book,
};

const WORKSPACE_DIR: &str = "../../";

#[test]
fn draw_entire_book() -> eyre::Result<()> {
    let pwd = env::current_dir()?;
    let cache_dir = Path::new(WORKSPACE_DIR).join("target/cache/");
    let cache_path = cache_dir.join("entire_book_test.bin");

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
            let pages =
                entire_book::recognize(&Path::new(WORKSPACE_DIR).join("assets/images/N1/"))?;

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

    let placed = entire_book::diff_and_place(old, &new, &boxes);

    eprintln!("drawing...");

    Ok(())
}
