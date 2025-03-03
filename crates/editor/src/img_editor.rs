use eframe::egui::{
    Align, Color32, CursorIcon, Image, Key, Rect, Response, Scene, Sense, TextEdit, Ui, Widget,
    include_image, load::TexturePoll, pos2,
};
use teamim::tesseract_ext::BoundingBox;

use crate::boxes_style;

pub(crate) struct ImgEditor {
    scene_rect: Rect,
    boxes: Vec<BoundingBox<String>>,
}

impl ImgEditor {
    pub(crate) fn new(boxes: Vec<BoundingBox<String>>) -> Self {
        Self {
            scene_rect: Rect::ZERO,
            boxes,
        }
    }
}

impl Widget for &mut ImgEditor {
    fn ui(self, ui: &mut Ui) -> Response {
        let image = Image::new(include_image!("../../../assets/images/245/007.jpg"));
        let Ok(TexturePoll::Ready { texture }) = image.load_for_size(ui.ctx(), ui.available_size())
        else {
            return ui.spinner();
        };
        Scene::new()
            .max_inner_size(texture.size)
            .zoom_range(0.1..=20.0)
            .show(ui, &mut self.scene_rect, |ui| {
                ui.add_sized(texture.size, image);
                for r#box in &mut self.boxes {
                    #[expect(clippy::cast_precision_loss)]
                    let max_rect = Rect::from_min_max(
                        pos2(r#box.left as f32, texture.size.y - r#box.top as f32),
                        pos2(r#box.right as f32, texture.size.y - r#box.bottom as f32),
                    );
                    let resp = ui.put(
                        max_rect,
                        TextEdit::singleline(&mut r#box.value)
                            .background_color(Color32::TRANSPARENT)
                            .font(boxes_style())
                            .text_color(Color32::RED)
                            .horizontal_align(Align::Max)
                            .clip_text(false),
                    );
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
                            let delta = resp.drag_delta();
                            #[expect(clippy::cast_possible_truncation)]
                            {
                                r#box.left += delta.x.round() as i32;
                                r#box.top -= delta.y.round() as i32;
                            }
                        }
                    }
                }
                ui.response()
            });
        ui.response()
    }
}
