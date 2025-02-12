use serde::Serialize;
use similar::utils::TextDiffRemapper;
use similar::ChangeTag;
use similar::{Algorithm, TextDiff};

use crate::tesseract_ext::BoundingBox;

pub type BoundingBoxDiff = BoundingBox<Vec<DiffOp>>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum DiffOp {
    Equal(String),
    Insert(String),
    Delete(String),
}

/// IMPORTANT: `ocr_text_lines` should have trailing spaces
#[must_use]
pub fn diff(ocr_text_lines: &[BoundingBox<&str>], old: &str) -> Vec<BoundingBoxDiff> {
    let new: String = ocr_text_lines.iter().map(|bb| bb.value).collect();
    let (mut texts, mut diffs): (Vec<_>, Vec<_>) = ocr_text_lines
        .iter()
        .map(|bb_line| (bb_line.value, bb_line.with_value(Vec::new())))
        .unzip();

    let mut lines_iter = texts
        .iter_mut()
        .zip(diffs.iter_mut())
        .enumerate()
        .peekable();

    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .diff_chars(old, new.as_str());
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new.as_str());
    let changes = diff
        .ops()
        .iter()
        .flat_map(move |op| remapper.iter_slices(op));

    'changes: for (tag, mut change) in changes {
        match tag {
            ChangeTag::Delete => {
                let Some((_, (_, line))) = lines_iter.peek_mut() else {
                    panic!("no line for DELETE {change:?}");
                };
                eprintln!("DELETE {change:?}");
                line.value.push(DiffOp::Delete(change.to_owned()));
            }
            ChangeTag::Equal | ChangeTag::Insert => 'change: loop {
                let Some((_line_idx, (line_text, line_diff))) = lines_iter.peek_mut() else {
                    break 'changes;
                };

                let end = change.len().min(line_text.len());
                let chunk = &change[..end];
                change = &change[end..];
                **line_text = &line_text[end..];
                let op = match tag {
                    ChangeTag::Equal => {
                        eprintln!("EQUAL {chunk:?}");
                        DiffOp::Equal(chunk.to_owned())
                    }
                    ChangeTag::Insert => {
                        eprintln!("INSERT {chunk:?}");
                        DiffOp::Insert(chunk.to_owned())
                    }
                    ChangeTag::Delete => unreachable!(),
                };
                line_diff.value.push(op);

                if line_text.is_empty() {
                    lines_iter.next();
                }
                if change.is_empty() {
                    break 'change;
                }
            },
        }
    }
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff() {
        let truth_text = "hello world how are you";
        let ocr_text_lines = vec![
            BoundingBox::new("hello wrld ", 1, 2, 3, 4),
            BoundingBox::new("how are you foo ", 5, 6, 7, 8),
        ];
        let diff = diff(&ocr_text_lines, truth_text);
        assert_eq!(
            diff,
            vec![
                ocr_text_lines[0].with_value(vec![
                    DiffOp::Equal("hello w".to_owned()),
                    DiffOp::Delete("o".to_owned()),
                    DiffOp::Equal("rld ".to_owned()),
                ]),
                ocr_text_lines[1].with_value(vec![
                    DiffOp::Equal("how are you".to_owned()),
                    DiffOp::Insert(" foo ".to_owned()),
                ])
            ]
        );
    }
}
