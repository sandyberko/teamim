fn main() {}

use similar::{utils::TextDiffRemapper, ChangeTag, TextDiff};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DiffOp {
    Equal(String),
    Insert { err: String },
    Delete(String),
}

impl DiffOp {
    pub fn insert(err: String) -> Self {
        DiffOp::Insert { err }
    }
}

/// Given:
/// - `old`: a single “ground truth” string (built by joining all pages’ lines with a space)
/// - `new_pages_lines`: a slice of pages, each a vector of lines (each ending with a newline)
///
/// This function:
/// 1. Trims the new lines and joins them with a space to form the “new” string.
/// 2. Computes a character-level diff between `old` and the new string using the similar crate.
/// 3. “Replays” the diff so that the resulting diff operations are split according to the original
///    pages/lines (i.e. so that when you remove the inserted and deleted parts you recover the new pages’ lines).
#[allow(clippy::too_many_lines)]
pub fn diff_page_lines(old: &str, new_pages_lines: &[Vec<String>]) -> Vec<Vec<Vec<DiffOp>>> {
    // Flatten new_pages_lines into a list of trimmed lines,
    // and also record how many lines per page.
    let mut new_lines = Vec::new();
    let mut lines_per_page = Vec::new();
    for page in new_pages_lines {
        lines_per_page.push(page.len());
        for line in page {
            new_lines.push(line.trim_end().to_owned());
        }
    }
    // Build the new “full” string by joining lines with a single space.
    let new_concat = new_lines.join(" ");

    // Compute (start, end) ranges for each new line within new_concat.
    // (These boundaries come from the fact that old was built by joining lines with a space.)
    let mut line_ranges = Vec::new();
    let mut pos = 0;
    for (i, line) in new_lines.iter().enumerate() {
        let start = pos;
        let end = pos + line.len();
        line_ranges.push((start, end));
        if i < new_lines.len() - 1 {
            pos = end + 1; // account for the separator space
        } else {
            pos = end;
        }
    }

    // Compute a character diff between old and new_concat.
    let diff = TextDiff::from_chars(old, &new_concat);

    // We will “simulate” consuming the new_concat string.
    // For Equal and Insert changes we advance a pointer (new_idx).
    // For Delete changes we simply attach them to the current line.
    let mut line_diffs: Vec<Vec<DiffOp>> = Vec::new();
    let mut current_ops: Vec<DiffOp> = Vec::new();
    // current_line indexes line_ranges.
    let mut current_line = 0;
    // new_idx is the index (in new_concat) we have “consumed” so far.
    let mut new_idx = 0;

    // Helper: flush the current line’s ops and move to the next line.
    let mut flush_line = |line_diffs: &mut Vec<Vec<DiffOp>>,
                          current_ops: &mut Vec<DiffOp>,
                          current_line: &mut usize,
                          new_idx: &mut usize,
                          new_concat: &str,
                          line_ranges: &[(usize, usize)]| {
        line_diffs.push(current_ops.clone());
        current_ops.clear();
        *current_line += 1;
        // If the next character in new_concat is the separator, skip it.
        if *current_line < line_ranges.len() && *new_idx < new_concat.len() {
            if &new_concat[*new_idx..*new_idx + 1] == " " {
                *new_idx += 1;
            }
        }
    };

    let remapper = TextDiffRemapper::from_text_diff(&diff, old, &new_concat);
    let changes = diff
        .ops()
        .iter()
        .flat_map(move |op| remapper.iter_slices(op));

    // Process each change from the diff.
    for (tag, value) in changes {
        // Before processing a change, if our new_idx is exactly at the end of the current line,
        // flush the current ops (i.e. we have “filled” the line).
        while current_line < line_ranges.len() && new_idx == line_ranges[current_line].1 {
            flush_line(
                &mut line_diffs,
                &mut current_ops,
                &mut current_line,
                &mut new_idx,
                &new_concat,
                &line_ranges,
            );
        }
        match tag {
            ChangeTag::Delete => {
                // A deletion does not consume any new characters.
                current_ops.push(DiffOp::Delete(value.to_owned()));
            }
            ChangeTag::Equal | ChangeTag::Insert => {
                // For these changes, we must “consume” new text.
                let is_equal = tag == ChangeTag::Equal;
                let mut text = value;
                while !text.is_empty() {
                    // If we have run out of lines, simply attach the remainder.
                    if current_line >= line_ranges.len() {
                        if is_equal {
                            current_ops.push(DiffOp::Equal(text.to_owned()));
                        } else {
                            current_ops.push(DiffOp::Insert {
                                err: text.to_owned(),
                            });
                        }
                        new_idx += text.len();
                        text = "";
                        break;
                    }
                    let (_, line_end) = line_ranges[current_line];
                    // If we are exactly at the end of the current line, flush it.
                    if new_idx == line_end {
                        flush_line(
                            &mut line_diffs,
                            &mut current_ops,
                            &mut current_line,
                            &mut new_idx,
                            &new_concat,
                            &line_ranges,
                        );
                        continue;
                    }
                    // How many characters remain in the current line?
                    let available = line_end - new_idx;
                    if text.len() <= available {
                        // The entire text fits in the current line.
                        if is_equal {
                            current_ops.push(DiffOp::Equal(text.to_owned()));
                        } else {
                            current_ops.push(DiffOp::Insert {
                                err: text.to_owned(),
                            });
                        }
                        new_idx += text.len();
                        text = "";
                    } else {
                        // Only a part of text fits; split and flush the line.
                        let (part, rest) = text.split_at(available);
                        if is_equal {
                            current_ops.push(DiffOp::Equal(part.to_owned()));
                        } else {
                            current_ops.push(DiffOp::Insert {
                                err: part.to_owned(),
                            });
                        }
                        new_idx += available; // now new_idx equals line_end
                        text = rest;
                        flush_line(
                            &mut line_diffs,
                            &mut current_ops,
                            &mut current_line,
                            &mut new_idx,
                            &new_concat,
                            &line_ranges,
                        );
                    }
                }
            }
        }
    }
    // Flush the final (possibly partially filled) line.
    if current_line < line_ranges.len() {
        line_diffs.push(current_ops);
        current_line += 1;
    }
    // If there are any remaining lines (unlikely), add empty vectors.
    while current_line < line_ranges.len() {
        line_diffs.push(Vec::new());
        current_line += 1;
    }

    // Now reassemble the per–line diff results into pages.
    let mut pages = Vec::new();
    let mut line_iter = line_diffs.into_iter();
    for &page_line_count in &lines_per_page {
        let mut page = Vec::new();
        for _ in 0..page_line_count {
            if let Some(line_diff) = line_iter.next() {
                page.push(line_diff);
            }
        }
        pages.push(page);
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_diff() {
        let old = "hello world how are you bla bla";
        let new = &[
            vec!["hello wrld\n".to_owned(), "how are you foo\n".to_owned()],
            vec!["bla bla\n".to_owned()],
        ];
        let diff = diff_page_lines(old, new);
        assert_eq!(
            diff,
            vec![
                // page 0
                vec![
                    vec![
                        DiffOp::Equal("hello w".to_owned()),
                        DiffOp::Delete("o".to_owned()),
                        DiffOp::Equal("rld".to_owned()),
                    ],
                    vec![
                        DiffOp::Equal("how are you".to_owned()),
                        DiffOp::Insert {
                            err: " foo".to_owned()
                        },
                    ]
                ],
                // page 1
                vec![vec![DiffOp::Equal("bla bla".to_owned())]]
            ]
        );
    }
}
