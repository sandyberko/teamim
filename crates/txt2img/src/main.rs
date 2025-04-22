mod augment;

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use ab_glyph::{Font, FontRef, point};
use augment::Bulge;
use clap::Parser;
use imageproc::{
    drawing::{draw_cross_mut, draw_hollow_rect_mut, draw_text_mut, text_size},
    image::{GrayImage, Luma},
};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use rand::rngs::ThreadRng;
use teamim::{
    TRAINING_TEXT,
    tesseract_ext::bounding_box::{BoundingBox, LINE_TERMINATOR, Rect},
};
use tiff::{
    encoder::{Rational, TiffEncoder, colortype::Gray8, compression::Lzw},
    tags::ResolutionUnit,
};

#[derive(Debug, Parser)]
struct Args {
    output: PathBuf,

    #[arg(short, long)]
    quiet: bool,
}

const LINE_COUNT: u32 = 42;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let bars = MultiProgress::new();

    let bar = bars.add(ProgressBar::new(TRAINING_TEXT.len() as u64));
    if args.quiet {
        bar.set_draw_target(ProgressDrawTarget::hidden());
    }
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] / [{eta_precise}] {msg:.yellow} \x1B]9;4;1;{percent}\x07",
        )?
        .tick_chars("◐◓◑◒"),
    );
    bar.enable_steady_tick(Duration::from_millis(200));

    let font = FontRef::try_from_slice(include_bytes!("../../../assets/fonts/Guttman_Stam.ttf"))
        .expect("Failed to load font");

    generate(&args.output, &bar, font)?;

    Ok(())
}

const DPI: u32 = 300;
fn generate(output: &Path, bar: &ProgressBar, font: impl Font) -> eyre::Result<()> {
    let xsize: u32 = 2257;
    let ysize: u32 = 5075;
    let margin = 250;
    let ptsize: u16 = 134;

    let mut box_writer = BufWriter::new(File::create(output.with_extension("box"))?);
    let mut encoder = TiffEncoder::new(BufWriter::new(File::create(output)?))?;
    let mut image_buf = GrayImage::new(xsize, ysize);

    let mut pages = PageIter {
        line_buf: &mut String::new(),
        text: &mut &TRAINING_TEXT[..],
        margin,
        xsize,
        ysize,
        font,
        ptsize,
        box_writer: &mut box_writer,
        line_height: (ysize - margin * 2) / LINE_COUNT,
        page_i: 0,
        bar: bar.clone(),
        rng: ThreadRng::default(),
    };

    #[expect(clippy::never_loop)]
    while !pages.text.is_empty() {
        let page_bulge = pages.render_page(&mut image_buf)?;

        if pages.page_i > 0 {
            // line terminator between pages
            #[allow(clippy::cast_possible_wrap)]
            let rect = Rect::new(
                (xsize - margin - 1) as _,
                margin as _,
                (xsize - margin) as _,
                (margin + 1) as _,
            );
            let rect = page_bulge.warp_rect(rect);
            writeln!(
                pages.box_writer,
                "{}",
                BoundingBox::new_paged(LINE_TERMINATOR, rect, pages.page_i - 1,)
            )?;
        }

        {
            let mut encoder = encoder.new_image_with_compression::<Gray8, _>(xsize, ysize, Lzw)?;
            encoder.resolution(ResolutionUnit::Inch, Rational { n: DPI, d: 1 });
            encoder.write_data(image_buf.as_raw())?;
        }
        break;
    }
    pages.box_writer.flush()?;
    bar.finish_with_message("🏁 done");
    Ok(())
}

struct PageIter<'s, F, W> {
    line_buf: &'s mut String,
    text: &'s mut &'s str,
    margin: u32,
    xsize: u32,
    ysize: u32,
    ptsize: u16,
    font: F,
    line_height: u32,
    box_writer: W,
    page_i: usize,
    bar: ProgressBar,
    rng: ThreadRng,
}

impl<F, W> PageIter<'_, F, W>
where
    F: Font,
    W: Write,
{
    fn render_page(&mut self, img: &mut GrayImage) -> eyre::Result<Bulge> {
        self.bar.set_message(format!("page {}", self.page_i));

        let Self { margin, xsize, ysize, ptsize, .. } = *self;
        let inner_width = xsize - margin * 2;
        let bulge = Bulge::new(&mut self.rng, xsize, ysize);
        img.fill(u8::MAX);

        #[expect(
            clippy::cast_possible_wrap,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        for line_i in 0..LINE_COUNT {
            let Some(line_bounds) = self.prepare_line() else {
                break;
            };

            let draw_x = i32::try_from(margin + (inner_width - line_bounds.max.x.ceil() as u32))?;
            let draw_y = i32::try_from(margin + line_i * self.line_height)?;

            // write boxes
            {
                let rect = Rect::new(
                    draw_x + line_bounds.min.x.floor() as i32,
                    ysize as i32 - draw_y - line_bounds.max.y.ceil() as i32,
                    draw_x + line_bounds.max.x.ceil() as i32,
                    ysize as i32 - draw_y - line_bounds.min.y.ceil() as i32,
                );
                let rect = bulge.warp_rect(rect);

                {
                    // DEBUG
                    let rect = imageproc::rect::Rect::at(
                        rect.left,
                        (ysize - rect.bottom as u32 - rect.height()) as _,
                    )
                    .of_size(rect.width(), rect.height());
                    draw_hollow_rect_mut(img, rect, Luma([100]));
                    draw_cross_mut(img, Luma([75]), draw_x, draw_y);
                }

                if line_i > 0 {
                    writeln!(
                        self.box_writer,
                        "{}",
                        BoundingBox::new_paged(LINE_TERMINATOR, rect, self.page_i)
                    )?;
                }

                for c in self.line_buf.chars() {
                    writeln!(self.box_writer, "{}", BoundingBox::new_paged(c, rect, self.page_i))?;
                }
            }
            draw_text_mut(
                img,
                Luma([0]),
                draw_x,
                draw_y,
                f32::from(ptsize),
                &self.font,
                self.line_buf,
            );
            self.bar.inc(self.line_buf.len() as u64);
        }

        // augmet image
        // augment::augment(img, &mut self.rng, bulge);

        self.page_i += 1;
        Ok(bulge)
    }

    fn prepare_line(&mut self) -> Option<ab_glyph::Rect> {
        let Self { margin, xsize, ptsize, ref font, .. } = *self;
        self.line_buf.clear();

        let inner_width = xsize - margin * 2;
        let mut line_bb: Option<ab_glyph::Rect> = None;
        while !self.text.is_empty() {
            let end = self
                .text
                .match_indices(' ')
                .map(|(idx, _)| idx)
                .find(|&idx| idx > 0)
                .unwrap_or(self.text.len());

            // attemt to fit the word
            let start_in_line = self.line_buf.len();
            for c in self.text[..end].chars().rev() {
                self.line_buf.push(c);
            }

            let word_bb = text_size(f32::from(ptsize), &font, &self.line_buf[start_in_line..]);
            let appended = if let Some(bounds) = line_bb {
                // union
                ab_glyph::Rect {
                    min: point(bounds.min.x.min(word_bb.min.x), bounds.min.y.min(word_bb.min.y)),
                    max: point(bounds.max.x.max(word_bb.max.x), bounds.max.y.max(word_bb.max.y)),
                }
            } else {
                word_bb
            };

            if appended.max.x >= inner_width as _ {
                // backoff
                self.line_buf.truncate(start_in_line);
                break;
            }

            *self.text = &self.text[end..];
            line_bb = Some(appended);
        }

        // eat the inter-line space
        if !self.text.is_empty() {
            *self.text = &self.text[1..];
        }

        if self.line_buf.is_empty() {
            return None;
        }
        line_bb
    }
}
