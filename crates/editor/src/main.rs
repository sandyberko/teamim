#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod img_editor;

use eframe::{
    App, Frame,
    egui::{
        self, ColorImage, Context, FontData, FontFamily, FontId, Rect, TextStyle, TextureOptions,
        Vec2,
    },
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
};
use eyre::{OptionExt, ensure};
use img_editor::LoadedData;
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    thread,
};

use teamim::tesseract_ext::BoundingBox;

struct EditableRect {
    rect: Rect,
    text: String,
}

enum LoadingState {
    NotStarted,
    Loading,
    Loaded(LoadedData),
    Failed,
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
            // Load image
            let Ok(image) = image::open(Path::new(&image_path)) else {
                // Handle loading error
                let mut state_lock = state.lock().unwrap();
                *state_lock = LoadingState::Failed;
                return;
            };

            let size = [image.width() as _, image.height() as _];
            let image_height = size[1] as f32;
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();

            let color_image = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

            // read boxes
            let Ok(textboxes) = fs::read_to_string(Path::new(&boxes_path)) else {
                // Handle loading error
                let mut state_lock = state.lock().unwrap();
                *state_lock = LoadingState::Failed;
                return;
            };

            let Ok(textboxes) = parse_lstm_box(&textboxes) else {
                // Handle loading error
                let mut state_lock = state.lock().unwrap();
                *state_lock = LoadingState::Failed;
                return;
            };

            // Convert TextBoxes to EditableRects
            let editable_rects = textboxes
                .iter()
                .map(|tb| {
                    // Convert from bottom-left to top-left origin
                    let top_left = egui::pos2(tb.left as f32, image_height - tb.top as f32);

                    let bottom_right = egui::pos2(tb.right as f32, image_height - tb.bottom as f32);

                    EditableRect {
                        rect: Rect::from_min_max(top_left, bottom_right),
                        text: tb.value.clone(),
                    }
                })
                .collect();

            let image_size = Vec2::new(size[0] as f32, size[1] as f32);

            let texture = ctx.load_texture("image-texture", color_image, TextureOptions::default());

            // We'll set the texture later in the main thread
            let mut state_lock = state.lock().unwrap();
            *state_lock =
                LoadingState::Loaded(LoadedData::new(texture, image_size, editable_rects));
        });
    }
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
                LoadingState::Failed => {
                    ui.centered_and_justified(|ui| {
                        ui.label("Failed to load image!");
                    });
                }
            }
        });
    }
}

fn main() -> eframe::Result {
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
            .insert(boxes_style(), FontId::new(127.0, font_family.clone()));
    });
}

fn parse_lstm_box(s: &str) -> eyre::Result<Vec<BoundingBox<String>>> {
    let mut boxes = Vec::new();
    let mut last_box: Option<BoundingBox<String>> = None;
    for line in s.lines() {
        let mut chars = line.chars();
        let char = chars.next().ok_or_eyre("missing first char")?;
        if char == '\t' {
            boxes.push(last_box.ok_or_eyre("missing last box")?);
            last_box = None;
        } else if let Some(r#box) = last_box.as_mut() {
            r#box.value.push(char);
        } else {
            let space = chars.next().ok_or_else(|| eyre::eyre!("missing space"))?;
            ensure!(space == ' ', "expected space, found {char:?}");
            let mut parts = chars.as_str().split(' ');
            last_box = Some(BoundingBox {
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
    }
    Ok(boxes)
}
