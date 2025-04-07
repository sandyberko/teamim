use color_eyre::owo_colors::OwoColorize;
use eyre::{OptionExt, ensure};
use rayon::prelude::*;
use std::{fs, path::Path, process::Command};

use training::tess_dir;

const ROOT_DIR: &str = "../training";

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let root_dir = Path::new(ROOT_DIR);

    let out_dir = root_dir.join("out");
    ensure!(out_dir.is_dir(), "{out_dir:?} is not a dir");

    let lang_name = "eng";
    let eval_list_file = root_dir.join("eval/list.txt");

    let traineddata_path = root_dir
        .join("lang_model")
        .join("stam")
        .join("stam.traineddata");

    fs::read_dir(out_dir)?
        .collect::<Result<Vec<_>, _>>()?
        .par_iter()
        .filter(|path| {
            path.path()
                .extension()
                .is_some_and(|ext| ext == "checkpoint")
        })
        .map(|entry| {
            let output = Command::new(tess_dir().join("lstmeval"))
                .arg("--model")
                .arg(entry.path())
                .arg("--traineddata")
                .arg(&traineddata_path)
                .arg("--eval_listfile")
                .arg(&eval_list_file)
                .arg("--verbosity")
                .arg("0")
                .output()?;

            if !output.status.success() {
                eprintln!("lstmeval failed: {output:?}");
            }

            
            eyre::Ok(())
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}
