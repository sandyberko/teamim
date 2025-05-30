use color_eyre::Section;
use eyre::{Context, OptionExt, bail, eyre};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use itertools::Itertools;
use maud::Markup;
use rayon::prelude::*;
use similar::{Algorithm, TextDiff, get_diff_ratio, utils::TextDiffRemapper};
use std::{
    cell::RefCell,
    fs,
    io::{BufRead, BufReader, BufWriter, Write},
    iter::Peekable,
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{self, AtomicBool},
    },
    time::Duration,
};
use teamim::{
    TRAINING_TEXT, TeamimCtx,
    tesseract_ext::bounding_box::BoundingBox,
    training_diff::{self, Div},
};

use clap::Parser;

#[derive(Parser)]
struct Args {
    input_dir: PathBuf,
    out_dir: PathBuf,

    #[clap(short, long)]
    /// process a single scroll directory `input_dir`
    /// instead of a directory containing multiple scrolls
    single: bool,

    #[arg(short, long)]
    /// Hide progress bars
    quiet: bool,

    #[arg(long)]
    test_cutoff: Option<String>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    fs::create_dir_all(&args.out_dir)
        .wrap_err_with(|| eyre!("Failed to create output dir {:?}", args.out_dir))?;

    let old = if let Some(cutoff) = args.test_cutoff {
        let end = TRAINING_TEXT.find(&cutoff).ok_or_eyre("cutoff not found")?;
        &TRAINING_TEXT[..end]
    } else {
        TRAINING_TEXT
    };

    let draw_target =
        if args.quiet { ProgressDrawTarget::hidden() } else { ProgressDrawTarget::stdout() };
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
    let dirs = if args.single {
        vec![Scroll::from_dir(&args.input_dir, &bars, dir_bar_style.clone(), &args.out_dir)?]
    } else {
        fs::read_dir(args.input_dir)?
            .map(|entry| {
                Scroll::from_dir(&entry?.path(), &bars, dir_bar_style.clone(), &args.out_dir)
            })
            .collect::<eyre::Result<Vec<_>>>()?
    };
    overall_pb.set_length(
        dirs.iter().map(|scroll| scroll.imgs.as_deref().unwrap_or_default().len() as u64).sum(),
    );
    overall_pb.set_message(format!("⏳ processing {} scrolls...", dirs.len()));

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
        .into_par_iter()
        .take_any_while(|_| running.clone().load(atomic::Ordering::SeqCst))
        .map(|scroll| scroll.process(old, &overall_pb, &running))
        .collect::<Result<Vec<_>, _>>()?
        .concat();

    if running.load(atomic::Ordering::SeqCst) {
        overall_pb.finish_with_message("🏁 done");
    } else {
        overall_pb.abandon_with_message("🚩 interrupted");
    }

    if !args.single {
        write_distances(&args.out_dir, &mut distances)?;
    }

    Ok(())
}

const DISTANCES_FILE_NAME: &str = "distances.txt";
fn write_distances(out_dir: &Path, distances: &mut [(f32, PathBuf)]) -> eyre::Result<()> {
    let mut w = BufWriter::new(fs::File::create(out_dir.join(DISTANCES_FILE_NAME))?);
    distances.sort_unstable_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());
    for (distance, img) in distances {
        writeln!(w, "{distance}\t{}", img.display())?;
    }
    w.flush()?;
    Ok(())
}

thread_local! {
    static CTX: RefCell<eyre::Result<TeamimCtx>> = {
        RefCell::new((|| TeamimCtx::new()?.with_debug_file(Path::new("assets/tesseract.log")))())
    };
}

struct Scroll {
    /// `None` if the scroll has already been processed
    /// and we should load the existing distances file
    imgs: Option<Vec<PathBuf>>,
    bar: ProgressBar,
    out_dir: PathBuf,
}
impl Scroll {
    fn from_dir(
        path: &Path,
        bars: &MultiProgress,
        dir_bar_style: ProgressStyle,
        out_dir: &Path,
    ) -> eyre::Result<Self> {
        let dir_name = path
            .file_name()
            .ok_or_else(|| eyre!("invalid dir {path:?}"))?
            .to_str()
            .ok_or_else(|| eyre!("non-utf8 dir name {path:?}"))?;

        let out_dir = out_dir.join(dir_name);

        if fs::exists(&out_dir)? {
            let bar = bars.add(
                ProgressBar::no_length().with_style(dir_bar_style).with_prefix(dir_name.to_owned()),
            );
            bar.set_message("🔷 restoring...");
            return Ok(Scroll { imgs: None, bar, out_dir });
        }

        let mut imgs = fs::read_dir(path)?
            .map(|file| eyre::Ok(file?.path()))
            .filter(|path| {
                let Ok(path) = path else { return true };
                path.extension().is_some_and(|ext| ext == "jpg" || ext == "jpeg")
            })
            .collect::<Result<Vec<_>, _>>()?;
        imgs.sort_unstable();

        let bar = bars.add(
            ProgressBar::new(imgs.len() as u64)
                .with_style(dir_bar_style)
                .with_prefix(dir_name.to_owned()),
        );
        bar.set_message("📁 initialized");

        Ok(Scroll { imgs: Some(imgs), bar, out_dir })
    }

    fn process(
        self,
        old: &str,
        overall_pb: &ProgressBar,
        running: &Arc<AtomicBool>,
    ) -> eyre::Result<Vec<(f32, PathBuf)>> {
        let Some(imgs) = self.imgs else {
            // parse existing distances file
            let mut buf = String::new();
            let filename = self.out_dir.join(DISTANCES_FILE_NAME);
            let file = fs::File::open(&filename).wrap_err_with(|| {
                format!("failed to open distances file: {}", filename.display())
            })?;
            let mut reader = BufReader::new(file);
            let mut distances = Vec::new();
            while reader.read_line(&mut buf)? > 0 {
                let (distance, img) =
                    buf.split_whitespace().collect_tuple().ok_or_eyre("invalid line")?;
                let distance = distance.parse().wrap_err("invalid distance")?;
                distances.push((distance, img.into()));
                buf.clear();
            }
            self.bar.finish_with_message("🏁 restored");
            return Ok(distances);
        };

        fs::create_dir(&self.out_dir)?;
        // recognize
        let pages = imgs
            .into_par_iter()
            .take_any_while(|_| running.clone().load(atomic::Ordering::SeqCst))
            .map(|img| {
                let img_name = img.file_name().ok_or_else(|| eyre!("invalid img {img:?}"))?;
                self.bar.set_message(format!("🔍 recognizing {}", img_name.display()));

                CTX.with_borrow_mut(|ctx| -> eyre::Result<_> {
                    let ctx = ctx.as_mut().map_err(|err| eyre!("failed to get ctx: {err}"))?;

                    let mut line_start = 0;
                    let mut boxes = Vec::new();
                    let mut ocr_text = String::new();
                    let (boxes_iter, width, height) = ctx.file_boxes(&img)?;
                    for (idx, bx) in boxes_iter.enumerate() {
                        const NEWLINE: &str = "\n";
                        let Some(value) = bx.value.as_str()?.strip_suffix('\n') else {
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
        let ocr_text = pages.iter().map(|(_, ocr_text, ..)| ocr_text.as_str()).collect::<String>();
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
        let mut distances = pages
            .into_iter()
            .map(|(img, _, boxes, width, height)| -> eyre::Result<_> {
                self.bar.set_message(format!("🗺️ mapping {}", img.display()));

                let tess_box = pages_ctx.map_diff_page(&mut ops_iter, &boxes)?;
                let div = Div { width, height, tess_box };
                let img_name = img.file_name().ok_or_else(|| eyre!("invalid img {img:?}"))?;
                let out_file = self.out_dir.join(img_name).with_extension("html");
                let mut w = BufWriter::new(
                    fs::File::create(&out_file)
                        .wrap_err_with(|| eyre!("failed to create diff file: {out_file:?}"))?,
                );
                write!(w, "{}", Markup::from(div).into_string())?;

                self.bar.set_message(format!("📊 calculating ratio {}", img.display()));
                let old_len = pages_ctx.page_ops.iter().map(|op| op.old_range().len()).sum();
                let new_len = pages_ctx.page_ops.iter().map(|op| op.new_range().len()).sum();
                let ratio = get_diff_ratio(&pages_ctx.page_ops, old_len, new_len);
                pages_ctx.page_ops.clear();
                Ok((ratio, img))
            })
            .enumerate()
            .map(|(page_idx, res)| res.wrap_err_with(|| eyre!("failed diffing page #{page_idx}")))
            .collect::<Result<Vec<_>, _>>()?;

        self.bar.set_message("📃 writing distances...");
        write_distances(&self.out_dir, &mut distances)?;
        self.bar.finish_with_message("🏁 done");
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
                similar::DiffOp::Delete { old_index, old_len, new_index: _ } => {
                    let Some((_, line)) = lines_iter.peek_mut() else {
                        break 'page;
                    };
                    let text = self
                        .remapper
                        .slice_old(*old_index..*old_index + *old_len)
                        .ok_or_eyre("-")?;
                    line.value.push(training_diff::DiffOp::Delete(text.to_owned()));
                    self.page_ops.push(*op);
                    ops_iter.next();
                }
                similar::DiffOp::Equal { old_index, new_index, len: new_len }
                | similar::DiffOp::Insert { old_index, new_index, new_len } => {
                    if self.ins_eq(&mut lines_iter, tag, old_index, new_index, new_len)? {
                        break 'page;
                    }

                    ops_iter.next();
                }
                similar::DiffOp::Replace { old_index, old_len, new_index, new_len } => {
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
                        line.value.push(training_diff::DiffOp::Delete(text.to_owned()));
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
                    let similar::DiffOp::Insert { old_index, new_index, new_len } = op else {
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
            let chunk =
                self.remapper.slice_new(*new_index..*new_index + chunk_len).ok_or_else(|| {
                    eyre!("byte index {chunk_len} is not a char boundary")
                        .section(format!("line: {new_line:?}"))
                })?;

            match tag {
                similar::DiffTag::Equal => {
                    line_diff.value.push(training_diff::DiffOp::Equal(chunk.to_owned()));
                    self.page_ops.push(similar::DiffOp::Equal {
                        old_index: *old_index,
                        new_index: *new_index,
                        len: chunk_len,
                    });
                }
                similar::DiffTag::Insert => {
                    line_diff.value.push(training_diff::DiffOp::insert(chunk.to_owned()));
                    self.page_ops.push(similar::DiffOp::Insert {
                        old_index: *old_index,
                        new_index: *new_index,
                        new_len: chunk_len,
                    });
                }
                _ => unreachable!(),
            }

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
