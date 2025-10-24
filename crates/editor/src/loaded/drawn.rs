#[cfg(test)]
mod tests;

use ::image::{ImageBuffer, ImageFormat, Rgb, Rgba, RgbaImage, buffer::ConvertBuffer};
use ::tap::prelude::*;
use eyre::{OptionExt as _, WrapErr as _};
use iced::{
    Color, Element, Font, Point, Subscription, Task, Vector,
    futures::StreamExt,
    keyboard::{Key, key::Named, on_key_press},
    mouse::Interaction,
    stream::channel,
    widget::{button, image, image::Handle, mouse_area, text},
};
use num_traits::AsPrimitive;
use rfd::AsyncFileDialog;
use std::{
    ffi::CStr,
    sync::{Arc, Mutex},
};
use teamim::{
    DATAPATH, DiacResult, DiacResultKind, PositStatus, leptonica_ext::PixBox,
    tesseract_ext::bounding_box::Rect,
};

use crate::{
    FONT_SIZE, IMG_EXTS, NamedImg, SaveStatus, diac_renderer,
    img::Img,
    loaded::Transform,
    stage, strs,
    task::{Poll, TryPoll},
    with_tctx,
};

pub type PollDraw = Poll<Result<Arc<[RenderedDiac]>, Arc<eyre::Report>>, PositStatus>;

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
    diac_idx: usize,
    diac: Arc<str>,
    position: Option<Point>,
}

#[derive(Debug)]
pub(super) struct RenderedDiac {
    diac: char,
    kind: DiacResultKind,
    img: Handle,
}

impl RenderedDiac {
    fn as_pos(&self) -> Option<(char, Rect<u32>)> {
        let DiacResultKind::Pos(rect) = self.kind else { return None };
        Some((self.diac, rect))
    }
}

#[derive(Debug)]
pub struct Drawn {
    diacs: Arc<[RenderedDiac]>,
    saving: Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>,
    place_diac: TryPoll<Option<Place>>,
}

impl Drawn {
    pub fn new(diacs: Arc<[RenderedDiac]>) -> Self {
        Self { diacs, saving: Poll::Ready(Ok(())), place_diac: Poll::Ready(Ok(None)) }
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Save(msg) => match msg {
                // trigger
                Poll::Pending(SaveStatus::Trigger(img, transform)) => Task::stream(
                    channel(1, {
                        let positions = Arc::clone(&self.diacs);
                        async move |mut tx| {
                            let positions = positions.iter().filter_map(RenderedDiac::as_pos);
                            let report = |status| _ = tx.clone().try_send(Poll::Pending(status));
                            let result =
                                save(img, positions, transform, report).await.map_err(Arc::new);
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
            Message::PlaceMode(diac_idx) => {
                let diac = self.diacs[diac_idx].diac.to_string().into();
                self.place_diac =
                    TryPoll::Ready(Ok(Some(Place { diac_idx, diac, position: None })));
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

    pub fn diac_view(&self, opts: Transform) -> Element<'_, Message> {
        let Transform { scroll_offset, zoom } = opts;
        let place = self.place_diac.as_ready_ok().and_then(Option::as_ref);
        mouse_area(stage(Iterator::chain(
            // place
            self.place_diac
                .as_ready_ok()
                .and_then(Option::as_ref)
                .and_then(|place| {
                    let position = place.position?;
                    Some((
                        // [TODO] color(black)
                        mouse_area(text(place.diac.as_ref()).size(FONT_SIZE))
                            .interaction(Interaction::Crosshair)
                            .on_move(move |offset| {
                                Message::PlaceMove(position + Vector::new(offset.x, offset.y))
                            })
                            .on_press(Message::Place(Transform { scroll_offset, zoom }))
                            .into(),
                        position,
                    ))
                })
                .into_iter(),
            // positioned
            self.diacs.iter().filter_map(|pos| {
                let DiacResultKind::Pos(rect) = pos.kind else { return None };
                Some((
                    image(pos.img.clone()).into(),
                    #[expect(clippy::cast_precision_loss)]
                    [rect.left, rect.top].map(|coord| coord as f32 * zoom).into(),
                ))
            }),
        )))
        .on_move(Message::PlaceMove)
        .on_press(Message::Place(Transform { scroll_offset, zoom }))
        .interaction(if place.is_some() { Interaction::Crosshair } else { Interaction::default() })
        .into()
    }

    pub(crate) fn misses_view(&self, zoom: f32) -> Element<'_, Message> {
        stage(self.diacs.iter().enumerate().filter_map(|(result_idx, result)| {
            let DiacResultKind::Miss(miss) = &result.kind else {
                return None;
            };
            Some((
                button(text(&miss.missing_text).size(48.0 * zoom))
                    .on_press_maybe(
                        if let Some(place) = self.place_diac.as_ready_ok().and_then(Option::as_ref)
                            && place.diac_idx == result_idx
                        {
                            None
                        } else {
                            Some(Message::PlaceMode(result_idx))
                        },
                    )
                    .into(),
                Point::new(0.0, AsPrimitive::<f32>::as_(miss.top) * zoom),
            ))
        }))
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
    positions: impl IntoIterator<Item = (char, Rect<u32>)>,
    transform: Transform,
    progress: impl Fn(SaveStatus),
) -> eyre::Result<()> {
    let path = img.path;
    let mut img = img.img.img().to_rgba();

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

fn render(
    positions: impl IntoIterator<Item = (char, Rect<u32>)>,
    transform: Transform,
    img: &mut RgbaImage,
) {
    let mut renderer = diac_renderer::Renderer::new();
    for (diac, rect) in positions {
        // DEBUG
        draw_red_rectangle(img, rect);

        let Rect { left, bottom, .. } = rect;
        // let pos = match GLYPHS[&diac.diacritic].placement {
        //     Placement::Top => [left, top],
        //     Placement::Bottom => [left, bottom],
        //     // [TODO]
        //     Placement::After => [left - FONT_SIZE as i32 / 2, top],
        // };

        // [TODO] cache, optimize
        renderer.draw_glyph(img, diac, [left, bottom], FONT_SIZE * transform.zoom);
    }
}

/// Draws a red rectangle onto an RGBA image.
///
/// Coordinates are inclusive on top/left and exclusive on bottom/right.
pub fn draw_red_rectangle(img: &mut RgbaImage, rect: Rect<u32>) {
    let red = Rgba([255, 0, 0, 255]);

    let Rect { left, bottom, right, top } = rect;

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
pub(super) fn posit_diacs(
    img: &Img,
    datapath: &CStr,
    zoom: f32,
    progress_callback: impl Fn(PositStatus),
) -> eyre::Result<Arc<[RenderedDiac]>> {
    let mut renderer = diac_renderer::Renderer::new();
    PixBox::from_rgba8_with(
        &mut img.pixels.to_vec(),
        img.width.try_into()?,
        img.height.try_into()?,
        |img| {
            with_tctx(datapath, |ctx| ctx.positions(img, progress_callback).wrap_err("place error"))
        },
    )???
    .into_iter()
    .map(|(diac, kind)| RenderedDiac {
        diac,
        kind,
        img: renderer
            .render(diac, FONT_SIZE * zoom)
            .pipe(|img| Handle::from_rgba(img.width(), img.height(), img.into_raw())),
    })
    .collect::<Arc<[_]>>()
    .pipe(Ok)
}
