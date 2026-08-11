mod block_hashing;

use {
    crate::{
        tesseract_ext::{PageIteratorLevel, bounding_box::BoundingBox},
        test_utils,
    },
    eyre::OptionExt,
    rayon::iter::{IntoParallelIterator, ParallelIterator},
    std::{
        fs,
        path::Path,
        sync::atomic::{AtomicU8, Ordering},
    },
};

pub use block_hashing::diff_and_place;

pub fn recognize(book_dir: &Path) -> eyre::Result<(Vec<BoundingBox<char>>, String)> {
    let range = 1..=248;

    let pages_processed_count = AtomicU8::new(0);
    let order = Ordering::Relaxed;
    range
        .into_par_iter()
        .map_init(
            || test_utils::ctx().unwrap(),
            |ctx, page_i| -> eyre::Result<_> {
                let img_path = book_dir.join(format!("{page_i:03}.jpg"));
                if !fs::exists(&img_path)? {
                    if matches!(page_i, 246..=248) {
                        return Ok(None);
                    }
                    panic!("not found: {}", img_path.display());
                }

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
