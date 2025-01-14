use std::{io::Write, iter::once};

use strsim::normalized_levenshtein;

pub(crate) struct Location {
    line: usize,
    column: usize,
}

pub(crate) fn find(src: &str, text: &str) -> Option<usize> {
    Some(0)
    // let mut best_match = None;
    // let mut best_score = 0.0;

    // let text_char_len = text.chars().count();
    // let mut start_iter = src.char_indices();
    // let mut end_iter = src
    //     .char_indices()
    //     .skip(text_char_len)
    //     .chain(once((text.len(), '\u{0}')));

    // loop {
    //     let Some((end, _)) = end_iter.next() else {
    //         break;
    //     };
    //     let (start, _) = start_iter.next().expect("end_iter should finish first");

    //     print!("\rSearching {}%...", (start as f64 / src.len() as f64) * 100.0);
    //     std::io::stdout().flush().unwrap();

    //     let window = &src[start..end];
    //     let score = normalized_levenshtein(window, text);
    //     eprintln!("{score}");
    //     if score > best_score {
    //         best_score = score;
    //         best_match = Some(start);
    //     }
    // }

    // best_match
}
