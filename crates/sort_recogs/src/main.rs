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
    iter::Peekable,
    ops::Range,
    path::PathBuf,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    thread::LocalKey,
    time::Duration,
};
use teamim::{
    tesseract_ext::BoundingBox,
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

    #[arg(long)]
    test_cutoff: Option<String>,
}

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
            let mut imgs = fs::read_dir(&path)?
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
            imgs.sort_unstable();

            let dir_name = path
                .file_name()
                .ok_or_else(|| eyre!("invalid dir {path:?}"))?
                .to_str()
                .ok_or_else(|| eyre!("non-utf8 dir name {path:?}"))?;

            let bar = bars.insert_before(
                &overall_pb,
                ProgressBar::new(imgs.len() as u64)
                    .with_style(dir_bar_style.clone())
                    .with_prefix(dir_name.to_owned()),
            );

            let out_dir = args.out_dir.join(dir_name);

            Ok(Scroll { imgs, bar, out_dir })
        })
        .collect::<eyre::Result<Vec<_>>>()?;

    overall_pb.set_length(dirs.iter().map(|scroll| scroll.imgs.len() as u64).sum());

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
        .map(|scroll| scroll.process(old, &overall_pb, &running, &CTX))
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

struct Scroll {
    imgs: Vec<PathBuf>,
    bar: ProgressBar,
    out_dir: PathBuf,
}
impl Scroll {
    fn process(
        &self,
        old: &str,
        overall_pb: &ProgressBar,
        running: &Arc<AtomicBool>,
        ctx: &'static LocalKey<RefCell<Option<TeamimCtx>>>,
    ) -> eyre::Result<Vec<(f32, &PathBuf)>> {
        fs::create_dir(&self.out_dir)?;
        // recognize
        let pages = self
            .imgs
            .par_iter()
            .take_any_while(|_| running.clone().load(atomic::Ordering::SeqCst))
            .map(|img| {
                let img_name = img
                    .file_name()
                    .ok_or_else(|| eyre!("invalid img {img:?}"))?;
                self.bar.set_message(format!("🔍 recognizing {img_name:?}"));
                self.bar.tick();

                ctx.with_borrow_mut(|ctx| -> eyre::Result<_> {
                    let ctx = if let Some(ctx) = ctx.as_mut() {
                        ctx
                    } else {
                        let new = TeamimCtx::new()?
                            .with_debug_file(self.out_dir.join("tesseract.log"))?;
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
                        boxes.push(bx.with_value(range));
                        line_start += value_len;
                    }
                    self.bar.inc(1);
                    overall_pb.inc(1);
                    Ok((img, ocr_text, boxes, width, height))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !running.load(atomic::Ordering::SeqCst) {
            self.bar.abandon_with_message("🚩 interrupted");
            return Ok(vec![]);
        }
        self.bar.set_message("± diffing...");
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
        let mut pages_ctx = RemapCtx {
            remapper: TextDiffRemapper::from_text_diff(&diff, old, &ocr_text),
            page_ops: Vec::new(),
        };
        let ops = diff.ops().to_vec();
        let mut ops_iter = ops.iter().copied().peekable();
        // mapping
        let distances = pages
            .into_iter()
            .map(|(img, _, boxes, width, height)| -> eyre::Result<_> {
                self.bar
                    .set_message(format!("🗺️ mapping {}", img.display()));

                let tess_box = pages_ctx.map_diff_page(&mut ops_iter, &boxes)?;
                let div = Div {
                    width,
                    height,
                    tess_box,
                };
                let img_name = img
                    .file_name()
                    .ok_or_else(|| eyre!("invalid img {img:?}"))?;
                let out_file = self.out_dir.join(img_name).with_extension("html");
                let mut w = BufWriter::new(
                    fs::File::create(&out_file)
                        .wrap_err_with(|| eyre!("failed to create diff file: {out_file:?}"))?,
                );
                write!(w, "{}", Markup::from(div).into_string())?;

                self.bar
                    .set_message(format!("📊 calculating ratio {}", img.display()));
                let old_len = pages_ctx
                    .page_ops
                    .iter()
                    .map(|op| op.old_range().len())
                    .sum();
                let new_len = pages_ctx
                    .page_ops
                    .iter()
                    .map(|op| op.new_range().len())
                    .sum();
                let ratio = get_diff_ratio(&pages_ctx.page_ops, old_len, new_len);
                pages_ctx.page_ops.clear();
                Ok((ratio, img))
            })
            .enumerate()
            .map(|(page_idx, res)| res.wrap_err_with(|| eyre!("failed diffing page #{page_idx}")))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(distances)
    }
}

struct RemapCtx<'s> {
    remapper: TextDiffRemapper<'s, str>,
    page_ops: Vec<similar::DiffOp>,
}

impl RemapCtx<'_> {
    fn map_diff_page(
        &mut self,
        ops_iter: &mut Peekable<impl Iterator<Item = similar::DiffOp>>,
        ocr_lines: &[BoundingBox<Range<usize>>],
    ) -> eyre::Result<Vec<BoundingBox<Vec<training_diff::DiffOp>>>> {
        let (mut texts, mut diffs): (Vec<_>, Vec<_>) = ocr_lines
            .iter()
            .map(|bb_line| (bb_line.value.clone(), bb_line.with_value(Vec::new())))
            .unzip();
        let mut lines_iter = texts.iter_mut().zip(diffs.iter_mut()).peekable();
        'page: while let Some(op) = ops_iter.peek_mut() {
            let tag = op.tag();
            match op {
                similar::DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index: _,
                } => {
                    let Some((_, line)) = lines_iter.peek_mut() else {
                        break 'page;
                    };
                    let text = self
                        .remapper
                        .slice_old(*old_index..*old_index + *old_len)
                        .ok_or_eyre("-")?;
                    line.value
                        .push(training_diff::DiffOp::Delete(text.to_owned()));
                    self.page_ops.push(*op);
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
                } => {
                    if self.ins_eq(&mut lines_iter, tag, old_index, new_index, new_len)? {
                        break 'page;
                    }

                    ops_iter.next();
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
                        let Some((_, line)) = lines_iter.peek_mut() else {
                            break 'page;
                        };
                        let text = self
                            .remapper
                            .slice_old(*old_index..*old_index + *old_len)
                            .ok_or_eyre("-")?;
                        line.value
                            .push(training_diff::DiffOp::Delete(text.to_owned()));
                        self.page_ops.push(similar::DiffOp::Delete {
                            old_index: *old_index,
                            old_len: *old_len,
                            new_index: *new_index,
                        });
                    };
                    // advance
                    let next_op = similar::DiffOp::Insert {
                        old_index: *old_index,
                        new_index: *new_index,
                        new_len: *new_len,
                    };
                    *op = next_op;
                    // insert
                    let similar::DiffOp::Insert {
                        old_index,
                        new_index,
                        new_len,
                    } = op
                    else {
                        unreachable!()
                    };
                    if self.ins_eq(
                        &mut lines_iter,
                        similar::DiffTag::Insert,
                        old_index,
                        new_index,
                        new_len,
                    )? {
                        break 'page;
                    }

                    ops_iter.next();
                }
            }
        }
        Ok(diffs)
    }

    fn ins_eq<'ls>(
        &mut self,
        lines_iter: &mut Peekable<
            impl Iterator<
                Item = (
                    &'ls mut Range<usize>,
                    &'ls mut BoundingBox<Vec<teamim::training_diff::DiffOp>>,
                ),
            >,
        >,
        tag: similar::DiffTag,
        old_index: &mut usize,
        new_index: &mut usize,
        new_len: &mut usize,
    ) -> eyre::Result<bool> {
        loop {
            let Some((new_line, line_diff)) = lines_iter.peek_mut() else {
                return Ok(true);
            };
            let chunk_len = (*new_len).min(new_line.len());
            let chunk = self
                .remapper
                .slice_new(*new_index..*new_index + chunk_len)
                .ok_or_else(|| {
                    eyre!("byte index {chunk_len} is not a char boundary")
                        .section(format!("line: {new_line:?}"))
                })?;

            match tag {
                similar::DiffTag::Equal => {
                    line_diff
                        .value
                        .push(training_diff::DiffOp::Equal(chunk.to_owned()));
                    self.page_ops.push(similar::DiffOp::Equal {
                        old_index: *old_index,
                        new_index: *new_index,
                        len: chunk_len,
                    });
                }
                similar::DiffTag::Insert => {
                    line_diff
                        .value
                        .push(training_diff::DiffOp::insert(chunk.to_owned()));
                    self.page_ops.push(similar::DiffOp::Insert {
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
                return Ok(false);
            }
        }
    }
}
