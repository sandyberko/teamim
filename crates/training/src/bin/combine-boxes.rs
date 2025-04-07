use color_eyre::owo_colors::OwoColorize;
use core::str;
use eyre::{OptionExt, bail, ensure};
use rayon::prelude::*;
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{BufWriter, Write, stderr, stdout},
    path::{Path, PathBuf},
    process::{Command, Output},
};
use training::tess_dir;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let assets_dir = Path::new("assets");

    let out_dir = assets_dir.join("training");

    let files = fs::read_dir(assets_dir.join("corrected_boxfiles"))?
        .map(|entry| {
            let entry = entry?;
            ensure!(entry.file_type()?.is_file(), "{entry:?} is not a file");
            Ok(entry.path())
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_par_iter()
        .map(|box_orig_path| {
            let stem = box_orig_path.file_stem().ok_or_eyre("no stem")?;

            let output_path = out_dir.join("combined").join(stem);
            let lstmf_path = output_path.with_extension("lstmf");

            let output = if lstmf_path.exists() {
                None
            } else {
                Some(generate_lstmf(stem, &box_orig_path, &output_path)?)
            };

            eyre::Ok((lstmf_path, output))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut list_file = BufWriter::new(File::create(out_dir.join("list.txt"))?);
    for (lstmf_path, output) in files {
        let Some(output) = output else {
            println!("{} {lstmf_path:?}", "Skipped".yellow());
            continue;
        };

        if output.status.success() {
            println!("{} {lstmf_path:?}", "Generated".blue());
        } else {
            println!("{} {lstmf_path:?}", "Failed".red());
        }

        // print output to current process
        stdout().write_all(output.stdout.as_slice())?;
        stdout().flush()?;
        stderr().write_all(output.stderr.as_slice())?;
        stderr().flush()?;

        list_file.write_all(
            lstmf_path
                .file_name()
                .ok_or_eyre("no file name")?
                .as_encoded_bytes(),
        )?;
        writeln!(list_file)?;
    }
    println!("{}", "Done!".green());

    Ok(())
}

fn generate_lstmf(
    stem: &OsStr,
    box_orig_path: &Path,
    output_path_stem: &Path,
) -> eyre::Result<Output> {
    let box_path = output_path_stem.with_extension("box");
    if !box_path.exists() {
        fs::copy(box_orig_path, &box_path)?;
    }

    let tif_path = output_path_stem.with_extension("tif");
    if !tif_path.exists() {
        let (book, page) = stem
            .to_str()
            .ok_or_eyre("invalid utf-8")?
            .split_once('_')
            .ok_or_eyre("invalid stem")?;

        let jpg_path = ["assets", "images", book, page]
            .into_iter()
            .collect::<PathBuf>()
            .with_extension("jpg");

        let output = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error"])
            .args(["-i"])
            .arg(jpg_path)
            .args(["-pix_fmt", "rgba"])
            .arg(&tif_path)
            .output()?;

        if !output.status.success() {
            bail!("ffmpeg failed:\n{}", str::from_utf8(&output.stderr)?);
        }
    }

    let output = Command::new(tess_dir().join("tesseract"))
        .arg(tif_path)
        .arg(output_path_stem)
        .arg(tess_dir().join("tessdata/configs/lstm.train"))
        .output()?;

    Ok(output)
}
