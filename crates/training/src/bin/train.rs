use clap::Parser;
use color_eyre::owo_colors::OwoColorize;
use std::{path::PathBuf, process::Command};
use training::tess_dir;

#[derive(Debug, Parser)]
struct Args {
    #[clap(short, long)]
    continue_from: Option<PathBuf>,

    #[clap(short, long)]
    /// Weight factor for new deltas. (type:double default:0.001)
    learning_rate: Option<f64>,

    #[clap(short, long)]
    verbose: bool,

    #[clap(short, long)]
    /// How many imperfect samples between perfect ones
    perfect_sample_delay: Option<u32>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    #[cfg(windows)]
    {
        use eyre::WrapErr;

        Command::new("cmd")
            .args(["-c", "chcp 65001"])
            .status()
            .wrap_err("failed to set codepage")?;
    }

    println!("{}", "Start training...".blue());
    let mut cmd = Command::new(tess_dir().join("lstmtraining"));
    cmd.args(["--old_traineddata", "assets/langdata/heb/heb.traineddata"])
        .args(["--traineddata", "assets/langdata/stam/stam.traineddata"])
        .args(["--continue_from", "assets/langdata/heb/heb.lstm"])
        .args(["--train_listfile", "assets/training/train.txt"])
        .args(["--eval_listfile", "assets/training/eval.txt"])
        .args(["--model_output", "assets/training/model/stam"]);

    if let Some(lr) = args.learning_rate {
        cmd.arg("--learning_rate").arg(lr.to_string());
    }

    if let Some(continue_from) = args.continue_from {
        cmd.arg("--continue_from").arg(continue_from);
    }

    if let Some(perfect_sample_delay) = args.perfect_sample_delay {
        cmd.arg("--perfect_sample_delay").arg(perfect_sample_delay.to_string());
    }

    if args.verbose {
        cmd.arg("--debug_interval").arg("-1");
    }

    #[cfg(windows)]
    {
        cmd.spawn()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        return Err(cmd.exec().into());
    }

    Ok(())
}
