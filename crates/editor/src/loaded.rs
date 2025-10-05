#[cfg(test)]
mod tests;

use super::{Drawn, Img, with_tctx};
use crate::{
    FONT_SIZE, PADDING, Place, Spinner, diac_renderer, strs,
    task::{Poll, TryPoll},
};
use eyre::{Context, OptionExt as _, eyre};
use iced::{
    Element, Point, Task,
    advanced::image::Bytes,
    alignment::Vertical,
    futures::future::Ready,
    widget::{image::Handle, operation::AbsoluteOffset, row, slider, text},
};
use iced_futures::stream;
use image::{ImageBuffer, Rgba};
use num_traits::ToPrimitive as _;
use std::{
    ffi::CStr,
    mem,
    sync::{Arc, Mutex},
};
use teamim::{DATAPATH, DiacResults, PlaceOptions, PositProgress, leptonica_ext::PixBox};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    PositDiacs(Poll<Arc<Mutex<eyre::Result<DiacResults>>>, PositProgress>),
    // [TODO] move to `mod loaded`, refactor to `Poll`
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

#[derive(Debug)]
pub(crate) struct LoadedImage {
    pub(crate) img: Img,
    pub(crate) zoom: f32,
    pub(crate) drawing: TryPoll<Option<Drawn>, (Spinner, PositProgress)>,
    blur: u32,
    opts: PlaceOptions,
}

impl LoadedImage {
    pub(crate) fn new(img: Img) -> Self {
        // [TODO] zoom
        Self {
            img,
            zoom: 0.4,
            drawing: TryPoll::Ready(Ok(None)),
            blur: 0,
            opts: PlaceOptions::default(),
        }
    }
    fn drawn_mut(&mut self) -> Option<&mut Drawn> {
        if let TryPoll::Ready(Ok(Some(drawn))) = &mut self.drawing { Some(drawn) } else { None }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::PositDiacs(msg) => self.posit_diacs(msg),
            Message::Save => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                // [TODO] Task::batch
                drawn.saving = Poll::Pending(Spinner::new());
                let img = drawn.img.clone();
                Task::future(async move {
                    let result = img.save().await.map_err(Arc::new);
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
                drawn.place_diac =
                    TryPoll::Ready(Ok(Some(Place { miss_idx, diac, position: None })));
                Task::none()
            }
            Message::Place { scroll_offset } => {
                let Some(drawn) = self.drawing.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                // [TODO] offload
                if let Err(err) = drawn.place_diac(scroll_offset, self.zoom) {
                    self.drawing = Poll::Ready(Err(err));
                }
                Task::none()
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
                drawn.place_diac = TryPoll::Ready(Ok(None));
                Task::none()
            }
            Message::SetBlur(blur) => {
                self.blur = blur;
                Task::none()
            }
        }
    }

    fn posit_diacs(
        &mut self,
        msg: Poll<Arc<Mutex<eyre::Result<DiacResults, eyre::Error>>>, PositProgress>,
    ) -> Task<Message> {
        match msg {
            Poll::Pending(PositProgress::Pending) => {
                self.drawing = Poll::Pending((Spinner::new(), PositProgress::default()));
                let img = self.img.clone();
                Task::stream(stream::channel(0, async move |mut tx| {
                    let progress_callback = {
                        let tx = tx.clone();
                        move |progress| {
                            _ = tx.clone().try_send(Poll::Pending(progress));
                        }
                    };
                    let result =
                        spawn_blocking(move || position_diacs(img, DATAPATH, progress_callback))
                            .await
                            .expect("blocking task to finish");
                    _ = tx.try_send(Poll::Ready(Arc::new(Mutex::new(result))));
                }))
                .map(Message::PositDiacs)
            }
            Poll::Pending(progress) => {
                self.drawing = Poll::Pending((Spinner::new(), progress));
                Task::none()
            }
            Poll::Ready(diac_res) => {
                let mut drawn_res = diac_res.lock().unwrap();
                let drawn_res = mem::replace(&mut *drawn_res, Err(eyre!("result taken")))
                    .map(|(positions, misses)| Drawn::new(self.img.clone(), positions, misses));
                self.drawing = Poll::Ready(drawn_res.map(Some));
                Task::none()
            }
        }
    }

    pub(crate) fn draw_tools<'a>(&self) -> Element<'a, Message> {
        let (label, state) = match &self.drawing {
            Poll::Pending((spinner, progress)) => {
                (strs::draw_progress(*progress), Poll::Pending(*spinner))
            }
            Poll::Ready(ready) => (strs::DRAW_TEAMIM, Poll::Ready(ready)),
        };

        row![
            // blur
            slider(0..=25, self.blur, Message::SetBlur).width(FONT_SIZE * 3.0),
            text(strs::BLUR),
            // draw
            state
                .loading_btn(label)
                .on_press(Message::PositDiacs(Poll::Pending(PositProgress::Pending))),
        ]
        .align_y(Vertical::Center)
        .spacing(PADDING as u32)
        .into()
    }
}

fn position_diacs(
    img: Img,
    datapath: &CStr,
    progress_callback: impl Fn(PositProgress),
) -> Result<(Vec<teamim::DiacPos>, Vec<teamim::DiacMiss>), eyre::Error> {
    PixBox::from_rgba8_with(
        &mut img.pixels.to_vec(),
        img.width.try_into()?,
        img.height.try_into()?,
        |img| {
            with_tctx(datapath, |ctx| ctx.positions(img, progress_callback).wrap_err("place error"))
        },
    )
    .flatten()
    .flatten()
}
