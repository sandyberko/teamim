use std::fmt::Display;

use eyre::{OptionExt, WrapErr, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundingBox<Value> {
    pub value: Value,
    pub rect: Rect,
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
        } = self;
        write!(f, "{char} {left} {bottom} {right} {top} 0")
    }
}

impl<V> BoundingBox<V> {
    pub fn new(value: V, left: i32, bottom: i32, right: i32, top: i32) -> Self {
        Self {
            value,
            rect: Rect {
                left,
                bottom,
                right,
                top,
            },
        }
    }

    #[must_use]
    pub fn with_value<O>(&self, value: O) -> BoundingBox<O> {
        BoundingBox {
            value,
            rect: self.rect,
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

    Ok(BoundingBox { value, rect })
}
