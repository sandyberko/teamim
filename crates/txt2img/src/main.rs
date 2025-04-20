use std::{collections::VecDeque, fs::File, io::BufWriter, iter::Peekable, path::PathBuf};

use ab_glyph::{Font, FontRef};
use clap::Parser;
use eyre::bail;
use imageproc::{
    drawing::{draw_text_mut, text_size},
    image::{GrayImage, Luma},
    noise::gaussian_noise_mut,
};
use teamim::TRAINING_TEXT;
use tiff::encoder::{TiffEncoder, colortype::Gray8};

#[derive(Debug, Parser)]
struct Args {
    txt_path: PathBuf,
    output: PathBuf,
}

const LINE_COUNT: u32 = 42;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let font = FontRef::try_from_slice(include_bytes!("../../../assets/fonts/Guttman_Stam.ttf"))
        .expect("Failed to load font");
    let xsize: u32 = 2257;
    let ysize: u32 = 5075;
    let margin = 250;
    let ptsize: u16 = 134;

    let out_file = File::create(args.output)?;
    let mut encoder = TiffEncoder::new(BufWriter::new(out_file))?;
    let mut image_buf = GrayImage::new(xsize, ysize);
    image_buf.fill(u8::MAX);

    let mut pages = PageIter {
        lines: LineIter {
            line_buf: VecDeque::new(),
            margin,
            xsize,
            space_width: text_size(f32::from(ptsize), &font, " ").0,
            words: TRAINING_TEXT[..2049]
                .split(' ')
                .map(|str| {
                    let (width, _) = text_size(f32::from(ptsize), &font, str);
                    SizedStr { str, width }
                })
                .peekable(),
        },
        font: &font,
        ptsize,
        line_height: (ysize - margin * 2) / LINE_COUNT,
    };

    let mut pages_peek = pages.page_next(&mut image_buf);
    if pages_peek.is_none() {
        bail!("no pages");
    }
    while let Some(page) = pages_peek {
        encoder.write_image::<Gray8>(xsize, ysize, page.as_raw())?;
        pages_peek = pages.page_next(&mut image_buf);
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct SizedStr<'s> {
    str: &'s str,
    width: u32,
}

impl<'s> SizedStr<'s> {
    fn new(str: &'s str, width: u32) -> Self {
        Self { str, width }
    }
}

struct WordIter<'it, 's, Iter>
where
    Iter: Iterator<Item = SizedStr<'s>>,
{
    words: &'it mut Peekable<Iter>,
    inner_width: u32,
    space_width: u32,

    x: u32,
}

impl<'it, 's, Iter> WordIter<'it, 's, Iter>
where
    Iter: Iterator<Item = SizedStr<'s>>,
{
    fn new(words: &'it mut Peekable<Iter>, inner_width: u32, space_width: u32) -> Self {
        Self {
            words,
            inner_width,
            space_width,
            x: 0,
        }
    }
}

impl<'s, Iter> Iterator for WordIter<'_, 's, Iter>
where
    Iter: Iterator<Item = SizedStr<'s>>,
{
    type Item = &'s str;

    fn next(&mut self) -> Option<Self::Item> {
        let SizedStr { mut width, .. } = *self.words.peek()?;

        // `+ ptsize` for space
        if self.x > 0 {
            width += self.space_width;
        }

        if self.x + width > self.inner_width {
            return None;
        }

        self.x += width;
        self.words.next().map(|w| w.str)
    }
}

struct LineIter<Words: Iterator> {
    line_buf: VecDeque<u8>,
    words: Peekable<Words>,
    margin: u32,
    xsize: u32,
    space_width: u32,
}

impl<'s, Words> LineIter<Words>
where
    Words: Iterator<Item = SizedStr<'s>>,
{
    fn next(&mut self) -> Option<SizedStr> {
        let Self {
            ref mut line_buf,
            ref mut words,
            margin,
            xsize,
            space_width,
        } = *self;
        line_buf.clear();
        let mut words = WordIter::new(&mut *words, xsize - margin * 2, space_width);
        for word in &mut words {
            if !line_buf.is_empty() {
                line_buf.push_front(b' ');
            }
            for c in word.chars() {
                let mut buf = [0u8; 4];
                let len = c.encode_utf8(&mut buf).len();
                for &b in buf[..len].iter().rev() {
                    line_buf.push_front(b);
                }
            }
        }
        if line_buf.is_empty() {
            return None;
        }
        let (_, line_buf) = line_buf.as_slices();
        let str = unsafe { core::str::from_utf8_unchecked(line_buf) };
        Some(SizedStr::new(str, words.x))
    }
}

struct PageIter<Words: Iterator, F> {
    lines: LineIter<Words>,
    line_height: u32,
    ptsize: u16,
    font: F,
}

impl<'s, Words, F> PageIter<Words, F>
where
    Words: Iterator<Item = SizedStr<'s>>,
    F: Font,
{
    fn page_next<'b>(&mut self, image_buf: &'b mut GrayImage) -> Option<&'b GrayImage> {
        let LineIter { margin, xsize, .. } = self.lines;
        let inner_width = xsize - margin * 2;

        let mut line_peek = self.lines.next();
        let mut line_i = 0u32;

        line_peek?;
        image_buf.fill(u8::MAX);
        while let Some(line) = line_peek {
            draw_text_mut(
                image_buf,
                Luma([0]),
                i32::try_from(margin + (inner_width - line.width)).unwrap(),
                i32::try_from(margin + line_i * self.line_height).unwrap(),
                f32::from(self.ptsize),
                &self.font,
                line.str,
            );
            line_peek = self.lines.next();
            line_i += 1;

            if line_i >= LINE_COUNT {
                break;
            }
        }

        let complexity = 10u32;
        gaussian_noise_mut(
            image_buf,
            (complexity - 1).into(),
            (10 * complexity - 10).into(),
            (5 * complexity - 5).into(),
        );
        Some(image_buf)
    }
}
