use std::{cmp::Reverse, collections::HashMap, fs, num::NonZero, path::PathBuf};

use clap::Parser;

#[derive(Parser)]
struct Args {
    training_text: PathBuf,
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let mut words = HashMap::<&str, NonZero<usize>>::new();
    let file = fs::read_to_string(args.training_text)?;
    for word in file.split(' ') {
        words
            .entry(word)
            .and_modify(|e| *e = e.saturating_add(1))
            .or_insert(NonZero::new(1).unwrap());
    }

    let mut words = words.into_iter().collect::<Vec<_>>();
    words.sort_unstable_by_key(|(_, count)| Reverse(count.get()));

    for (word, _) in words {
        println!("{word}");
    }

    Ok(())
}
