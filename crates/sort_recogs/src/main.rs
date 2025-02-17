use eyre::{bail, eyre, Context};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use maud::Markup;
use rayon::prelude::*;
use similar::{get_diff_ratio, utils::TextDiffRemapper, Algorithm, TextDiff};
use std::{
    cell::RefCell,
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    time::Duration,
};
use teamim::{
    training_diff::{self, Div},
    TeamimCtx, TRAINING_TEXT,
};

use clap::Parser;

#[derive(Parser)]
struct Args {
    input_dir: PathBuf,
    out_dir: PathBuf,

    #[arg(short, long)]
    /// Hide progress bars
    quiet: bool,
}

const PAGE_SEP: char = '\n';
const LINE_SEP: char = ' ';

#[allow(clippy::too_many_lines)]
fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    fs::create_dir(&args.out_dir)
        .wrap_err_with(|| eyre!("Failed to create output dir {:?}", args.out_dir))?;

    let test_cutoff = {
        let snippet = "במה אדע כי אירשנה";
        TRAINING_TEXT.find(snippet).unwrap() + snippet.len()
    };
    let old = &TRAINING_TEXT[..test_cutoff];

    let draw_target = if args.quiet {
        ProgressDrawTarget::hidden()
    } else {
        ProgressDrawTarget::stdout()
    };
    let bars = MultiProgress::with_draw_target(draw_target);
    bars.set_move_cursor(true);

    let overall_pb = bars.add(ProgressBar::new_spinner());
    overall_pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] / [{eta_precise}] {msg:.yellow} \x1B]9;4;1;{percent}\x07",
        )?
        .tick_chars("◐◓◑◒"),
    );
    overall_pb.enable_steady_tick(Duration::from_millis(200));
    let dir_bar_style = ProgressStyle::with_template(
        "{prefix:>12} |{bar:40.cyan/black}| <{pos:>3}/{len:3}> {msg}",
    )?;

    let dirs = fs::read_dir(args.input_dir)?
        .map(|dir| {
            let path = dir?.path();
            let mut files = fs::read_dir(&path)?
                .map(|file| {
                    let path = file?.path();
                    if !path
                        .extension()
                        .is_some_and(|ext| ext == "jpg" || ext == "jpeg")
                    {
                        bail!("expected jpg or jpeg, got {path:?}");
                    }
                    Ok(path)
                })
                .collect::<Result<Vec<_>, _>>()?;
            files.sort_unstable();

            let dir_name = path
                .file_name()
                .ok_or_else(|| eyre!("invalid dir {path:?}"))?
                .to_str()
                .ok_or_else(|| eyre!("non-utf8 dir name {path:?}"))?;

            let bar = bars.insert_before(
                &overall_pb,
                ProgressBar::new(files.len() as u64)
                    .with_style(dir_bar_style.clone())
                    .with_prefix(dir_name.to_owned()),
            );

            Ok((path, files, bar))
        })
        .collect::<eyre::Result<Vec<_>>>()?;

    overall_pb.set_length(dirs.iter().map(|(_, imgs, _)| imgs.len() as u64).sum());

    thread_local! {
        static CTX: RefCell<Option<TeamimCtx>> = RefCell::default();
    }

    let running = Arc::new(AtomicBool::new(true));
    ctrlc::set_handler({
        let running = running.clone();
        let overall_pb = overall_pb.clone();
        move || {
            running.store(false, atomic::Ordering::SeqCst);
            overall_pb.set_message("🚩 interrupting...");
        }
    })?;

    let mut distances = dirs
        .par_iter()
        .take_any_while(|_| running.clone().load(atomic::Ordering::SeqCst))
        .map(|(dir, imgs, bar)| -> eyre::Result<_> {
            let dir_name = dir
                .file_name()
                .ok_or_else(|| eyre!("invalid dir {dir:?}"))?
                .to_str()
                .ok_or_else(|| eyre!("non-utf8 dir name {dir:?}"))?;

            let out_dir = args.out_dir.join(dir_name);

            fs::create_dir(&out_dir)?;

            let mut ocr_text = String::new();
            let mut page_ranges = Vec::with_capacity(imgs.len());
            let mut page_boxes = Vec::with_capacity(imgs.len());

            for img in imgs {
                if !running.load(atomic::Ordering::SeqCst) {
                    bar.abandon_with_message("🚩 interrupted");
                    return Ok(vec![]);
                }

                let img_name = img
                    .file_name()
                    .ok_or_else(|| eyre!("invalid img {img:?}"))?;
                bar.set_message(format!("🔍 recognizing {img_name:?}"));
                bar.tick();

                CTX.with_borrow_mut(|ctx| -> eyre::Result<_> {
                    let ctx = if let Some(ctx) = ctx.as_mut() {
                        ctx
                    } else {
                        ctx.insert(
                            TeamimCtx::new()?
                                .with_debug_file(args.out_dir.join("tesseract.log"))?,
                        )
                    };

                    let page_start = ocr_text.len();
                    let mut line_start = 0;
                    let mut boxes = Vec::new();
                    for (idx, bx) in ctx.file_boxes(img)?.enumerate() {
                        if idx > 0 {
                            ocr_text.push(LINE_SEP);
                        }
                        let value = bx.value.trim_end();
                        let value_len = value.len() + 1 /* line/page sep */;
                        ocr_text.push_str(value);

                        let range = line_start..line_start + value_len;
                        boxes.push(bx.with_value(range));
                        line_start += value_len;
                    }
                    ocr_text.push(PAGE_SEP);
                    let range = page_start..ocr_text.len();
                    page_ranges.push(range);
                    page_boxes.push(boxes);
                    Ok(())
                })?;
                bar.inc(1);
                overall_pb.inc(1);
            }

            bar.set_message("± diffing...");

            let diff = TextDiff::configure()
                .algorithm(Algorithm::Myers)
                .timeout(Duration::from_secs(const { 60 * 5 }))
                .diff_chars(old, &ocr_text);

            let remapper = TextDiffRemapper::from_text_diff(&diff, old, &ocr_text);

            let mut ops = diff.ops();
            let mut old_start = 0;
            let distances = imgs
                .iter()
                .zip(&page_ranges)
                .zip(&page_boxes)
                .map(|((img, new_page), boxes)| -> eyre::Result<_> {
                    let sep_op = {
                        let mut iter = 0..ops.len();
                        loop {
                            let i = iter.next().ok_or_else(|| eyre!("no seperator "))?;
                            if remapper
                                .slice_new(ops[i].new_range())
                                .ok_or_else(|| eyre!("can't slice: {:?}", ops[i]))?
                                .contains(PAGE_SEP)
                            {
                                break i;
                            }
                        }
                    };
                    // TODO what if sep_op is not exactly PAGE_SEP?``
                    let page_ops = &ops[..sep_op];
                    let old_len = page_ops
                        .iter()
                        .map(|op| op.old_range().len())
                        .sum::<usize>();

                    let old = remapper
                        .slice_old(old_start..old_start + old_len)
                        .ok_or_else(|| eyre!("failed to remap old"))?;

                    let tess_box = training_diff::diff(boxes, &ocr_text[new_page.clone()], old);
                    let width = 0;
                    let height = 0;
                    let div = Div {
                        width,
                        height,
                        tess_box,
                    };
                    let img_name = img
                        .file_name()
                        .ok_or_else(|| eyre!("invalid img {img:?}"))?;
                    let out_file = out_dir.join(img_name).with_extension("html");
                    let mut w = BufWriter::new(
                        fs::File::create(&out_file)
                            .wrap_err_with(|| eyre!("failed to create diff file: {out_file:?}"))?,
                    );
                    write!(w, "{}", Markup::from(div).into_string())?;

                    let ratio = get_diff_ratio(page_ops, old_len, new_page.len());
                    ops = &ops[sep_op + 1..];
                    old_start += old_len;
                    Ok((ratio, img))
                })
                .enumerate()
                .map(|(page_idx, res)| {
                    res.wrap_err_with(|| eyre!("failed diffing page #{page_idx}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(distances)
        })
        .collect::<Result<Vec<_>, _>>()?
        .concat();

    let mut w = BufWriter::new(fs::File::create(args.out_dir.join("distances.txt"))?);

    if running.load(atomic::Ordering::SeqCst) {
        overall_pb.finish_with_message("🏁 done");
    } else {
        writeln!(w, "<interrupted>")?;
        overall_pb.abandon_with_message("🚩 interrupted");
    }

    distances.sort_unstable_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());

    for (distance, img) in distances {
        if distance == f32::MAX {
            writeln!(w, "<not found>\t\t\t{img}", img = img.display())?;
        } else {
            writeln!(w, "{distance}\t\t\t{img}", img = img.display())?;
        }
    }
    w.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{stdout, Write};

    #[test]
    fn test_progress() {
        for i in 0..100 {
            println!("\x1B]9;4;1;{i}\x07");
            stdout().flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
