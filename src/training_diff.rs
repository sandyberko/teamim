use similar::utils::TextDiffRemapper;
use similar::ChangeTag;
use similar::{Algorithm, TextDiff};

use crate::tesseract_ext::BoundingBox;

pub type BoundingBoxDiff = BoundingBox<Vec<DiffOp>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffOp {
    Equal(String),
    Insert { err: String },
    Delete(String),
}

impl DiffOp {
    #[must_use]
    pub fn insert(err: String) -> Self {
        DiffOp::Insert { err }
    }
}

/// IMPORTANT: `ocr_text_lines` should have trailing spaces
#[must_use]
pub fn diff(ocr_lines: &[BoundingBox<&str>], new: &str, old: &str) -> Vec<BoundingBoxDiff> {
    let (mut texts, mut diffs): (Vec<_>, Vec<_>) = ocr_lines
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
        .diff_chars(old, new);
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
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
                    ChangeTag::Equal => DiffOp::Equal(chunk.to_owned()),
                    ChangeTag::Insert => DiffOp::insert(chunk.to_owned()),
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
        let new = "hello wrld how are you foo ";
        let ocr_lines = vec![
            BoundingBox::new("hello wrld ", 1, 2, 3, 4),
            BoundingBox::new("how are you foo ", 5, 6, 7, 8),
        ];
        let diff = diff(&ocr_lines, new, truth_text);
        assert_eq!(
            diff,
            vec![
                ocr_lines[0].with_value(vec![
                    DiffOp::Equal("hello w".to_owned()),
                    DiffOp::Delete("o".to_owned()),
                    DiffOp::Equal("rld ".to_owned()),
                ]),
                ocr_lines[1].with_value(vec![
                    DiffOp::Equal("how are you".to_owned()),
                    DiffOp::insert(" foo ".to_owned()),
                ])
            ]
        );
    }
}
