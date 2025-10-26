mod augment;
mod text;

#[cfg(test)]
mod tests;

use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use ab_glyph::{Font, FontRef, point};
use augment::Bulge;
use clap::Parser;
use eyre::{Context, OptionExt, eyre};
use imageproc::image::{GrayImage, Luma};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use phf::{Map, phf_map};
use rand::rngs::ThreadRng;
use rayon::prelude::*;
use teamim::{
    TRAINING_TEXT,
    tesseract_ext::bounding_box::{BoundingBox, LINE_TERMINATOR, Rect},
};
use text::{draw_text_mut, text_size};
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
const TRAINING_TEXT_WIDE: &str = include_str!("../../../assets/text/mam/training-wide-letters.txt");

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let bars = MultiProgress::new();
    if args.quiet {
        bars.set_draw_target(ProgressDrawTarget::hidden());
    }
    let font_paths = fs::read_dir("assets/fonts")?
        .filter_map(|entry| match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "ttf") {
                    Some(Ok((path, TRAINING_TEXT, false)))
                } else {
                    None
                }
            }
            Err(err) => Some(Err(err)),
        })
        .chain([Ok((PathBuf::from("assets/fonts/Guttman_Stam.ttf"), TRAINING_TEXT_WIDE, true))])
        .collect::<Result<Vec<_>, _>>()?;

    let overall_bar = bars.add(ProgressBar::new((TRAINING_TEXT.len() * font_paths.len()) as _));
    overall_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] / [{eta_precise}] {msg:.yellow} \x1B]9;4;1;{percent}\x07",
            )?
            .tick_chars("◐◓◑◒"),
        );
    overall_bar.enable_steady_tick(Duration::from_millis(200));
    let max_font_name_len =
        font_paths.iter().filter_map(|(path, ..)| Some(path.file_stem()?.len())).max().unwrap_or(0);
    let font_bar_style = ProgressStyle::with_template(&format!(
        "{{prefix:>{max_font_name_len}}} |{{bar:40.cyan/black}}| {{msg}}"
    ))?;

    font_paths
        .into_par_iter()
        .map(|(font_path, text, is_wide)| {
            let font_name = font_path.file_stem().ok_or_eyre("No file stem")?.to_string_lossy();
            let file_name = format!("{font_name}{wide}", wide = if is_wide { "_wide" } else { "" });
            let out_path = args.output.join(&file_name).with_extension("tif");

            let bar = bars.add(ProgressBar::new(TRAINING_TEXT.len() as u64));
            bar.set_style(font_bar_style.clone());
            bar.set_prefix(file_name);

            let font = fs::read(&font_path)?;
            let font = FontRef::try_from_slice(&font).expect("Failed to load font");

            generate(&out_path, vec![overall_bar.clone(), bar.clone()], font, text, is_wide)?;

            bar.finish_with_message("🏁 done");
            eyre::Ok(())
        })
        .collect::<Result<(), _>>()?;

    overall_bar.finish_with_message("🏁 done");
    Ok(())
}

const DPI: u32 = 300;
fn generate(
    output: &Path,
    bars: Vec<ProgressBar>,
    font: impl Font,
    text: &str,
    is_wide: bool,
) -> eyre::Result<()> {
    let xsize: u32 = 2257;
    let ysize: u32 = 5075;
    let margin = 250;
    let ptsize: u16 = 134;

    let box_path = output.with_extension("box");
    let mut box_writer = BufWriter::new(
        File::create(&box_path)
            .wrap_err_with(|| eyre!("Failed to create box file {box_path:?}"))?,
    );
    let mut encoder = TiffEncoder::new(BufWriter::new(File::create(output)?))?;
    let mut image_buf = GrayImage::new(xsize, ysize);

    let mut pages = PageIter {
        text: &mut &*text,
        last_rect: None,
        margin,
        xsize,
        ysize,
        font,
        ptsize,
        box_writer: &mut box_writer,
        line_height: (ysize - margin * 2) / LINE_COUNT,
        page_i: 0,
        bars,
        rng: ThreadRng::default(),
        is_wide,
    };

    while !pages.text.is_empty() {
        pages.render_page(&mut image_buf)?;

        if let Some(last_rect) = pages.last_rect {
            // line terminator between pages
            let rect = Rect::<u32>::new(
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
    }
    pages.box_writer.flush()?;
    Ok(())
}

static WIDE_LETTERS: Map<char, char> = phf_map! {
    'ﬡ' =>'א',
    'ﬢ' =>'ד',
    'ﬣ' =>'ה',
    'ﬤ' =>'כ',
    'ﬥ' =>'ל',
    'ﬦ' =>'ס',
    'ﬧ' =>'ר',
    'ﬨ' =>'ת',
};

struct PageIter<'s, F, W> {
    text: &'s mut &'s str,
    last_rect: Option<Rect<u32>>,
    margin: u32,
    xsize: u32,
    ysize: u32,
    ptsize: u16,
    font: F,
    line_height: u32,
    box_writer: W,
    page_i: usize,
    bars: Vec<ProgressBar>,
    rng: ThreadRng,
    is_wide: bool,
}

impl<'s, F, W> PageIter<'s, F, W>
where
    F: Font,
    W: Write,
{
    fn render_page(&mut self, img: &mut GrayImage) -> eyre::Result<Bulge> {
        if let Some(page_bar) = self.bars.last() {
            page_bar.set_message(format!("page {}", self.page_i));
        }

        let Self { margin, xsize, ysize, ptsize, .. } = *self;
        let bulge = Bulge::new(&mut self.rng, xsize, ysize);
        img.fill(u8::MAX);

        #[expect(
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss
        )]
        for line_i in 0..LINE_COUNT {
            let Some((line_bounds, line)) = self.layout_line() else {
                break;
            };

            // let draw_x = margin as f32 + inner_width as f32 - line_bounds.max.x;
            let draw_x = (xsize - margin) as f32;
            let draw_y = (margin + line_i * self.line_height) as f32;

            // write boxes
            {
                let line_bounds = ab_glyph::Rect {
                    min: point(line_bounds.min.x + draw_x, line_bounds.min.y + draw_y),
                    max: point(line_bounds.max.x + draw_x, line_bounds.max.y + draw_y),
                };
                let rect = Rect::new(
                    line_bounds.min.x.floor() as u32,
                    ysize - line_bounds.max.y.ceil() as u32,
                    line_bounds.max.x.ceil() as u32,
                    ysize - line_bounds.min.y.ceil() as u32,
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
                    let c =
                        self.is_wide.then(|| WIDE_LETTERS.get(&c)).flatten().copied().unwrap_or(c);
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
                |(x, y)| {
                    let (x, y) = bulge.warp(x as _, y as _);
                    (x.round() as _, y.round() as _)
                },
            );

            for bar in &self.bars {
                bar.inc(line.len() as u64);
            }
        }

        // augmet image
        // augment::augment(img, &mut self.rng, bulge);

        self.page_i += 1;
        Ok(bulge)
    }

    fn layout_line(&mut self) -> Option<(ab_glyph::Rect, &'s str)> {
        if self.text.is_empty() {
            return None;
        }

        let Self { margin, xsize, ptsize, ref font, .. } = *self;

        #[expect(clippy::cast_precision_loss)]
        let inner_width = (xsize - margin * 2) as f32;
        let mut line_bb: Option<ab_glyph::Rect> = None;
        let mut word_start = 0;
        for word_end in self.text.match_indices(' ').map(|(idx, _)| idx).chain([self.text.len()]) {
            // attemt to fit the word
            let word = &self.text[word_start..word_end];
            let (word_bb, word_advance) = text_size(f32::from(ptsize), &font, word);
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
                break;
            }

            word_start = word_end;
            line_bb = Some(appended);
        }
        let line = &self.text[..word_start];

        // eat the inter-line space
        if word_start < self.text.len() {
            word_start += 1;
        }

        *self.text = &self.text[word_start..];

        Some((line_bb?, line))
    }
}
