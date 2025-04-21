use std::{f32::consts::PI, fs::File, io::BufWriter, path::PathBuf};

use ab_glyph::{Font, FontRef};
use clap::Parser;
use eyre::bail;
use imageproc::{
    distance_transform::Norm,
    drawing::{draw_text_mut, text_size},
    filter::gaussian_blur_f32,
    geometric_transformations::{Interpolation, Projection, warp},
    image::{GrayImage, Luma},
    morphology::{close_mut, dilate_mut, erode, erode_mut, open_mut},
    noise::{gaussian_noise_mut, salt_and_pepper_noise_mut},
    point::Point,
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
            line_buf: &mut String::new(),
            text: &mut &TRAINING_TEXT[..2049],
            margin,
            xsize,
            font: &font,
            ptsize,
        },
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
struct SizedStr<T> {
    str: T,
    width: u32,
}

impl<T> SizedStr<T> {
    fn new(str: T, width: u32) -> Self {
        Self { str, width }
    }
}

struct LineIter<'s, F> {
    line_buf: &'s mut String,
    text: &'s mut &'s str,
    margin: u32,
    xsize: u32,
    ptsize: u16,
    font: F,
}

impl<F> LineIter<'_, F>
where
    F: Font,
{
    fn next(&mut self) -> Option<SizedStr<()>> {
        let Self {
            margin,
            xsize,
            ptsize,
            ref font,
            ..
        } = *self;
        self.line_buf.clear();

        let line_width = xsize - margin * 2;
        let mut spaces = self.text.match_indices(' ').map(|(idx, _)| idx);
        let mut x = 0;
        let mut start = 0;
        while start < self.text.len() {
            let end = spaces.next().unwrap_or(self.text.len());
            let (word_width, _) = text_size(f32::from(ptsize), &font, &self.text[start..end]);
            if x + word_width >= line_width {
                // word doesn't fit, break and start a new line
                for c in self.text[..start].chars().rev() {
                    self.line_buf.push(c);
                }

                *self.text = &self.text[start + 1..];
                break;
            }
            x += word_width;
            start = end;
        }
        if self.line_buf.is_empty() {
            return None;
        }
        Some(SizedStr::new((), x))
    }
}

struct PageIter<'s, F> {
    lines: LineIter<'s, F>,
    line_height: u32,
}

impl<F> PageIter<'_, F>
where
    F: Font,
{
    fn page_next<'b>(&mut self, img: &'b mut GrayImage) -> Option<&'b GrayImage> {
        let LineIter {
            margin,
            xsize,
            ptsize,
            ..
        } = self.lines;
        let inner_width = xsize - margin * 2;

        let mut line_peek = self.lines.next();
        let mut line_i = 0u32;

        line_peek?;
        img.fill(u8::MAX);
        while let Some(line) = line_peek {
            draw_text_mut(
                img,
                Luma([0]),
                i32::try_from(margin + (inner_width - line.width)).unwrap(),
                i32::try_from(margin + line_i * self.line_height).unwrap(),
                f32::from(ptsize),
                &self.lines.font,
                self.lines.line_buf,
            );
            line_peek = self.lines.next();
            line_i += 1;

            if line_i >= LINE_COUNT {
                break;
            }
        }

        // augmet image

        // erode_mut(img, Norm::L2, 1);

        *img = gaussian_blur_f32(img, 1.);

        {
            // warp
            let src = [
                (0.0, 0.0),
                (img.width() as f32, 0.0),
                (img.width() as f32, img.height() as f32),
                (0.0, img.height() as f32),
            ];

            // Perturb destination points for warping
            let dst = [
                (5.0, 2.0), // Slight top-left shift
                (img.width() as f32 - 100.0, 3.0),
                (img.width() as f32 - 5.0, img.height() as f32 - 5.0),
                (8.0, img.height() as f32 - 4.0),
            ];

            *img = warp(
                img,
                &Projection::from_control_points(src, dst).unwrap(),
                Interpolation::Bilinear,
                Luma([u8::MAX]),
            );
        }

        let complexity = 7u32;
        gaussian_noise_mut(
            img,
            (complexity - 1).into(),
            (10 * complexity - 10).into(),
            (5 * complexity - 5).into(),
        );

        Some(img)
    }
}
