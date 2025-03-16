#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod img_editor;

use eframe::{
    App, Frame,
    egui::{
        self, ColorImage, Context, FontData, FontFamily, FontId, Rect, TextStyle, TextureOptions,
        pos2,
    },
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
};
use eyre::{OptionExt, ensure};
use image::ImageReader;
use img_editor::LoadedData;
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    ops::ControlFlow,
    sync::{Arc, Mutex},
    thread,
};

use teamim::tesseract_ext::BoundingBox;

#[derive(Clone, PartialEq, Eq)]
struct EditableRect {
    rect: Rect,
    text: String,
}

enum LoadingState {
    NotStarted,
    Loading,
    Loaded(LoadedData),
    Failed(String),
}

struct TextBoxApp {
    state: Arc<Mutex<LoadingState>>,
}

impl TextBoxApp {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(LoadingState::NotStarted)),
        }
    }

    fn start_loading(&self, ctx: Context, image_path: String, boxes_path: String) {
        let state = Arc::clone(&self.state);

        // Update state to Loading
        {
            let mut state_lock = state.lock().unwrap();
            if let LoadingState::NotStarted = *state_lock {
                *state_lock = LoadingState::Loading;
            } else {
                return; // Already loading or loaded
            }
        }

        thread::spawn(move || {
            if let Err(err) = load(ctx, image_path, boxes_path, state.clone()) {
                *state.lock().unwrap() = LoadingState::Failed(err.to_string());
            }
        });
    }
}

#[expect(clippy::needless_pass_by_value)]
fn load(
    ctx: Context,
    image_path: String,
    boxes_path: String,
    state: Arc<Mutex<LoadingState>>,
) -> eyre::Result<()> {
    // image
    let image = ImageReader::open(image_path)?.decode()?;
    let size = [image.width() as _, image.height() as _];
    #[expect(clippy::cast_precision_loss)]
    let image_size = size.map(|x| x as _).into();

    let pixels = image.into_rgba8().into_flat_samples();
    let color_image = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
    let texture = ctx.load_texture("image-texture", color_image, TextureOptions::default());
    *state.lock().unwrap() = LoadingState::Loaded(LoadedData::new(texture, image_size, Vec::new()));

    // boxes
    let lines = BufReader::new(File::open(boxes_path)?).lines();
    for tb in BBIter::new(lines) {
        let tb = tb?;
        let LoadingState::Loaded(ref mut loaded_data) = *state.lock().unwrap() else {
            return Err(eyre::eyre!("LoadedData is not Loaded"));
        };

        // Convert from bottom-left to top-left origin
        #[expect(clippy::cast_precision_loss)]
        let rect = Rect::from_min_max(
            pos2(tb.left as _, image_size.y - tb.top as f32),
            pos2(tb.right as _, image_size.y - tb.bottom as f32),
        );
        loaded_data.boxes.push(EditableRect {
            rect,
            text: tb.value,
        });
    }
    Ok(())
}

impl App for TextBoxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut state_lock = self.state.lock().unwrap();

            match *state_lock {
                LoadingState::NotStarted => {
                    ui.centered_and_justified(|ui| {
                        ui.label("Starting to load...");
                    });
                }
                LoadingState::Loading => {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                        ui.label("Loading image and preparing TextBoxes...");
                    });
                }
                LoadingState::Loaded(ref mut loaded_data) => {
                    ui.add(loaded_data);
                }
                LoadingState::Failed(ref err) => {
                    ui.centered_and_justified(|ui| {
                        ui.label("Failed to load image:");
                        ui.label(err.to_string());
                    });
                }
            }
        });
    }
}

fn main() -> eframe::Result {
    #[cfg(debug_assertions)]
    {
        // set console codepage to UTF-8
        std::process::Command::new("cmd")
            .args(["/C", "chcp 65001"])
            .output()
            .map_err(From::from)
            .map_err(eframe::Error::AppCreation)?;
    }

    color_eyre::install().unwrap();
    env_logger::init();
    let options = eframe::NativeOptions {
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "Teamim",
        options,
        Box::new(|cc| {
            let app = TextBoxApp::new();
            configure_text_styles(&cc.egui_ctx);
            app.start_loading(
                cc.egui_ctx.clone(),
                "assets/images/245/007.jpg".to_owned(),
                "assets/corrected_boxfiles/245_007.box".to_owned(),
            );

            Ok(Box::new(app))
        }),
    )
}

#[inline]
fn boxes_style() -> TextStyle {
    TextStyle::Name("Boxes".into())
}

const FONT_SIZE: f32 = 127.0;
fn configure_text_styles(ctx: &egui::Context) {
    let font_name = "Guttman Stam";
    let font_family = FontFamily::Name(font_name.into());
    ctx.add_font(FontInsert {
        name: font_name.to_owned(),
        data: FontData::from_static(include_bytes!("../../../assets/fonts/Guttman_Stam.ttf")),
        families: vec![InsertFontFamily {
            family: font_family.clone(),
            priority: FontPriority::Highest,
        }],
    });
    ctx.all_styles_mut(move |style| {
        style
            .text_styles
            .insert(boxes_style(), FontId::new(FONT_SIZE, font_family.clone()));
    });
}

struct BBIter<I> {
    iter: I,
}

impl<I> BBIter<I> {
    fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I> Iterator for BBIter<I>
where
    I: Iterator<Item = io::Result<String>>,
{
    type Item = eyre::Result<BoundingBox<String>>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut last_box: Option<BoundingBox<String>> = None;
        for line in &mut self.iter {
            match line
                .map_err(eyre::Report::from)
                .and_then(|line| parse_line(&mut last_box, &line))
            {
                Ok(ControlFlow::Break(())) => {
                    return Some(last_box.ok_or_eyre("missing last box"));
                }
                Ok(ControlFlow::Continue(())) => {}
                Err(e) => return Some(Err(e)),
            }
        }
        last_box.map(Ok)
    }
}

fn parse_line(
    last_box: &mut Option<BoundingBox<String>>,
    line: &str,
) -> eyre::Result<ControlFlow<()>> {
    let mut chars = line.chars();
    let char = chars.next().ok_or_eyre("missing first char")?;
    if char == '\t' {
        return Ok(ControlFlow::Break(()));
    } else if let Some(r#box) = last_box.as_mut() {
        r#box.value.insert(0, char);
    } else {
        let space = chars.next().ok_or_else(|| eyre::eyre!("missing space"))?;
        ensure!(space == ' ', "expected space, found {char:?}");
        let mut parts = chars.as_str().split(' ');
        *last_box = Some(BoundingBox {
            value: char.to_string(),
            left: parts
                .next()
                .ok_or_else(|| eyre::eyre!("missing left"))?
                .parse()?,
            bottom: parts
                .next()
                .ok_or_else(|| eyre::eyre!("missing bottom"))?
                .parse()?,
            right: parts
                .next()
                .ok_or_else(|| eyre::eyre!("missing right"))?
                .parse()?,
            top: parts
                .next()
                .ok_or_else(|| eyre::eyre!("missing top"))?
                .parse()?,
        });
    }
    Ok(ControlFlow::Continue(()))
}
