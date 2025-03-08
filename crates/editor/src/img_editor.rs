use eframe::egui::{
    Color32, CursorIcon, Id, Image, Key, Rect, Response, Scene, Sense, Stroke, StrokeKind,
    TextEdit, TextureHandle, Ui, UiBuilder, Vec2, Widget, pos2, vec2,
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

const FRAME_WIDTH: f32 = 5.0;
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
            Stroke::new(FRAME_WIDTH, frame_color),
            StrokeKind::Outside,
        );
    }

    // interaction
    if selected {
        if ui.input(|inp| inp.modifiers.ctrl) {
            // move
            let resp = ui.interact(
                bx.rect.expand(FRAME_WIDTH),
                Id::new("move_handle"),
                Sense::drag(),
            );
            if resp.hovered() {
                ui.ctx().output_mut(|o| o.cursor_icon = CursorIcon::Move);
            }
            if resp.dragged() {
                bx.rect = bx.rect.translate(resp.drag_delta());
            }
        } else {
            handle_resize(bx, ui);
        }

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

fn handle_resize(bx: &mut EditableRect, ui: &mut Ui) {
    // resize handle size
    const SZ: f32 = 8.0;

    let north = {
        let resp = ui.interact(
            Rect::from_min_size(
                bx.rect.min - Vec2::splat(SZ),
                vec2(bx.rect.width() + SZ * 2.0, SZ),
            ),
            Id::new("resize_handle_top"),
            Sense::drag(),
        );
        if resp.dragged() {
            bx.rect.min.y += resp.drag_delta().y;
        }
        resp.hovered()
    };
    let east = {
        let resp = ui.interact(
            Rect::from_min_max(
                pos2(bx.rect.max.x, bx.rect.min.y - SZ),
                pos2(bx.rect.max.x + SZ, bx.rect.max.y + SZ),
            ),
            Id::new("resize_handle_right"),
            Sense::drag(),
        );
        if resp.dragged() {
            bx.rect.max.x += resp.drag_delta().x;
        }
        resp.hovered()
    };
    let south = {
        let resp = ui.interact(
            Rect::from_min_max(
                pos2(bx.rect.min.x - SZ, bx.rect.max.y),
                pos2(bx.rect.max.x + SZ, bx.rect.max.y + SZ),
            ),
            Id::new("resize_handle_bottom"),
            Sense::drag(),
        );
        if resp.dragged() {
            bx.rect.max.y += resp.drag_delta().y;
        }
        resp.hovered()
    };
    let west = {
        let resp = ui.interact(
            Rect::from_min_max(
                pos2(bx.rect.min.x - SZ, bx.rect.min.y - SZ),
                pos2(bx.rect.min.x, bx.rect.max.y + SZ),
            ),
            Id::new("resize_handle_left"),
            Sense::drag(),
        );
        if resp.dragged() {
            bx.rect.min.x += resp.drag_delta().x;
        }
        resp.hovered()
    };
    let icon = match (north, east, south, west) {
        (true, false, false, false) | (false, false, true, false) => CursorIcon::ResizeVertical,
        (false, false, false, true) | (false, true, false, false) => CursorIcon::ResizeHorizontal,
        (true, false, false, true) | (false, true, true, false) => CursorIcon::ResizeNwSe,
        (true, true, false, false) | (false, false, true, true) => CursorIcon::ResizeNeSw,

        _ => ui.ctx().output(|o| o.cursor_icon),
    };
    ui.ctx().output_mut(|o| o.cursor_icon = icon);
}
