#[cfg(test)]
mod tests;

use std::{iter::once, sync::Arc};

use eyre::{OptionExt as _, WrapErr as _};
use iced::{
    Border, Color, Element, Font, Point, Subscription, Task, Vector,
    futures::StreamExt,
    keyboard::{Key, key::Named, on_key_press},
    mouse::Interaction,
    stream::channel,
    widget::{bottom_right, button, canvas, container, mouse_area, row, stack, text},
};
use image::{ImageBuffer, ImageFormat, Rgb, Rgba, RgbaImage, buffer::ConvertBuffer};
use num_traits::AsPrimitive;
use rfd::AsyncFileDialog;
use teamim::{
    DiacMiss, DiacPos,
    glyph::{GLYPHS, Placement, SPACED},
    tesseract_ext::bounding_box::Rect,
};

use crate::{
    FONT_SIZE, IMG_EXTS, NamedImg, SaveStatus, diac_renderer,
    loaded::Transform,
    stage, strs,
    task::{Poll, TryPoll},
};

#[derive(Debug, Clone)]
pub enum Message {
    Save(Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>),
    // <place>
    /// enters placing mode with the given miss' diacritic
    PlaceMode(usize),
    /// places discritic at [`Place::position`]
    Place(Transform),
    PlaceMove(Point),
    PlaceCancel,
    // </place>
}
#[derive(Debug, Clone)]
struct Place {
    miss_idx: usize,
    diac: Arc<str>,
    position: Option<Point>,
}

#[derive(Debug)]
pub struct Drawn {
    positions: Vec<DiacPos>,
    misses: Vec<DiacMiss>,
    saving: Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>,
    place_diac: TryPoll<Option<Place>>,
}

impl Drawn {
    pub fn new(positions: Vec<DiacPos>, misses: Vec<DiacMiss>) -> Self {
        Self { positions, misses, saving: Poll::Ready(Ok(())), place_diac: Poll::Ready(Ok(None)) }
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Save(msg) => match msg {
                // trigger
                Poll::Pending(SaveStatus::Trigger(img, transform)) => Task::stream(
                    channel(1, {
                        let positions = self.positions.clone();
                        async move |mut tx| {
                            let report = |status| _ = tx.clone().try_send(Poll::Pending(status));
                            let result =
                                save(img, &positions, transform, report).await.map_err(Arc::new);
                            _ = tx.try_send(Poll::Ready(result));
                        }
                    })
                    .map(Message::Save),
                ),
                Poll::Pending(status) => {
                    self.saving = Poll::Pending(status);
                    Task::none()
                }
                Poll::Ready(msg) => {
                    self.saving = Poll::Ready(msg);
                    Task::none()
                }
            },
            Message::PlaceMode(miss_idx) => {
                let diac = self.misses[miss_idx].diacritic.to_string().into();
                self.place_diac =
                    TryPoll::Ready(Ok(Some(Place { miss_idx, diac, position: None })));
                Task::none()
            }
            Message::Place(opts) => {
                // [TODO] offload
                if let Err(err) = self.place_diac(opts) {
                    self.place_diac = Poll::Ready(Err(err));
                }
                Task::none()
            }
            Message::PlaceMove(point) => {
                let Some(place) = self.place_diac.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                place.position = Some(point);
                Task::none()
            }
            Message::PlaceCancel => {
                self.place_diac = TryPoll::Ready(Ok(None));
                Task::none()
            }
        }
    }

    pub fn subscription() -> Subscription<Message> {
        on_key_press(|key, _| (key == Key::Named(Named::Escape)).then_some(Message::PlaceCancel))
    }

    pub fn toolbar_view<'a>(
        &self,
        img_to_save: NamedImg,
        transform: Transform,
    ) -> Element<'a, Message> {
        self.saving
            .loading_btn()
            .on_press(Message::Save(Poll::Pending(SaveStatus::Trigger(img_to_save, transform))))
            .into()
    }

    pub fn view(&self, opts: Transform, img: &NamedImg) -> Element<'_, Message> {
        let Transform { scroll_offset, zoom } = opts;
        let place = self.place_diac.as_ready_ok().and_then(Option::as_ref);
        row([
            // misses
            stage(self.misses.iter().enumerate().map(|(miss_idx, miss)| {
                (
                    button(text(miss.missing_text.as_ref()).size(48.0 * zoom))
                        .on_press_maybe(
                            if let Some(place) =
                                self.place_diac.as_ready_ok().and_then(Option::as_ref)
                                && place.miss_idx == miss_idx
                            {
                                None
                            } else {
                                Some(Message::PlaceMode(miss_idx))
                            },
                        )
                        .into(),
                    Point::new(0.0, AsPrimitive::<f32>::as_(miss.top) * zoom),
                )
            }))
            .into(),
            stack([
                mouse_area(stage(
                    Iterator::chain(
                        // image
                        once((img.view(zoom), Point::ORIGIN)),
                        // place
                        self.place_diac.as_ready_ok().and_then(Option::as_ref).and_then(|place| {
                            let position = place.position?;
                            Some((
                                // [TODO] color(black)
                                mouse_area(text(place.diac.as_ref()).size(FONT_SIZE))
                                    .interaction(Interaction::Crosshair)
                                    .on_move(move |offset| {
                                        Message::PlaceMove(
                                            position + Vector::new(offset.x, offset.y),
                                        )
                                    })
                                    .on_press(Message::Place(Transform { scroll_offset, zoom }))
                                    .into(),
                                position,
                            ))
                        }),
                    )
                    // positioned diacs
                    .chain(self.positions.iter().map(|pos| {
                        let spaced = SPACED.get(&pos.diacritic).copied().unwrap_or("?");
                        (
                            text(spaced)
                                .color(Color::from_rgb(1., 0., 0.))
                                .size(FONT_SIZE * zoom)
                                .font(Font::with_name("Guttman Stam"))
                                .into(),
                            #[expect(clippy::cast_precision_loss)]
                            [pos.rect.left, pos.rect.top].map(|coord| coord as f32 * zoom).into(),
                        )
                    })),
                ))
                .on_move(Message::PlaceMove)
                .on_press(Message::Place(Transform { scroll_offset, zoom }))
                .interaction(if place.is_some() {
                    Interaction::Crosshair
                } else {
                    Interaction::default()
                })
                .into(),
                // debug
                stage(self.positions.iter().map(|pos| {
                    (
                        container("")
                            .width(pos.rect.width())
                            .height(pos.rect.height())
                            .style(|_| container::Style {
                                border: Border::default()
                                    .width(2)
                                    .color(Color::from_rgb8(255, 0, 0)),
                                ..Default::default()
                            })
                            .into(),
                        #[expect(clippy::cast_precision_loss)]
                        [pos.rect.left, pos.rect.top].map(|coord| coord as f32 * zoom).into(),
                    )
                }))
                .into(),
            ])
            .into(),
        ])
        .into()
    }

    fn place_diac(&mut self, _opts: Transform) -> eyre::Result<()> {
        todo!()
        // let Some(place) = self.place_diac.ready_ok_mut().and_then(Option::as_mut) else {
        //     bail!("stale")
        // };
        // let Some(&position) = place.position.as_ref() else { bail!("stale") };
        // let Transform { scroll_offset, zoom } = opts;

        // let x = ((position.x + scroll_offset.x) / zoom)
        //     .to_i32()
        //     .ok_or_eyre("Failed to convert x coordinate to i32")?;
        // let y = ((position.y + scroll_offset.y) / zoom)
        //     .to_i32()
        //     .ok_or_eyre("Failed to convert y coordinate to i32")?;

        // [TODO]
        // let mut pixels = self.img.pixels.to_vec();

        // let mut img =
        //     ImageBuffer::<Rgba<u8>, _>::from_raw(self.img.width, self.img.height, pixels.as_mut())
        //         .ok_or_eyre("failed to convert to image")?;

        // let mut buf = [0; 4];
        // let text = self.misses[place.miss_idx].diacritic.encode_utf8(&mut buf);
        // diac_renderer::draw_text(&mut img, [x, y], text, FONT_SIZE);

        // let pixels = Bytes::from(pixels);
        // let handle = Handle::from_rgba(self.img.width, self.img.height, pixels.clone());
        // self.img = Img { pixels, handle, ..self.img.clone() };
        // self.misses.remove(place.miss_idx);
        // Ok(())
    }
}

async fn save(
    img: NamedImg,
    positions: &[DiacPos],
    transform: Transform,
    progress: impl Fn(SaveStatus),
) -> eyre::Result<()> {
    let path = img.path;
    let mut img = img.img.to_rgba();

    progress(SaveStatus::SelectingFile);
    // [TODO]
    let mut dialog = AsyncFileDialog::new().set_title(strs::SAVE).add_filter("image", IMG_EXTS);
    if let Some(dir) = path.parent() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(file_name) = path.file_name() {
        dialog = dialog.set_file_name(file_name.to_str().ok_or_eyre("שם הקובץ לא תקין")?);
    }
    let file = dialog.save_file().await.ok_or_eyre("שמירה בוטלה")?;

    progress(SaveStatus::Rendering);
    render(positions, transform, &mut img);

    progress(SaveStatus::Writing);
    let format = ImageFormat::from_path(file.path())?;
    if format == ImageFormat::Jpeg {
        // [FIXME]: [red] diacs?
        ConvertBuffer::<ImageBuffer<Rgb<u8>, _>>::convert(&img)
            .save_with_format(file.path(), format)
            .wrap_err(strs::SAVE_FAILED)?;
    } else {
        img.save_with_format(file.path(), format).wrap_err(strs::SAVE_FAILED)?;
    }
    Ok(())
}

fn render(positions: &[DiacPos], transform: Transform, img: &mut RgbaImage) {
    let mut renderer = diac_renderer::Renderer::new();
    for diac in positions {
        // debug
        draw_red_rectangle(img, diac.rect);

        let Rect { left, bottom, top, .. } = diac.rect;
        // let pos = match GLYPHS[&diac.diacritic].placement {
        //     Placement::Top => [left, top],
        //     Placement::Bottom => [left, bottom],
        //     // [TODO]
        //     Placement::After => [left - FONT_SIZE as i32 / 2, top],
        // };

        // [TODO] cache, optimize
        renderer.draw_glyph(img, diac.diacritic, [left, bottom].into(), FONT_SIZE * transform.zoom);
    }
}

/// Draws a red rectangle onto an RGBA image.
///
/// Coordinates are inclusive on top/left and exclusive on bottom/right.
pub fn draw_red_rectangle(img: &mut RgbaImage, rect: Rect) {
    let red = Rgba([255, 0, 0, 255]);

    let Rect { left, bottom, right, top } = rect.map(i32::unsigned_abs);

    // Draw horizontal edges
    for x in left..right {
        img.put_pixel(x, top, red);

        img.put_pixel(x, bottom - 1, red);
    }

    // Draw vertical edges
    for y in top..bottom {
        img.put_pixel(left, y, red);

        img.put_pixel(right - 1, y, red);
    }
}

struct PosProg<'a> {
    diacs: &'a [DiacPos],
}

impl canvas::Program for PosProg<'_> {
    type State = ();

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &iced_renderer::core::Theme,
        bounds: iced::Rectangle,
        cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        todo!()
    }
}
