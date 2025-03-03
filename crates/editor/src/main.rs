#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod img_editor;

use eframe::{
    egui::{self, CentralPanel, FontData, FontFamily, FontId, TextStyle},
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
};
use eyre::{OptionExt, ensure};
use teamim::tesseract_ext::BoundingBox;

fn main() -> eframe::Result {
    color_eyre::install().unwrap();
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "Teamim",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            configure_text_styles(&cc.egui_ctx);

            Ok(Box::<App>::default())
        }),
    )
}

struct App {
    editor: img_editor::ImgEditor,
}

impl Default for App {
    fn default() -> Self {
        let boxes = parse_lstm_box(include_str!(
            "../../../assets/corrected_boxfiles/245_007.box"
        ));
        Self {
            editor: img_editor::ImgEditor::new(boxes.unwrap()),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Teamim");
            ui.add(&mut self.editor);
        });
    }
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
                top: parts
                    .next()
                    .ok_or_else(|| eyre::eyre!("missing top"))?
                    .parse()?,
                right: parts
                    .next()
                    .ok_or_else(|| eyre::eyre!("missing right"))?
                    .parse()?,
                bottom: parts
                    .next()
                    .ok_or_else(|| eyre::eyre!("missing bottom"))?
                    .parse()?,
            });
        }
    }
    Ok(boxes)
}
