use super::{Drawn, Img, with_tctx};
use crate::{FONT_SIZE, PADDING, Place, Spinner, diac_renderer, strs, task::Poll};
use eyre::OptionExt as _;
use iced::{
    Element, Point, Task,
    advanced::image::Bytes,
    alignment::Vertical,
    widget::{image::Handle, operation::AbsoluteOffset, row, slider, text},
};
use image::{ImageBuffer, Rgba};
use num_traits::ToPrimitive as _;
use std::{
    ffi::CStr,
    sync::{Arc, Mutex},
};
use teamim::{DATAPATH, DrawProgress, PlaceOptions, leptonica_ext::PixBox};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Draw(Poll<Drawn, DrawProgress>),
    Save,
    Saved(Result<(), Arc<eyre::Report>>),
    // <place>
    /// enters placing mode with the given miss' diacritic
    PlaceMode(usize),
    /// places discritic at [`Place::position`]
    Place {
        scroll_offset: AbsoluteOffset,
    },
    PlaceMove(Point),
    PlaceCancel,
    // </place>
    SetBlur(u32),
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedImage {
    pub(crate) img: Img,
    pub(crate) zoom: f32,
    pub(crate) drawing: Poll<Option<Drawn>, (Spinner, DrawProgress)>,
    blur: u32,
    opts: PlaceOptions,
}

impl LoadedImage {
    pub(crate) fn new(img: Img) -> Self {
        // [TODO] zoom
        Self {
            img,
            zoom: 0.4,
            drawing: Poll::Ready(Ok(None)),
            blur: 0,
            opts: PlaceOptions::default(),
        }
    }
    fn drawn_mut(&mut self) -> Option<&mut Drawn> {
        if let Poll::Ready(Ok(Some(drawn))) = &mut self.drawing { Some(drawn) } else { None }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Draw(Poll::Pending(DrawProgress::Pending)) => {
                self.drawing = Poll::Pending((Spinner::new(), DrawProgress::default()));
                let loaded = self.clone();
                Task::stream(iced_futures::stream::channel(0, async move |mut tx| {
                    let result = tokio::task::spawn_blocking({
                        let tx = Mutex::new(tx.clone());
                        move || {
                            loaded
                                .draw_teamim(DATAPATH, {
                                    move |progress| {
                                        _ = tx.lock().unwrap().try_send(Poll::Pending(progress));
                                    }
                                })
                                .map_err(Arc::new)
                        }
                    })
                    .await
                    .expect("blocking task to finish");
                    _ = tx.try_send(Poll::Ready(result));
                }))
                .map(Message::Draw)
            }
            Message::Draw(Poll::Pending(progress)) => {
                self.drawing = Poll::Pending((Spinner::new(), progress));
                Task::none()
            }
            Message::Draw(Poll::Ready(drawn_res)) => {
                self.drawing = Poll::Ready(drawn_res.map(Some));
                Task::none()
            }
            Message::Save => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                drawn.saving = Poll::Pending(Spinner::new());
                let drawn = drawn.clone();
                Task::future(async move {
                    let result = drawn.save().await.map_err(Arc::new);
                    Message::Saved(result)
                })
            }
            Message::Saved(saved) => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                drawn.saving = Poll::Ready(saved);
                Task::none()
            }
            Message::PlaceMode(miss_idx) => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                let diac = drawn.misses[miss_idx].diacritic.to_string().into();
                drawn.place_diac = Poll::Ready(Ok(Some(Place { miss_idx, diac, position: None })));
                Task::none()
            }
            Message::Place { scroll_offset } => {
                let Some(drawn) = self.drawing.ready_ok().and_then(Option::as_ref) else {
                    return Task::none();
                };
                let Some(place) = drawn.place_diac.ready_ok().and_then(Option::as_ref) else {
                    return Task::none();
                };
                let Some(&position) = place.position.as_ref() else { return Task::none() };
                let drawn = drawn.clone();
                let place = place.clone();
                let zoom = self.zoom;
                Task::future(async move {
                    let res = tokio::task::spawn_blocking(move || {
                        place_diac(&place, position, drawn, scroll_offset, zoom).map_err(Arc::new)
                    })
                    .await
                    .expect("blocking task to finish");
                    Message::Draw(Poll::Ready(res))
                })
            }
            Message::PlaceMove(point) => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                let Some(place) = drawn.place_diac.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                place.position = Some(point);
                Task::none()
            }
            Message::PlaceCancel => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                drawn.place_diac = Poll::Ready(Ok(None));
                Task::none()
            }
            Message::SetBlur(blur) => {
                self.blur = blur;
                Task::none()
            }
        }
    }

    pub(crate) fn draw_teamim(
        self,
        datapath: &'static CStr,
        progress_callback: impl Fn(DrawProgress) + Send + 'static,
    ) -> eyre::Result<Drawn> {
        let mut data = self.img.pixels.to_vec();

        let misses = PixBox::from_rgba8_with(
            &mut data,
            self.img.width.try_into()?,
            self.img.height.try_into()?,
            |img| {
                with_tctx(datapath, |ctx| ctx.place_teamim_pix(img, self.opts, progress_callback))
            },
        )?
        .unwrap_or_else(|err| Err(err.into()))?;

        let pixels = Bytes::from(data);
        let handle = Handle::from_rgba(self.img.width, self.img.height, pixels.clone());
        let img = Img { pixels, handle, ..self.img };
        Ok(Drawn::new(img, misses))
    }

    pub(crate) fn draw_tools<'a>(&self) -> Element<'a, Message> {
        let (label, state) = match self.drawing.clone() {
            Poll::Pending((spinner, progress)) => {
                (strs::draw_progress(progress), Poll::Pending(spinner))
            }
            Poll::Ready(ready) => (strs::DRAW_TEAMIM, Poll::Ready(ready)),
        };

        row![
            // blur
            slider(0..=25, self.blur, Message::SetBlur).width(FONT_SIZE * 3.0),
            text(strs::BLUR),
            // draw
            state.loading_btn(label).on_press(Message::Draw(Poll::Pending(DrawProgress::Pending))),
        ]
        .align_y(Vertical::Center)
        .spacing(PADDING as u32)
        .into()
    }
}

fn place_diac(
    place: &Place,
    position: Point,
    drawn: Drawn,
    scroll_offset: AbsoluteOffset,
    zoom: f32,
) -> eyre::Result<Drawn> {
    let x = ((position.x + scroll_offset.x) / zoom)
        .to_i32()
        .ok_or_eyre("Failed to convert x coordinate to i32")?;
    let y = ((position.y + scroll_offset.y) / zoom)
        .to_i32()
        .ok_or_eyre("Failed to convert y coordinate to i32")?;

    let Img { width, height, pixels, .. } = drawn.img;

    // [TODO]
    let mut pixels = pixels.to_vec();

    let mut img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, pixels.as_mut())
        .ok_or_eyre("failed to convert to image")?;

    let mut buf = [0; 4];
    let text = drawn.misses[place.miss_idx].diacritic.encode_utf8(&mut buf);
    diac_renderer::draw_text(&mut img, [x, y], text, FONT_SIZE);

    let misses = drawn
        .misses
        .into_iter()
        .enumerate()
        .filter_map(|(i, miss)| (i != place.miss_idx).then_some(miss))
        .collect();
    let pixels = Bytes::from(pixels);
    let handle = Handle::from_rgba(drawn.img.width, drawn.img.height, pixels.clone());
    let img = Img { pixels, handle, ..drawn.img };

    Ok(Drawn { img, misses, saving: Poll::Ready(Ok(())), place_diac: Poll::Ready(Ok(None)) })
}
