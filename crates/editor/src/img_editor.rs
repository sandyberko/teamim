use eframe::egui::{
    Align, Button, Color32, CursorIcon, Id, Image, InputState, Key, Layout, Modifiers, Rect,
    Response, Scene, Sense, Stroke, StrokeKind, TextEdit, TextureHandle, Ui, UiBuilder, Vec2,
    Widget, pos2, text::CursorRange, util::undoer::Undoer, vec2,
};
use teamim::tesseract_ext::BoundingBox;

use crate::{EditableRect, FONT_SIZE, boxes_style};

const DISPLAY_OPT_LEN: usize = 4;
const DISPLAY_OPTS: [(&str, Key); DISPLAY_OPT_LEN] = [
    ("image", Key::F1),
    ("frames", Key::F2),
    ("text", Key::F3),
    ("solo", Key::F4),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResizeDir {
    North,
    East,
    South,
    West,
}

pub(crate) struct LoadedData {
    texture: TextureHandle,
    image_size: Vec2,
    pub(crate) boxes: Vec<EditableRect>,

    // ui
    scene_rect: Rect,
    display_opts: [bool; DISPLAY_OPT_LEN],
    selected_rect: usize,
    kbd_resize_dir: Option<ResizeDir>,
    undoer: Undoer<Vec<EditableRect>>,
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
            boxes: editable_rects,
            scene_rect: Rect::ZERO,
            display_opts: [true; DISPLAY_OPT_LEN],
            selected_rect: 0,
            undoer: Undoer::default(),
            kbd_resize_dir: None,
        }
    }
    // Convert EditableRects back to TextBoxes (bottom-left origin)
    #[expect(clippy::cast_possible_truncation, unused)]
    pub(crate) fn get_textboxes(&self) -> Vec<BoundingBox<String>> {
        self.boxes
            .iter()
            .map(|r| BoundingBox {
                value: r.text.clone(),
                left: r.rect.min.x.round() as i32,
                bottom: (self.image_size.y - r.rect.max.y) as i32,
                right: r.rect.max.x as i32,
                top: (self.image_size.y - r.rect.min.y) as i32,
            })
            .collect()
    }
}

impl Widget for &mut LoadedData {
    fn ui(self, ui: &mut Ui) -> Response {
        // display options keyboard shortcuts
        ui.input_mut(|inp| {
            for ((_, key), enabled) in DISPLAY_OPTS.iter().zip(&mut self.display_opts) {
                if inp.key_pressed(*key) {
                    *enabled = false;
                } else if !inp.modifiers.shift && inp.key_released(*key) {
                    *enabled = true;
                }
            }

            // move between rows
            if inp.consume_key(Modifiers::NONE, Key::ArrowUp) {
                self.selected_rect = self.selected_rect.saturating_sub(1);
            } else if inp.consume_key(Modifiers::NONE, Key::ArrowDown) {
                self.selected_rect = (self.selected_rect + 1).min(self.boxes.len() - 1);
            }

            // undo/redo
            if inp.consume_key(Modifiers::CTRL, Key::Z) {
                self.undoer.undo(&self.boxes);
            } else if inp.consume_key(Modifiers::CTRL, Key::Y) {
                self.undoer.redo(&self.boxes);
            }

            self.handle_kbd_resize(inp);
        });

        let can_undo = self.undoer.has_undo(&self.boxes);
        let can_redo = self.undoer.has_redo(&self.boxes);

        ui.horizontal(|ui| {
            for ((name, _), enabled) in DISPLAY_OPTS.iter().zip(&mut self.display_opts) {
                ui.checkbox(enabled, *name);
            }
            ui.separator();

            let undo = ui.add_enabled(can_undo, Button::new("⟲ Undo")).clicked();
            let redo = ui.add_enabled(can_redo, Button::new("⟳ Redo")).clicked();

            if undo {
                if let Some(undo_text) = self.undoer.undo(&self.boxes) {
                    undo_text.clone_into(&mut self.boxes);
                }
            }
            if redo {
                if let Some(redo_text) = self.undoer.redo(&self.boxes) {
                    redo_text.clone_into(&mut self.boxes);
                }
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

                let interactive = self.kbd_resize_dir.is_none();
                if self.display_opts[3] {
                    for bx_i in 0..self.boxes.len() {
                        let selected = self.selected_rect == bx_i;
                        let resp = show_rect(
                            &mut self.boxes[bx_i],
                            self.display_opts,
                            selected,
                            interactive,
                            self.kbd_resize_dir,
                            ui,
                        );
                        if resp.inner.gained_focus() {
                            self.selected_rect = bx_i;
                        }
                        match resp.line_down {
                            Some((LineDown::NextLine, text)) if bx_i < self.boxes.len() - 1 => {
                                self.boxes[bx_i].text.insert_str(0, &text);
                            }
                            // new line
                            Some((_, text)) => {
                                let rect =
                                    self.boxes[bx_i].rect.translate(vec2(0.0, FONT_SIZE + 10.0));
                                self.boxes.insert(bx_i + 1, EditableRect { rect, text });
                                self.selected_rect = bx_i + 1;
                            }
                            None => {}
                        }
                    }
                } else {
                    show_rect(
                        &mut self.boxes[self.selected_rect],
                        self.display_opts,
                        true,
                        interactive,
                        self.kbd_resize_dir,
                        ui,
                    );
                }
                ui.response()
            });

        self.undoer
            .feed_state(ui.ctx().input(|input| input.time), &self.boxes);

        ui.response()
    }
}

impl LoadedData {
    fn handle_kbd_resize(&mut self, inp: &mut InputState) {
        const SZ: f32 = 1.0;

        if inp.key_pressed(Key::Escape) {
            self.kbd_resize_dir = None;
        } else if inp.modifiers.alt {
            if inp.key_pressed(Key::W) {
                self.kbd_resize_dir = Some(ResizeDir::North);
            } else if inp.key_pressed(Key::A) {
                self.kbd_resize_dir = Some(ResizeDir::West);
            } else if inp.key_pressed(Key::S) {
                self.kbd_resize_dir = Some(ResizeDir::South);
            } else if inp.key_pressed(Key::D) {
                self.kbd_resize_dir = Some(ResizeDir::East);
            }
        } else if let Some(dir) = self.kbd_resize_dir {
            let bx = &mut self.boxes[self.selected_rect];
            let xd = if inp.consume_key(Modifiers::NONE, Key::A) {
                -SZ
            } else if inp.consume_key(Modifiers::NONE, Key::D) {
                SZ
            } else {
                0.0
            };
            let yd = if inp.consume_key(Modifiers::NONE, Key::W) {
                -SZ
            } else if inp.consume_key(Modifiers::NONE, Key::S) {
                SZ
            } else {
                0.0
            };
            match dir {
                ResizeDir::North => bx.rect.min.y += yd,
                ResizeDir::East => bx.rect.max.x += xd,
                ResizeDir::South => bx.rect.max.y += yd,
                ResizeDir::West => bx.rect.min.x += xd,
            }
        }
    }
}

const FRAME_W: f32 = 5.0;

struct BoxResponse {
    inner: Response,
    line_down: Option<(LineDown, String)>,
}

enum LineDown {
    NextLine,
    NewLine,
}

fn show_rect(
    bx: &mut EditableRect,
    display_opts: [bool; DISPLAY_OPT_LEN],
    selected: bool,
    interactive: bool,
    kbd_resize_dir: Option<ResizeDir>,
    ui: &mut Ui,
) -> BoxResponse {
    let text_color = if display_opts[2] {
        Color32::RED
    } else {
        Color32::TRANSPARENT
    };

    // text
    let resp = ui.allocate_new_ui(
        UiBuilder::new()
            .layout(Layout::right_to_left(Align::Min))
            .max_rect(bx.rect),
        |ui| {
            TextEdit::singleline(&mut bx.text)
                .interactive(!selected || interactive)
                .clip_text(false)
                .frame(false)
                .background_color(Color32::TRANSPARENT)
                .text_color(text_color)
                .show(ui)
        },
    );

    if selected {
        resp.inner.response.request_focus();
    }

    // frame
    if display_opts[1] {
        if selected {
            show_selected_frame(bx, kbd_resize_dir, ui);
        } else {
            ui.painter().rect_stroke(
                bx.rect,
                0.0,
                Stroke::new(FRAME_W, Color32::GRAY),
                StrokeKind::Outside,
            );
        };
    }

    // interaction
    let mut line_down = None;
    if selected {
        if ui.input(|inp| inp.modifiers.ctrl) {
            let resp = &resp.inner.response;
            if resp.hovered() {
                ui.ctx().output_mut(|o| o.cursor_icon = CursorIcon::Move);
            }
            if resp.dragged() {
                bx.rect = bx.rect.translate(resp.drag_delta());
            }
        }

        ui.input(|inp| {
            // new line
            if inp.key_pressed(Key::Enter) {
                let cursor = resp
                    .inner
                    .cursor_range
                    .as_ref()
                    .and_then(CursorRange::single);
                if let Some(cursor) = cursor {
                    let ty = if inp.modifiers.shift {
                        LineDown::NextLine
                    } else {
                        LineDown::NewLine
                    };
                    let Some((byte_idx, _)) = bx.text.char_indices().nth(cursor.ccursor.index)
                    else {
                        eprintln!("char index out of bounds");
                        return;
                    };
                    line_down = Some((ty, bx.text.split_off(byte_idx)));
                }
            }
        });
    }

    BoxResponse {
        inner: resp.inner.response,
        line_down,
    }
}

fn show_selected_frame(bx: &mut EditableRect, kbd_resize_dir: Option<ResizeDir>, ui: &mut Ui) {
    let rect_n = Rect::from_min_size(
        bx.rect.min - Vec2::splat(FRAME_W),
        vec2(bx.rect.width() + FRAME_W * 2.0, FRAME_W),
    );
    let rect_e = Rect::from_min_max(
        pos2(bx.rect.max.x, bx.rect.min.y - FRAME_W),
        pos2(bx.rect.max.x + FRAME_W, bx.rect.max.y + FRAME_W),
    );
    let rect_s = Rect::from_min_max(
        pos2(bx.rect.min.x - FRAME_W, bx.rect.max.y),
        pos2(bx.rect.max.x + FRAME_W, bx.rect.max.y + FRAME_W),
    );
    let rect_w = Rect::from_min_max(
        pos2(bx.rect.min.x - FRAME_W, bx.rect.min.y - FRAME_W),
        pos2(bx.rect.min.x, bx.rect.max.y + FRAME_W),
    );

    // resize
    let sense = Sense::DRAG | Sense::HOVER;
    let threshold = 10.0;

    let hover_n = {
        let resp = ui.interact(
            rect_n.expand(threshold),
            Id::new("resize_handle_top"),
            sense,
        );
        if resp.dragged() {
            bx.rect.min.y += resp.drag_delta().y;
        }
        resp.hovered()
    };
    let hover_e = {
        let resp = ui.interact(
            rect_e.expand(threshold),
            Id::new("resize_handle_right"),
            sense,
        );
        if resp.dragged() {
            bx.rect.max.x += resp.drag_delta().x;
        }
        resp.hovered()
    };
    let hover_s = {
        let resp = ui.interact(
            rect_s.expand(threshold),
            Id::new("resize_handle_bottom"),
            sense,
        );
        if resp.dragged() {
            bx.rect.max.y += resp.drag_delta().y;
        }
        resp.hovered()
    };
    let hover_w = {
        let resp = ui.interact(
            rect_w.expand(threshold),
            Id::new("resize_handle_left"),
            sense,
        );
        if resp.dragged() {
            bx.rect.min.x += resp.drag_delta().x;
        }
        resp.hovered()
    };
    let icon = match (hover_n, hover_e, hover_s, hover_w) {
        (true, false, false, false) | (false, false, true, false) => CursorIcon::ResizeVertical,
        (false, false, false, true) | (false, true, false, false) => CursorIcon::ResizeHorizontal,
        (true, false, false, true) | (false, true, true, false) => CursorIcon::ResizeNwSe,
        (true, true, false, false) | (false, false, true, true) => CursorIcon::ResizeNeSw,

        _ => ui.ctx().output(|o| o.cursor_icon),
    };
    ui.ctx().output_mut(|o| o.cursor_icon = icon);

    // frame
    let dirs = [
        (rect_n, ResizeDir::North),
        (rect_e, ResizeDir::East),
        (rect_s, ResizeDir::South),
        (rect_w, ResizeDir::West),
    ];
    for (rect, dir) in dirs {
        let fill_color = if kbd_resize_dir == Some(dir) {
            Color32::CYAN
        } else {
            Color32::RED
        };
        ui.painter().rect_filled(rect, 0.0, fill_color);
    }
}
