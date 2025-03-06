use eframe::egui::{
    Align, Color32, CursorIcon, Image, Key, Layout, Response, Scene, Sense, Stroke, StrokeKind,
    TextEdit, Ui, UiBuilder, Widget,
};

use crate::{LoadedData, boxes_style};

impl Widget for &mut LoadedData {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.visuals_mut().extreme_bg_color = Color32::TRANSPARENT;
        ui.visuals_mut().override_text_color = Some(Color32::RED);
        ui.style_mut().override_text_style = Some(boxes_style());

        Scene::new()
            .max_inner_size(self.image_size)
            .zoom_range(0.1..=20.0)
            .show(ui, &mut self.scene_rect, |ui| {
                ui.add_sized(self.image_size, Image::new(&self.texture));
                for bx in &mut self.editable_rects {
                    let resp = ui.allocate_new_ui(
                        UiBuilder::new()
                            .layout(Layout::right_to_left(Align::Min))
                            .max_rect(bx.rect),
                        |ui| {
                            let resp = ui.add(
                                TextEdit::singleline(&mut bx.text)
                                    .clip_text(false)
                                    .frame(false),
                            );
                            let color = if resp.has_focus() {
                                Color32::RED
                            } else {
                                Color32::GRAY
                            };
                            ui.painter().rect_stroke(
                                bx.rect,
                                0.0,
                                Stroke::new(5.0, color),
                                StrokeKind::Outside,
                            );
                            resp
                        },
                    );
                    let resp = resp.inner;
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
                }
                ui.response()
            });
        ui.response()
    }
}
