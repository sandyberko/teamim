mod augment;

use std::{
    collections::VecDeque,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use ab_glyph::{Font, FontRef, point};
use augment::Bulge;
use clap::Parser;
use imageproc::{drawing::{draw_text_mut, text_size}, image::{GrayImage, Luma}};
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
        line_buf: &mut VecDeque::new(),
        text: &mut &TRAINING_TEXT[..],
        last_rect: None,
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
        pages.render_page(&mut image_buf)?;

        if let Some(last_rect) = pages.last_rect {
            // line terminator between pages
            let rect = Rect::new(
                last_rect.left,
                last_rect.bottom,
                last_rect.left + 1,
                last_rect.bottom + 1,
            );
            writeln!(
                pages.box_writer,
                "{}",
                BoundingBox::new_paged(LINE_TERMINATOR, rect, pages.page_i - 1)
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
    line_buf: &'s mut VecDeque<u8>,
    text: &'s mut &'s str,
    last_rect: Option<Rect>,
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
            clippy::cast_precision_loss
        )]
        for line_i in 0..LINE_COUNT {
            let Some(line_bounds) = self.layout_line() else {
                break;
            };
            let line = {
                let (front, _) = self.line_buf.as_slices();
                std::str::from_utf8(front).unwrap()
            };

            let draw_x = margin as f32 + inner_width as f32 - line_bounds.max.x;
            let draw_y = (margin + line_i * self.line_height) as f32;

            // write boxes
            {
                let line_bounds = ab_glyph::Rect {
                    min: point(line_bounds.min.x + draw_x, line_bounds.min.y + draw_y),
                    max: point(line_bounds.max.x + draw_x, line_bounds.max.y + draw_y),
                };
                let rect = Rect::new(
                    line_bounds.min.x.floor() as i32,
                    ysize as i32 - line_bounds.max.y.ceil() as i32,
                    line_bounds.max.x.ceil() as i32,
                    ysize as i32 - line_bounds.min.y.ceil() as i32,
                );
                self.last_rect = Some(rect);

                if line_i > 0 {
                    writeln!(
                        self.box_writer,
                        "{}",
                        BoundingBox::new_paged(LINE_TERMINATOR, rect, self.page_i)
                    )?;
                }

                for c in line.chars() {
                    writeln!(self.box_writer, "{}", BoundingBox::new_paged(c, rect, self.page_i))?;
                }
            }
            draw_text_mut(
                img,
                Luma([0]),
                draw_x as _,
                draw_y as _,
                f32::from(ptsize),
                &self.font,
                line,
            );
            self.bar.inc(self.line_buf.len() as u64);
        }

        // augmet image
        augment::augment(img, &mut self.rng, bulge);

        self.page_i += 1;
        Ok(bulge)
    }

    fn layout_line(&mut self) -> Option<ab_glyph::Rect> {
        let Self { margin, xsize, ptsize, ref font, .. } = *self;
        self.line_buf.clear();

        #[expect(clippy::cast_precision_loss)]
        let inner_width = (xsize - margin * 2) as f32;
        let mut line_bb: Option<ab_glyph::Rect> = None;
        while !self.text.is_empty() {
            let space_ix = self
                .text
                .match_indices(' ')
                .map(|(idx, _)| idx)
                .find(|&idx| idx > 0)
                .unwrap_or(self.text.len());

            // attemt to fit the word
            for c in self.text[..space_ix].chars() {
                let mut buf = [0u8; 4];
                for &b in c.encode_utf8(&mut buf).as_bytes().iter().rev() {
                    self.line_buf.push_front(b);
                }
            }

            let rev_word = {
                self.line_buf.make_contiguous();
                let (front, _) = self.line_buf.as_slices();
                let rev_word = &front[..space_ix];
                std::str::from_utf8(rev_word).unwrap()
            };
            let (word_bb, word_advance) = text_size(f32::from(ptsize), &font, rev_word);
            let appended = if let Some(bounds) = line_bb {
                // union
                ab_glyph::Rect {
                    min: point(word_bb.min.x, bounds.min.y.min(word_bb.min.y)),
                    max: point(bounds.max.x + word_advance, bounds.max.y.max(word_bb.max.y)),
                }
            } else {
                word_bb
            };

            if appended.max.x >= inner_width {
                // backoff
                for _ in 0..space_ix {
                    self.line_buf.pop_front();
                }
                break;
            }

            *self.text = &self.text[space_ix..];
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
