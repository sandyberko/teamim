use std::{collections::VecDeque, fmt::Display};

use eyre::{OptionExt, WrapErr, ensure};

pub const LINE_TERMINATOR: char = '\t';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// NOTE: origin is at **bottom** left
pub struct Rect {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

impl Rect {
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            left: self.left.min(other.left),
            bottom: self.bottom.min(other.bottom),
            right: self.right.max(other.right),
            top: self.top.max(other.top),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundingBox<Value> {
    pub value: Value,
    pub rect: Rect,
    pub page: usize,
}

impl<V: Display> Display for BoundingBox<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            value: char,
            rect:
                Rect {
                    left,
                    bottom,
                    right,
                    top,
                },
            page,
        } = self;
        write!(f, "{char} {left} {bottom} {right} {top} {page}")
    }
}

impl<V> BoundingBox<V> {
    pub fn new_paged(value: V, left: i32, bottom: i32, right: i32, top: i32, page: usize) -> Self {
        Self {
            value,
            rect: Rect {
                left,
                bottom,
                right,
                top,
            },
            page,
        }
    }

    pub fn new(value: V, left: i32, bottom: i32, right: i32, top: i32) -> Self {
        Self::new_paged(value, left, bottom, right, top, 0)
    }

    #[must_use]
    pub fn with_value<O>(&self, value: O) -> BoundingBox<O> {
        BoundingBox {
            value,
            rect: self.rect,
            page: self.page,
        }
    }

    #[must_use]
    pub fn map_value<O, F: FnOnce(V) -> O>(self, f: F) -> BoundingBox<O> {
        BoundingBox {
            value: f(self.value),
            rect: self.rect,
            page: self.page,
        }
    }

    #[must_use]
    pub fn into_bottom_left(self, img_h: i32) -> Self {
        Self {
            value: self.value,
            rect: Rect {
                left: self.rect.left,
                bottom: img_h - self.rect.bottom,
                right: self.rect.right,
                top: img_h - self.rect.top,
            },
            page: self.page,
        }
    }
}

pub fn parse_char_box(line: &str) -> eyre::Result<BoundingBox<char>> {
    // `char` needs special parsing since it can just be a space itself
    let mut line = line.chars();
    let value = line.next().ok_or_eyre("expected char")?;
    ensure!(line.next() == Some(' '), "expected space");

    let mut parts = line.as_str().split(' ');
    let mut parse_part = || eyre::Ok(parts.next().ok_or_eyre("unexpected end")?.parse()?);

    let rect = Rect {
        left: parse_part().wrap_err("failed to parse left")?,
        bottom: parse_part().wrap_err("failed to parse bottom")?,
        right: parse_part().wrap_err("failed to parse right")?,
        top: parse_part().wrap_err("failed to parse top")?,
    };
    let page = parse_part()
        .wrap_err("failed to parse page")?
        .try_into()
        .wrap_err("page out of i32 range")?;

    Ok(BoundingBox { value, rect, page })
}

struct LineBoxIter<I> {
    iter: I,
}

impl<Item, Iter> Iterator for LineBoxIter<Iter>
where
    Item: AsRef<str>,
    Iter: Iterator<Item = eyre::Result<Item>>,
{
    type Item = eyre::Result<BoundingBox<String>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0u8; 4];
        let mut bx_accum: Option<BoundingBox<VecDeque<u8>>> = None;
        loop {
            let Some(line) = self.iter.next() else {
                break;
            };

            let line = match line {
                Ok(line) => line,
                Err(e) => return Some(Err(e)),
            };
            let bx = match parse_char_box(line.as_ref()) {
                Ok(bx) => bx,
                Err(e) => return Some(Err(e)),
            };

            if bx.value == LINE_TERMINATOR {
                break;
            }

            let bx_accum = bx_accum.get_or_insert_with(|| bx.with_value(VecDeque::new()));

            bx_accum.rect = bx_accum.rect.union(&bx.rect);

            for &byte in bx.value.encode_utf8(&mut buf).as_bytes().iter().rev() {
                bx_accum.value.push_front(byte);
            }
        }

        bx_accum.map(|bx| {
            Ok(bx.map_value(|value| {
                let value = Vec::from(value);
                // SAFETY: VecDeque was created using chars, so it's valid UTF-8
                unsafe { String::from_utf8_unchecked(value) }
            }))
        })
    }
}

pub fn parse_line_boxes<Item: AsRef<str>>(
    lines: impl IntoIterator<Item = eyre::Result<Item>>,
) -> impl Iterator<Item = eyre::Result<BoundingBox<String>>> {
    LineBoxIter {
        iter: lines.into_iter(),
    }
}
