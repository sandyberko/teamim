use std::{
    fs,
    io::BufRead,
    path::{Path, PathBuf},
    process::Command,
};

use clap::Parser;
use color_eyre::owo_colors::OwoColorize;
use eyre::OptionExt;
use itertools::Itertools;
use training::tess_dir;

#[derive(Debug, Parser)]
struct Args {
    input: PathBuf,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    println!("{}", "Start evaluation...".blue());
    let metadata = fs::metadata(&args.input)?;
    if metadata.is_file() {
        #[cfg(windows)]
        return Ok(eval_checkpoint(&path, false).spawn()?);

        #[cfg(unix)]
        return Err(std::os::unix::process::CommandExt::exec(&mut eval_checkpoint(
            &args.input,
            false,
        ))
        .into());
    }

    let mut checkpoints = fs::read_dir(&args.input)?
        .map(|entry| {
            let entry = entry?;
            Ok(entry.path())
        })
        .filter_map(|path| match path {
            Ok(path) => (path.extension()? == "checkpoint").then_some(eyre::Ok(path)),
            Err(err) => Some(Err(err)),
        })
        .map(|path| {
            let path = path?;
            // <model_base>_<char_error>_<learning_iteration>_<training_iteration>.checkpoint
            let stem =
                path.file_stem().ok_or_else(|| eyre::eyre!("no file stem"))?.to_string_lossy();
            let (_name, train_err, _learn_iter, _train_iter) =
                stem.split('_').collect_tuple().ok_or_eyre("invalid file stem")?;
            eyre::Ok((train_err.parse::<f32>()?, path))
        })
        .collect::<Result<Vec<_>, _>>()?;
    checkpoints.sort_by(|(a, _), (b, _)| b.partial_cmp(a).expect("finite f32"));

    for (err, path) in checkpoints {
        let res = eval_checkpoint(&path, true)
            .output()?
            .stderr
            .lines()
            .last()
            .ok_or_eyre("no result")??;
        println!("{err:<10} {res}",);
    }

    Ok(())
}

fn eval_checkpoint(path: &Path, quiet: bool) -> Command {
    let mut cmd = Command::new(tess_dir().join("lstmeval"));
    cmd.args(["--traineddata", "assets/langdata/stam/stam.traineddata"])
        .args(["--eval_listfile", "assets/training/eval.txt"])
        .args(["--verbosity", if quiet { "0" } else { "1" }])
        .arg("--model")
        .arg(path);
    cmd
}
