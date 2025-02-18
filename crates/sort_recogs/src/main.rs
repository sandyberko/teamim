use color_eyre::Section;
use eyre::{bail, eyre, Context, OptionExt};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use maud::Markup;
use rayon::prelude::*;
use similar::{get_diff_ratio, utils::TextDiffRemapper, Algorithm, TextDiff};
use std::{
    cell::RefCell,
    fs,
    io::{BufWriter, Write},
    ops::Range,
    path::PathBuf,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    time::Duration,
};
use teamim::{tesseract_ext::BoundingBox, training_diff::Div, TeamimCtx, TRAINING_TEXT};

use clap::Parser;

#[derive(Parser)]
struct Args {
    input_dir: PathBuf,
    out_dir: PathBuf,

    #[arg(short, long)]
    /// Hide progress bars
    quiet: bool,

    #[arg(long)]
    test_cutoff: Option<String>,
}

#[allow(clippy::too_many_lines)]
fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    fs::create_dir(&args.out_dir)
        .wrap_err_with(|| eyre!("Failed to create output dir {:?}", args.out_dir))?;

    let old = if let Some(cutoff) = args.test_cutoff {
        let end = TRAINING_TEXT.find(&cutoff).ok_or_eyre("cutoff not found")?;
        &TRAINING_TEXT[..end]
    } else {
        TRAINING_TEXT
    };

    let draw_target = if args.quiet {
        ProgressDrawTarget::hidden()
    } else {
        ProgressDrawTarget::stdout()
    };
    let bars = MultiProgress::with_draw_target(draw_target);

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

    // load file paths
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

    // recognize and diff
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

            // recognize
            let pages = imgs
                .par_iter()
                .take_any_while(|_| running.clone().load(atomic::Ordering::SeqCst))
                .map(|img| {
                    let img_name = img
                        .file_name()
                        .ok_or_else(|| eyre!("invalid img {img:?}"))?;
                    bar.set_message(format!("🔍 recognizing {img_name:?}"));
                    bar.tick();

                    CTX.with_borrow_mut(|ctx| -> eyre::Result<_> {
                        let ctx = if let Some(ctx) = ctx.as_mut() {
                            ctx
                        } else {
                            let new = TeamimCtx::new()?
                                .with_debug_file(args.out_dir.join("tesseract.log"))?;
                            ctx.insert(new)
                        };

                        let mut line_start = 0;
                        let mut boxes = Vec::new();
                        let mut ocr_text = String::new();
                        let (boxes_iter, width, height) = ctx.file_boxes(img)?;
                        for (idx, bx) in boxes_iter.enumerate() {
                            const NEWLINE: &str = "\n";
                            let Some(value) = bx.value.strip_suffix('\n') else {
                                bail!("missing trailing newline in {img:?}:{idx}: {:?}", bx.value);
                            };
                            let value_len = value.chars().count() + NEWLINE.len();
                            ocr_text.push_str(value);
                            ocr_text.push(' ');

                            let range = line_start..line_start + value_len;
                            eprintln!("[{range:<10?}]{:?}", bx.value);
                            boxes.push(bx.with_value(range));
                            line_start += value_len;
                        }
                        bar.inc(1);
                        overall_pb.inc(1);
                        Ok((img, ocr_text, boxes, width, height))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if !running.load(atomic::Ordering::SeqCst) {
                bar.abandon_with_message("🚩 interrupted");
                return Ok(vec![]);
            }

            bar.set_message("± diffing...");

            let ocr_text = pages
                .iter()
                .map(|(_, ocr_text, ..)| ocr_text.as_str())
                .collect::<String>();

            // TODO diff hook?
            // diff
            let diff = TextDiff::configure()
                .algorithm(Algorithm::Myers)
                .timeout(Duration::from_secs(const { 60 * 5 }))
                .diff_chars(old, &ocr_text);

            let remapper = TextDiffRemapper::from_text_diff(&diff, old, &ocr_text);
            let ops = diff.ops().to_vec();
            let mut ops_iter = ops.iter().copied().peekable();
            let mut page_ops = Vec::new();

            let distances = pages
                .into_iter()
                .map(
                    |(img, _new_page, boxes, width, height)| -> eyre::Result<_> {
                        bar.set_message(format!("🗺️ mapping {}", img.display()));

                        let (tess_box, old_len, new_len) = {
                            use teamim::training_diff::DiffOp;
                            let remapper: &TextDiffRemapper<str> = &remapper;
                            let ocr_lines: &[BoundingBox<Range<usize>>] = &boxes;

                            let (mut texts, mut diffs): (Vec<_>, Vec<_>) = ocr_lines
                                .iter()
                                .map(|bb_line| {
                                    (bb_line.value.clone(), bb_line.with_value(Vec::new()))
                                })
                                .unzip();

                            let mut lines_iter = texts
                                .iter_mut()
                                .zip(diffs.iter_mut())
                                .enumerate()
                                .peekable();

                            let mut page_old_len = 0;
                            let mut page_new_len = 0;
                            'page: while let Some(op) = ops_iter.peek_mut() {
                                let tag = op.tag();
                                eprintln!("=== {tag:?} ===");
                                eprintln!(
                                    "old: {:?}",
                                    remapper.slice_old(op.old_range()).unwrap_or("<err>")
                                );
                                eprintln!(
                                    "new: {:?}",
                                    remapper.slice_new(op.new_range()).unwrap_or("<err>")
                                );
                                match op {
                                    similar::DiffOp::Delete {
                                        old_index,
                                        old_len,
                                        new_index: _,
                                    } => {
                                        let Some((_, (_, line))) = lines_iter.peek_mut() else {
                                            break 'page;
                                        };
                                        let text = remapper
                                            .slice_old(*old_index..*old_index + *old_len)
                                            .ok_or_eyre("-")?;
                                        line.value.push(DiffOp::Delete(text.to_owned()));
                                        page_old_len += *old_len;
                                        page_ops.push(*op);
                                        ops_iter.next();
                                    }
                                    similar::DiffOp::Equal {
                                        old_index,
                                        new_index,
                                        len: new_len,
                                    }
                                    | similar::DiffOp::Insert {
                                        old_index,
                                        new_index,
                                        new_len,
                                    } => 'lines: loop {
                                        let Some((_line_idx, (new_line, line_diff))) =
                                            lines_iter.peek_mut()
                                        else {
                                            break 'page;
                                        };
                                        eprintln!("--- line ---");
                                        eprintln!("[{new_line:?}|{}]{:?}", new_line.len(), remapper.slice_new(new_line.clone()).unwrap_or("<err>"));
                                        let chunk_len = (*new_len).min(new_line.len());
                                        let chunk = remapper
                                            .slice_new(*new_index..*new_index + chunk_len)
                                            .ok_or_else(|| {
                                                eyre!("byte index {chunk_len} is not a char boundary")
                                                    .section(format!("line: {new_line:?}"))
                                            })?;
                                        eprintln!("chunk: {chunk:?}");

                                        page_new_len += *new_len;
                                        match tag {
                                            similar::DiffTag::Equal => {
                                                line_diff.value.push(DiffOp::Equal(chunk.to_owned()));
                                                page_ops.push(similar::DiffOp::Equal {
                                                    old_index: *old_index,
                                                    new_index: *new_index,
                                                    len: chunk_len,
                                                });
                                            },
                                            similar::DiffTag::Insert => {
                                                line_diff.value.push(DiffOp::insert(chunk.to_owned()));
                                                page_ops.push(similar::DiffOp::Insert {
                                                    old_index: *old_index,
                                                    new_index: *new_index,
                                                    new_len: chunk_len,
                                                });
                                            }
                                            _ => unreachable!(),
                                        };

                                        // advance
                                        *old_index += chunk_len;
                                        *new_index += chunk_len;
                                        *new_len -= chunk_len;

                                        new_line.start += chunk_len;
                                        if Range::<usize>::is_empty(new_line) {
                                            lines_iter.next();
                                        }
                                        if *new_len == 0 {
                                            ops_iter.next();
                                            break 'lines;
                                        }
                                    }
                                    similar::DiffOp::Replace {
                                        old_index,
                                        old_len,
                                        new_index,
                                        new_len,
                                    } => {
                                        // TODO dedup
                                        // delete
                                        {
                                            let Some((_, (_, line))) = lines_iter.peek_mut() else {
                                                break 'page;
                                            };
                                            let text = remapper
                                                .slice_old(*old_index..*old_index + *old_len)
                                                .ok_or_eyre("-")?;
                                            line.value.push(DiffOp::Delete(text.to_owned()));
                                            page_old_len += *old_len;
                                            page_ops.push(similar::DiffOp::Delete {
                                                old_index: *old_index,
                                                old_len: *old_len,
                                                new_index: *new_index,
                                            });
                                        };
                                        // advance
                                        let next_op = similar::DiffOp::Insert { old_index: *old_index, new_index: *new_index, new_len: *new_len };
                                        *op = next_op;
                                        let similar::DiffOp::Insert { old_index, new_index, new_len } = op else {
                                            unreachable!()
                                        };
                                        // insert
                                        'lines: loop {
                                            let Some((_line_idx, (new_line, line_diff))) =
                                                lines_iter.peek_mut()
                                            else {
                                                break 'page;
                                            };
                                            eprintln!("--- line ---");
                                            eprintln!("[{new_line:?}|{}]{:?}", new_line.len(), remapper.slice_new(new_line.clone()).unwrap_or("<err>"));
                                            let chunk_len = (*new_len).min(new_line.len());
                                            let chunk = remapper
                                                .slice_new(*new_index..*new_index + chunk_len)
                                                .ok_or_else(|| {
                                                    eyre!("byte index {chunk_len} is not a char boundary")
                                                        .section(format!("line: {new_line:?}"))
                                                })?;
                                            eprintln!("chunk: {chunk:?}");
                                            line_diff.value.push(DiffOp::insert(chunk.to_owned()));

                                            page_new_len += *new_len;
                                            page_ops.push(similar::DiffOp::Insert {
                                                old_index: *old_index,
                                                new_index: *new_index,
                                                new_len: chunk_len,
                                            });

                                            // advance
                                            *old_index += chunk_len;
                                            *new_index += chunk_len;
                                            *new_len -= chunk_len;

                                            new_line.start += chunk_len;
                                            if Range::<usize>::is_empty(new_line) {
                                                lines_iter.next();
                                            }
                                            if *new_len == 0 {
                                                ops_iter.next();
                                                break 'lines;
                                            }
                                        }
                                    }
                                }
                            }
                            (diffs, page_old_len, page_new_len)
                        };
                        let div = Div {
                            width,
                            height,
                            tess_box,
                        };
                        let img_name = img
                            .file_name()
                            .ok_or_else(|| eyre!("invalid img {img:?}"))?;
                        let out_file = out_dir.join(img_name).with_extension("html");
                        let mut w =
                            BufWriter::new(fs::File::create(&out_file).wrap_err_with(|| {
                                eyre!("failed to create diff file: {out_file:?}")
                            })?);
                        write!(w, "{}", Markup::from(div).into_string())?;

                        bar.set_message(format!("📊 calculating ratio {}", img.display()));
                        let ratio = get_diff_ratio(&page_ops, old_len, new_len);
                        page_ops.clear();
                        Ok((ratio, img))
                    },
                )
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

    distances.sort_unstable_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());

    for (distance, img) in distances {
        writeln!(w, "{distance:<10} {}", img.display())?;
    }
    w.flush()?;
    Ok(())
}
