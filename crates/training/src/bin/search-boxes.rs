use clap::Parser;
use eyre::{Context, OptionExt, ensure};
use rayon::iter::{ParallelBridge, ParallelIterator};
use teamim::tesseract_ext::bounding_box::{parse_char_box, BoundingBox};
use std::{ffi::OsStr, fs, path::Path};

#[derive(clap::Parser)]
struct Args {
    term: String,
}

/// - Add a trailing space to each line
fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let src_dir = Path::new("assets/corrected_boxfiles");
    ensure!(src_dir.is_dir(), "{src_dir:?} is not a dir");

    fs::read_dir(src_dir)?
        .chain(fs::read_dir("../training/training/images")?)
        .par_bridge()
        .map(|entry| {
            let entry = entry?;
            let path = entry.path();

            ensure!(entry.file_type()?.is_file(), "{entry:?} is not a file");
            if path.extension() != Some(OsStr::new("box")) {
                return Ok(());
            }

            let r = fs::read_to_string(&path)?;

            let lines = r.lines().enumerate().map(|(i, line)| {
                parse_char_box(line)
                    .wrap_err_with(|| format!("invalid line: {}:{}", path.display(), i + 1))
            });

            let mut line_i = 1;
            let mut line: Option<BoundingBox<String>> = None;

            for bx in lines {
                let bx = bx?;
                if bx.value == '\t' {
                    let found = line.ok_or_eyre("empty line")?.value.contains(&args.term);
                    if found {
                        // print link
                        println!(
                            "\x1b]8;;http://localhost:3000#box/{img_name}:{line_i}\x1b\\{path}:{line_i}\x1b]8;;\x1b\\",
                            path = path.display(),
                            img_name = path.file_stem().ok_or_eyre("no stem")?.to_string_lossy().replace('_', "/"),
                        );
                    }
                    line = None;
                } else if let Some(line) = &mut line {
                    line.value.push(bx.value);
                } else {
                    line = Some(bx.with_value(bx.value.to_string()));
                    line_i += 1;
                }
            }
            eyre::Ok(())
        })
        .collect::<Result<(), _>>()?;
    Ok(())
}
