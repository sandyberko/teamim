use eyre::bail;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use rayon::prelude::*;
use std::{
    cell::{OnceCell, RefCell},
    cmp::Reverse,
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
};
use strsim::levenshtein;
use teamim::{find_truth_text, TeamimCtx};

use clap::Parser;

#[derive(Parser)]
struct Args {
    input_dir: PathBuf,
    out_file: PathBuf,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let files = handle_dir(args.input_dir)?;

    let bars = MultiProgress::with_draw_target(ProgressDrawTarget::stdout());
    let overall_pb = bars.add(ProgressBar::new(files.len() as u64));
    overall_pb.set_style(ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )?);

    thread_local! {
        static CTX: RefCell<eyre::Result<TeamimCtx>> = RefCell::new(TeamimCtx::new());
        static BAR: OnceCell<ProgressBar> = const { OnceCell::new() };
    }

    let running = Arc::new(AtomicBool::new(true));
    ctrlc::set_handler({
        let running = running.clone();
        move || {
            running.store(false, atomic::Ordering::SeqCst);
        }
    })?;

    let mut distances = files
        .par_iter()
        .take_any_while(|_| running.clone().load(atomic::Ordering::SeqCst))
        .map(|img| {
            BAR.with(|bar| {
                let bar =
                    bar.get_or_init(|| bars.insert_before(&overall_pb, ProgressBar::new_spinner()));
                bar.set_message(format!("🔍 recognizing {}", img.display()));
                bar.tick();
            });
            let ocr_text = CTX.with_borrow_mut(|ctx| {
                let ctx = match ctx {
                    Ok(ctx) => ctx,
                    Err(err) => bail!("failed to init context {err:?}"),
                };

                ctx.file_text(img)
            })?;
            let ocr_text = ocr_text.as_str()?.replace('\n', " ");

            BAR.with(|bar| {
                bar.get()
                    .unwrap()
                    .set_message(format!("🔗 checking {}", img.display()));
            });

            let distance = find_truth_text(&ocr_text)
                .map_or(usize::MAX, |truth_text| levenshtein(truth_text, &ocr_text));

            BAR.with(|bar| {
                bar.get()
                    .unwrap()
                    .set_message(format!("🏁 done {}", img.display()));
            });
            overall_pb.inc(1);

            Ok((img, distance))
        })
        .collect::<eyre::Result<Vec<_>>>()?;

    let mut w = BufWriter::new(fs::File::create(args.out_file)?);

    if running.load(atomic::Ordering::SeqCst) {
        overall_pb.finish_with_message("🏁 done");
    } else {
        writeln!(w, "<interrupted>")?;
        overall_pb.abandon_with_message("🚩 interrupted");
    }

    distances.sort_unstable_by_key(|(_, distance)| Reverse(*distance));

    for (img, distance) in distances {
        if distance == usize::MAX {
            writeln!(w, "<not found>\t\t\t{img}", img = img.display())?;
        } else {
            writeln!(w, "{distance}\t\t\t{img}", img = img.display())?;
        }
    }
    w.flush()?;
    Ok(())
}

fn handle_dir_entry(entry: &fs::DirEntry) -> eyre::Result<Vec<PathBuf>> {
    let path = entry.path();
    if entry.file_type()?.is_dir() {
        handle_dir(path)
    } else if path
        .extension()
        .is_some_and(|ext| ext == "jpg" || ext == "jpeg")
    {
        Ok(vec![path])
    } else {
        eprintln!("skipping {path:?}");
        Ok(vec![])
    }
}

fn handle_dir(path: PathBuf) -> Result<Vec<PathBuf>, eyre::Error> {
    Ok(fs::read_dir(path)?
        .map(|entry| Ok(handle_dir_entry(&entry?)))
        .collect::<eyre::Result<Vec<_>>>()?
        .into_iter()
        .collect::<eyre::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}
