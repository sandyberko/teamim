use std::ops::Range;

use maud::{html, Markup};
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

pub struct Div {
    pub width: u32,
    pub height: u32,
    pub tess_box: Vec<BoundingBoxDiff>,
}

impl From<Div> for Markup {
    fn from(val: Div) -> Self {
        html! {
            div #box-container style={"width: "(val.width)"px; height: "(val.height)"px;"} {
                @for tess_box in val.tess_box {
                    tess-box top=(tess_box.top) left=(tess_box.left) right=(tess_box.right) bottom=(tess_box.bottom) {
                        @for op in tess_box.value {
                            @match op {
                                DiffOp::Equal(value) => (value),
                                DiffOp::Insert { err } => insert err=(err) {},
                                DiffOp::Delete(value) => delete { (value) },
                            }
                        }
                    }
                }
            }
        }
    }
}

/// IMPORTANT: `ocr_text_lines` should have trailing spaces
#[must_use]
pub fn diff(ocr_lines: &[BoundingBox<Range<usize>>], new: &str, old: &str) -> Vec<BoundingBoxDiff> {
    let (mut texts, mut diffs): (Vec<_>, Vec<_>) = ocr_lines
        .iter()
        .map(|bb_line| (bb_line.value.clone(), bb_line.with_value(Vec::new())))
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

    eprintln!("======= PAGE ======");
    eprintln!("new:\t\t{new}");
    'changes: for (tag, mut change) in changes {
        eprintln!("===");
        eprintln!("change:\t{tag} {change:?}");
        match tag {
            ChangeTag::Delete => {
                let Some((_, (_, line))) = lines_iter.peek_mut() else {
                    break 'changes;
                };
                line.value.push(DiffOp::Delete(change.to_owned()));
            }
            ChangeTag::Equal | ChangeTag::Insert => 'lines: loop {
                let Some((_line_idx, (new_line, line_diff))) = lines_iter.peek_mut() else {
                    break 'changes;
                };

                eprintln!("---");
                eprintln!("new_line:\t{new_line:?}");
                // FIXME: it crashes upon PAGE_SEP
                eprintln!("new_line:\t{:?}", &new[new_line.clone()]);
                eprintln!("new_line_len:\t{:?}", new_line.len());

                let chunk_len = change.len().min(new_line.len());

                eprintln!("chunk_len:\t{chunk_len}");

                let chunk = &change[..chunk_len];

                eprintln!("chunk:\t\t{chunk:?}");
                eprintln!("---");

                change = &change[chunk_len..];
                new_line.start += chunk_len;
                let op = match tag {
                    ChangeTag::Equal => DiffOp::Equal(chunk.to_owned()),
                    ChangeTag::Insert => DiffOp::insert(chunk.to_owned()),
                    ChangeTag::Delete => unreachable!(),
                };
                line_diff.value.push(op);

                if Range::<usize>::is_empty(new_line) {
                    lines_iter.next();
                }
                if change.is_empty() {
                    break 'lines;
                }
            },
        }
        eprintln!("===");
    }
    diffs
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_diff() {
//         let truth_text = "hello world how are you";
//         let new = "hello wrld how are you foo ";
//         let ocr_lines = vec![
//             BoundingBox::new("hello wrld ", 1, 2, 3, 4),
//             BoundingBox::new("how are you foo ", 5, 6, 7, 8),
//         ];
//         let diff = diff(&ocr_lines, new, truth_text);
//         assert_eq!(
//             diff,
//             vec![
//                 ocr_lines[0].with_value(vec![
//                     DiffOp::Equal("hello w".to_owned()),
//                     DiffOp::Delete("o".to_owned()),
//                     DiffOp::Equal("rld ".to_owned()),
//                 ]),
//                 ocr_lines[1].with_value(vec![
//                     DiffOp::Equal("how are you".to_owned()),
//                     DiffOp::insert(" foo ".to_owned()),
//                 ])
//             ]
//         );
//     }
// }
