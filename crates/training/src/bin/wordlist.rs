use std::{collections::HashSet, fs, path::PathBuf};

use clap::Parser;

#[derive(Parser)]
struct Args {
    training_text: PathBuf,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let mut set = HashSet::new();
    let file = fs::read_to_string(args.training_text)?;
    for word in file.split(' ') {
        if set.contains(word) {
            continue;
        }
        set.insert(word.to_string());
    }

    for word in set {
        println!("{word}");
    }

    Ok(())
}
