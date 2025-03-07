use eframe::egui::{
    Color32, CursorIcon, Id, Image, Key, Rect, Response, Scene, Sense, Stroke, StrokeKind,
    TextEdit, TextureHandle, Ui, UiBuilder, Vec2, Widget,
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
    sselected_rect: usize,
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
            sselected_rect: 0,
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

                if self.display_opts[3] {
                    for (rect_i, bx) in self.editable_rects.iter_mut().enumerate() {
                        let selected = self.sselected_rect == rect_i;
                        let resp = show_rect(bx, self.display_opts, selected, ui);
                        if resp.gained_focus() {
                            self.sselected_rect = rect_i;
                        }
                    }
                } else {
                    show_rect(
                        &mut self.editable_rects[self.sselected_rect],
                        self.display_opts,
                        true,
                        ui,
                    );
                }
                ui.response()
            });
        ui.response()
    }
}

const RESIZE_HANDLE_SIZE: f32 = 8.0;
fn show_rect(
    bx: &mut EditableRect,
    display_opts: [bool; DISPLAY_OPT_LEN],
    selected: bool,
    ui: &mut Ui,
) -> Response {
    let text_color = if display_opts[2] {
        Color32::RED
    } else {
        Color32::TRANSPARENT
    };

    // text
    let resp = ui.allocate_new_ui(UiBuilder::new().max_rect(bx.rect), |ui| {
        TextEdit::singleline(&mut bx.text)
            .clip_text(false)
            .frame(false)
            .background_color(Color32::TRANSPARENT)
            .text_color(text_color)
            .show(ui)
    });

    // frame
    if display_opts[1] {
        let frame_color = if selected {
            Color32::RED
        } else {
            Color32::GRAY
        };
        ui.painter().rect_stroke(
            bx.rect,
            0.0,
            Stroke::new(5.0, frame_color),
            StrokeKind::Outside,
        );
    }

    // interaction
    if selected {
        macro_rules! corner {
            ($corner_name:literal, $corner_pos:expr, $icon:expr, |$resp:ident| $resize:expr) => {{
                ui.painter()
                    .circle_filled($corner_pos, RESIZE_HANDLE_SIZE, Color32::WHITE);
                ui.painter().circle_stroke(
                    $corner_pos,
                    RESIZE_HANDLE_SIZE,
                    Stroke::new(2.0, Color32::LIGHT_BLUE),
                );
                let resp = ui.interact(
                    Rect::from_center_size($corner_pos, Vec2::splat(RESIZE_HANDLE_SIZE * 2.0)),
                    Id::new(concat!("resize_handle_", $corner_name)),
                    Sense::click_and_drag(),
                );
                if resp.hovered() {
                    ui.ctx().output_mut(|o| o.cursor_icon = $icon);
                }
                if resp.dragged() {
                    let $resp = resp;
                    $resize
                }
            }};
        }

        corner!(
            "left_top",
            bx.rect.left_top(),
            CursorIcon::ResizeNwSe,
            |resp| bx.rect.min += resp.drag_delta()
        );
        corner!(
            "right_top",
            bx.rect.right_top(),
            CursorIcon::ResizeNeSw,
            |resp| {
                bx.rect.max.x += resp.drag_delta().x;
                bx.rect.min.y += resp.drag_delta().y;
            }
        );
        corner!(
            "left_bottom",
            bx.rect.left_bottom(),
            CursorIcon::ResizeNeSw,
            |resp| {
                bx.rect.min.x += resp.drag_delta().x;
                bx.rect.max.y += resp.drag_delta().y;
            }
        );
        corner!(
            "right_bottom",
            bx.rect.right_bottom(),
            CursorIcon::ResizeNwSe,
            |resp| bx.rect.max += resp.drag_delta()
        );

        ui.input(|inp| {
            // new line
            if inp.key_pressed(Key::Enter) {
                if let Some(cursor_range) = resp.inner.cursor_range {
                    // cursor_range.primary.index
                }
            }
        });
    }
    resp.inner.response
}
