use clap::{Parser, ValueEnum};
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
use teamim::tesseract_ext::PageSegMode;
use training::{tess_dir, tessdata_dir};

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum InputType {
    Synthetic,
    Real,
}

#[derive(Debug, Parser)]
struct Args {
    kind: InputType,
    path: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let assets_dir = Path::new("assets");

    let out_dir = assets_dir.join("training");

    let files = match args.kind {
        InputType::Synthetic => {
            if let Some(path) = args.path {
                vec![process_synth(&out_dir, &path)?]
            } else {
                fs::read_dir(assets_dir.join("training/images"))?
                    .par_bridge()
                    .filter_map(|entry| match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            if path.extension().is_some_and(|ext| ext == "tif") {
                                Some(Ok(path))
                            } else {
                                None
                            }
                        }
                        Err(err) => Some(Err(err)),
                    })
                    .map(|path| {
                        let path = path?;
                        process_synth(&out_dir, &path)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        }
        InputType::Real => {
            let outs = if let Some(path) = args.path {
                vec![process_real(&out_dir, &path)?]
            } else {
                fs::read_dir(assets_dir.join("corrected_boxfiles"))?
                    .map(|entry| {
                        let entry = entry?;
                        ensure!(entry.file_type()?.is_file(), "{entry:?} is not a file");
                        Ok(entry.path())
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_par_iter()
                    .map(|box_orig_path| process_real(&out_dir, &box_orig_path))
                    .collect::<Result<Vec<_>, _>>()?
            };
            outs.into_iter()
                .map(|(path, output)| {
                    if output.status.success() {
                        println!("{} {path:?}", "Generated".blue());
                    } else {
                        println!("{} {path:?}", "Failed".red());
                    }

                    // print output to current process
                    stdout().write_all(output.stdout.as_slice())?;
                    stdout().flush()?;
                    stderr().write_all(output.stderr.as_slice())?;
                    stderr().flush()?;
                    eyre::Ok(path)
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };

    let mut list_file =
        BufWriter::new(File::options().append(true).create(true).open(out_dir.join("list.txt"))?);
    for lstmf_path in files {
        list_file
            .write_all(lstmf_path.file_name().ok_or_eyre("no file name")?.as_encoded_bytes())?;
        writeln!(list_file)?;
    }
    println!("{}", "Done!".green());

    Ok(())
}

fn process_real(out_dir: &Path, box_orig_path: &Path) -> eyre::Result<(PathBuf, Output)> {
    let stem = box_orig_path.file_stem().ok_or_eyre("no stem")?;

    let output_path_stem = out_dir.join("combined").join(stem);
    let lstmf_path = output_path_stem.with_extension("lstmf");

    if !lstmf_path.exists() {
        prepare_lstmf(stem, box_orig_path, &output_path_stem)?;
    }

    let output = generate_lstmf_cmd(
        &output_path_stem,
        &output_path_stem.with_extension("tif"),
        PageSegMode::SingleColumn,
    )
    .output()?;

    eyre::Ok((lstmf_path, output))
}

fn process_synth(out_dir: &Path, path: &Path) -> eyre::Result<PathBuf> {
    let stem = path.file_stem().ok_or_eyre("no stem")?;
    let output_path_stem = out_dir.join("lstmf").join(stem);
    let log_file = File::create(out_dir.join("logs").join(stem).with_extension("log"))?;
    generate_lstmf_cmd(&output_path_stem, path, PageSegMode::SingleBlock)
        .stderr(log_file.try_clone()?)
        .stdout(log_file)
        .spawn()?
        .wait()?;
    eyre::Ok(output_path_stem.with_extension("lstmf"))
}

fn prepare_lstmf(stem: &OsStr, box_orig_path: &Path, output_path_stem: &Path) -> eyre::Result<()> {
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

        let jpg_path =
            ["assets", "images", book, page].into_iter().collect::<PathBuf>().with_extension("jpg");

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

    Ok(())
}

fn generate_lstmf_cmd(output_path_stem: &Path, tif_path: &Path, psm: PageSegMode) -> Command {
    let mut cmd = Command::new(tess_dir().join("tesseract"));
    cmd.args(["-l", "stam"])
        .args(["--tessdata-dir", "assets/tessdata"])
        .args(["--psm", &(psm as u32).to_string()])
        .arg(tif_path)
        .arg(output_path_stem)
        .arg(tessdata_dir().join("configs/lstm.train"));
    cmd
}
