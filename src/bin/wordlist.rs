use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use clap::Parser;

#[derive(Parser)]
struct Args {
    training_text: PathBuf,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let mut set = HashSet::new();
    let file = BufReader::new(File::open(args.training_text)?);
    for line in file.lines() {
        for word in line?.split_whitespace() {
            if set.contains(word) {
                continue;
            }
            set.insert(word.to_string());
        }
    }

    for word in set {
        println!("{}", word);
    }

    Ok(())
}
