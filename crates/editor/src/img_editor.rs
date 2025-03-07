use eframe::egui::{
    Align, Color32, CursorIcon, Image, Key, Layout, Rect, Response, Scene, Sense, Stroke,
    StrokeKind, TextEdit, TextureHandle, Ui, UiBuilder, Vec2, Widget,
};
use teamim::tesseract_ext::BoundingBox;

use crate::{EditableRect, boxes_style};

const DISPLAY_OPT_LEN: usize = 4;
const DISPLAY_OPTS: [(&str, Key); DISPLAY_OPT_LEN] = [
    ("image", Key::F1),
    ("frames", Key::F2),
    ("text", Key::F3),
    ("solo", Key::F4),
];

pub(crate) struct LoadedData {
    texture: TextureHandle,
    image_size: Vec2,
    editable_rects: Vec<EditableRect>,

    // ui
    scene_rect: Rect,
    display_opts: [bool; DISPLAY_OPT_LEN],
    focused_rect: usize,
}

impl LoadedData {
    pub(crate) fn new(
        texture: TextureHandle,
        image_size: Vec2,
        editable_rects: Vec<EditableRect>,
    ) -> Self {
        Self {
            texture,
            image_size,
            editable_rects,
            scene_rect: Rect::ZERO,
            display_opts: [true; DISPLAY_OPT_LEN],
            focused_rect: 0,
        }
    }
    // Convert EditableRects back to TextBoxes (bottom-left origin)
    pub(crate) fn get_textboxes(&self) -> Vec<BoundingBox<String>> {
        self.editable_rects
            .iter()
            .map(|r| BoundingBox {
                value: r.text.clone(),
                left: r.rect.min.x as i32,
                bottom: (self.image_size.y - r.rect.max.y) as i32,
                right: r.rect.max.x as i32,
                top: (self.image_size.y - r.rect.min.y) as i32,
            })
            .collect()
    }
}

impl Widget for &mut LoadedData {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.input(|inp| {
            for ((_, key), enabled) in DISPLAY_OPTS.iter().zip(&mut self.display_opts) {
                if inp.key_pressed(*key) {
                    *enabled = false;
                } else if !inp.modifiers.shift && inp.key_released(*key) {
                    *enabled = true;
                }
            }
        });

        ui.horizontal(|ui| {
            for ((name, _), enabled) in DISPLAY_OPTS.iter().zip(&mut self.display_opts) {
                ui.checkbox(enabled, *name);
            }
        });

        Scene::new()
            .max_inner_size(self.image_size)
            .zoom_range(0.1..=20.0)
            .show(ui, &mut self.scene_rect, |ui| {
                ui.style_mut().override_text_style = Some(boxes_style());

                if self.display_opts[0] {
                    ui.add_sized(self.image_size, Image::new(&self.texture));
                }

                for (rect_i, bx) in self.editable_rects.iter_mut().enumerate() {
                    let resp = ui.allocate_new_ui(
                        UiBuilder::new()
                            .layout(Layout::right_to_left(Align::Min))
                            .max_rect(bx.rect),
                        |ui| show_rect(bx, self.display_opts, ui),
                    );
                    if resp.inner.gained_focus() {
                        self.focused_rect = rect_i;
                    }
                }
                ui.response()
            });
        ui.response()
    }
}

fn show_rect(
    bx: &mut EditableRect,
    display_opts: [bool; DISPLAY_OPT_LEN],
    ui: &mut Ui,
) -> Response {
    let text_color = if display_opts[3] {
        Color32::RED
    } else {
        Color32::TRANSPARENT
    };

    // text
    let resp = ui.add(
        TextEdit::singleline(&mut bx.text)
            .clip_text(false)
            .frame(false)
            .background_color(Color32::TRANSPARENT)
            .text_color(text_color),
    );

    // interaction
    if resp.has_focus() {
        ui.input(|inp| {
            if inp.key_pressed(Key::Enter) {
                // TODO new line
            }
        });
        let resp = resp.interact(Sense::drag() & Sense::hover());
        if resp.hovered() {
            ui.ctx().output_mut(|o| o.cursor_icon = CursorIcon::Move);
        }
        if resp.dragged() {
            bx.rect.min += resp.drag_delta();
        }
    }

    // frame
    if display_opts[1] {
        let color = if resp.has_focus() {
            Color32::RED
        } else {
            Color32::GRAY
        };
        ui.painter()
            .rect_stroke(bx.rect, 0.0, Stroke::new(5.0, color), StrokeKind::Outside);
    }
    resp
}
