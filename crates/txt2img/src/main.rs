use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    time::Duration,
};

use ab_glyph::{Font, FontRef};
use clap::Parser;
use eyre::bail;
use imageproc::{
    drawing::{draw_text_mut, text_size},
    filter::gaussian_blur_f32,
    image::{GrayImage, Luma},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use teamim::{
    TRAINING_TEXT,
    tesseract_ext::bounding_box::{BoundingBox, LINE_TERMINATOR},
};
use tiff::encoder::{
    TiffEncoder,
    colortype::Gray8,
    compression::{Deflate, DeflateLevel},
};

#[derive(Debug, Parser)]
struct Args {
    txt_path: PathBuf,
    output: PathBuf,
}

const LINE_COUNT: u32 = 42;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let bars = MultiProgress::new();

    let bar = bars.add(ProgressBar::new(TRAINING_TEXT.len() as u64));
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] / [{eta_precise}] {msg:.yellow} \x1B]9;4;1;{percent}\x07",
        )?
        .tick_chars("◐◓◑◒"),
    );
    bar.enable_steady_tick(Duration::from_millis(200));

    let font = FontRef::try_from_slice(include_bytes!("../../../assets/fonts/Guttman_Stam.ttf"))
        .expect("Failed to load font");
    let xsize: u32 = 2257;
    let ysize: u32 = 5075;
    let margin = 250;
    let ptsize: u16 = 134;

    let mut box_writer = BufWriter::new(File::create(args.output.with_extension("box"))?);

    let out_img = File::create(args.output)?;
    let mut encoder = TiffEncoder::new(BufWriter::new(out_img))?;
    let mut image_buf = GrayImage::new(xsize, ysize);
    image_buf.fill(u8::MAX);

    let mut pages = PageIter {
        lines: LineIter {
            line_buf: &mut String::new(),
            text: &mut &TRAINING_TEXT[..],
            margin,
            xsize,
            ysize,
            font,
            ptsize,
        },
        box_writer: &mut box_writer,
        line_height: (ysize - margin * 2) / LINE_COUNT,
        page_i: 0,
        bar: bar.clone(),
    };

    let mut pages_peek = pages.page_next(&mut image_buf);
    if pages_peek.is_none() {
        bail!("no pages");
    }
    while let Some(page) = pages_peek {
        encoder.write_image_with_compression::<Gray8, _>(
            xsize,
            ysize,
            Deflate::with_level(DeflateLevel::Fast),
            page.as_raw(),
        )?;
        pages_peek = pages.page_next(&mut image_buf);
    }
    pages.box_writer.flush()?;
    bar.finish_with_message("🏁 done");

    Ok(())
}

struct LineIter<'s, F> {
    line_buf: &'s mut String,
    text: &'s mut &'s str,
    margin: u32,
    xsize: u32,
    ysize: u32,
    ptsize: u16,
    font: F,
}

impl<F> LineIter<'_, F>
where
    F: Font,
{
    fn next(&mut self) -> Option<(u32, u32)> {
        let Self {
            margin,
            xsize,
            ptsize,
            ref font,
            ..
        } = *self;
        self.line_buf.clear();

        let inner_width = xsize - margin * 2;
        let mut width = 0;
        let mut height = 0;

        let mut spaces = self.text.match_indices(' ').map(|(idx, _)| idx);
        let mut start = 0;
        while start < self.text.len() {
            let end = spaces.next().unwrap_or(self.text.len());
            let (word_width, word_height) =
                text_size(f32::from(ptsize), &font, &self.text[start..end]);
            height = height.max(word_height);

            if width + word_width >= inner_width {
                // word doesn't fit, break and start a new line
                for c in self.text[..start].chars().rev() {
                    self.line_buf.push(c);
                }

                *self.text = &self.text[start + 1..];
                break;
            }

            width += word_width;
            start = end;
        }

        if self.line_buf.is_empty() {
            return None;
        }

        Some((width, height))
    }
}

struct PageIter<'s, F, W> {
    lines: LineIter<'s, F>,
    line_height: u32,
    box_writer: W,
    page_i: usize,
    bar: ProgressBar,
}

impl<F, W> PageIter<'_, F, W>
where
    F: Font,
    W: Write,
{
    fn page_next<'b>(&mut self, img: &'b mut GrayImage) -> Option<&'b GrayImage> {
        self.bar.set_message(format!("page {}", self.page_i));

        let LineIter {
            margin,
            xsize,
            ysize,
            ptsize,
            ..
        } = self.lines;
        let inner_width = xsize - margin * 2;

        let mut line_peek = self.lines.next();
        let mut line_i = 0u32;

        line_peek?;
        img.fill(u8::MAX);
        while let Some((width, height)) = line_peek {
            let x = i32::try_from(margin + (inner_width - width)).unwrap();
            let y = i32::try_from(margin + line_i * self.line_height).unwrap();

            #[allow(clippy::cast_possible_wrap)]
            {
                let bottom = ysize as i32 - y - height as i32;
                // write boxes
                if line_i > 0 {
                    writeln!(
                        self.box_writer,
                        "{}",
                        BoundingBox::new_paged(
                            LINE_TERMINATOR,
                            x,
                            bottom,
                            x + 1,
                            bottom + 1,
                            self.page_i
                        )
                    )
                    .unwrap();
                }
                for c in self.lines.line_buf.chars() {
                    writeln!(
                        self.box_writer,
                        "{}",
                        BoundingBox::new_paged(
                            c,
                            x,
                            bottom,
                            x + width as i32,
                            bottom + height as i32,
                            self.page_i
                        )
                    )
                    .unwrap();
                }
            }

            draw_text_mut(
                img,
                Luma([0]),
                x,
                y,
                f32::from(ptsize),
                &self.lines.font,
                self.lines.line_buf,
            );
            self.bar.inc(self.lines.line_buf.len() as u64);
            line_peek = self.lines.next();
            line_i += 1;

            if line_i >= LINE_COUNT {
                break;
            }
        }

        // augmet image

        // erode_mut(img, Norm::L2, 1);

        *img = gaussian_blur_f32(img, 1.);

        // #[allow(clippy::cast_precision_loss)]
        // {
        //     let width = img.width() as f32;
        //     let height = img.height() as f32;

        //     // warp
        //     let src = [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)];

        //     // Perturb destination points for warping
        //     let dst = [
        //         (5.0, 2.0), // Slight top-left shift
        //         (width - 100.0, 3.0),
        //         (width - 5.0, height - 5.0),
        //         (8.0, height - 4.0),
        //     ];

        //     *img = warp(
        //         img,
        //         &Projection::from_control_points(src, dst).unwrap(),
        //         Interpolation::Bilinear,
        //         Luma([u8::MAX]),
        //     );
        // }

        // let complexity = 7u32;
        // gaussian_noise_mut(
        //     img,
        //     (complexity - 1).into(),
        //     (10 * complexity - 10).into(),
        //     (5 * complexity - 5).into(),
        // );

        self.page_i += 1;
        Some(img)
    }
}
