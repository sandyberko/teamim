use color_eyre::owo_colors::OwoColorize;
use std::process::Command;
use training::tess_dir;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    #[cfg(windows)]
    {
        use eyre::WrapErr;

        Command::new("cmd")
            .args(["-c", "chcp 65001"])
            .status()
            .wrap_err("failed to set codepage")?;
    }

    println!("{}", "Start training...".blue());
    Command::new(tess_dir().join("lstmtraining"))
        .args(["--old_traineddata", "assets/langdata/heb/heb.traineddata"])
        .args(["--traineddata", "assets/langdata/stam/stam.traineddata"])
        .args(["--continue_from", "assets/langdata/heb/heb.lstm"])
        .args(["--train_listfile", "assets/training/train.txt"])
        .args(["--eval_listfile", "assets/training/eval.txt"])
        .args(["--model_output", "assets/training/model/stam"])
        .spawn()?;

    Ok(())
}
