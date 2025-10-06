mod drawn;

#[cfg(test)]
mod tests;

use crate::{
    FONT_SIZE, Img, PADDING, strs,
    task::{Poll, Progress, TryPoll},
    with_tctx,
};
use drawn::Drawn;
use editor::spinner::Spinner;
use eyre::{Context, eyre};
use iced::{
    Element, Subscription, Task,
    alignment::Vertical,
    stream::channel,
    widget::{
        operation::AbsoluteOffset,
        row, slider,
        text::{self, IntoFragment},
    },
};
use std::{
    borrow::Cow,
    ffi::CStr,
    mem,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use teamim::{DATAPATH, DiacResults, PositStatus, leptonica_ext::PixBox};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    scroll_offset: AbsoluteOffset,
    zoom: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum BlurStatus {
    #[default]
    Blurring,
}

impl<Ready> IntoFragment<'static> for &Poll<Ready, Progress<BlurStatus>> {
    fn into_fragment(self) -> text::Fragment<'static> {
        Cow::Borrowed(strs::BLUR)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Draw(Poll<Arc<Mutex<eyre::Result<DiacResults>>>, PositStatus>),
    Drawn(drawn::Message),
    Tick(Instant),
    SetBlur(u32),
    Blur(Poll<Img, BlurStatus>),
}

#[derive(Debug)]
pub(crate) struct LoadedImage {
    img: Img,
    zoom: f32,
    blur: u32,
    blurring: Poll<Option<Img>, Progress<BlurStatus>>,
    drawing: TryPoll<Option<Drawn>, Progress<PositStatus>>,
}

impl LoadedImage {
    pub(crate) fn new(img: Img) -> Self {
        // [TODO] zoom
        Self {
            img,
            zoom: 0.4,
            blur: 0,
            drawing: TryPoll::Ready(Ok(None)),
            blurring: Poll::Ready(None),
        }
    }
    fn drawn_mut(&mut self) -> Option<&mut Drawn> {
        if let TryPoll::Ready(Ok(Some(drawn))) = &mut self.drawing { Some(drawn) } else { None }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Draw(msg) => self.posit_diacs(msg),
            Message::Drawn(msg) => {
                self.drawn_mut().map_or(Task::none(), |drawn| drawn.update(msg)).map(Message::Drawn)
            }
            Message::Tick(now) => {
                if let Some(progress) = self.drawing.as_mut_pending() {
                    progress.spinner.tick(now);
                }
                Task::none()
            }
            Message::SetBlur(blur) => {
                self.blur = blur;
                Task::none()
            }
            Message::Blur(msg) => match msg {
                Poll::Pending(BlurStatus::Blurring) => {
                    self.blurring = Poll::Pending(Progress::with_status(BlurStatus::Blurring));
                    let img = self.img.clone();
                    Task::future(async move {
                        spawn_blocking(move || {
                            let blurred = img.blur();
                            Message::Blur(Poll::Ready(blurred))
                        })
                        .await
                        .expect("blocking task to finish")
                    })
                }
                Poll::Ready(blurred) => {
                    self.blurring = Poll::Ready(Some(blurred.clone()));
                    Task::none()
                }
            },
        }
    }
    pub fn view(&'_ self, scroll_offset: AbsoluteOffset) -> Element<'_, Message> {
        if let Some(drawn) = self.drawing.as_ready_ok().and_then(Option::as_ref) {
            let opts = Transform { zoom: self.zoom, scroll_offset };
            drawn.view(opts, self.img()).map(Message::Drawn)
        } else {
            self.img().view(self.zoom)
        }
    }

    fn img(&self) -> &Img {
        self.blurring.as_ready().and_then(Option::as_ref).unwrap_or(&self.img)
    }

    fn posit_diacs(
        &mut self,
        msg: Poll<Arc<Mutex<eyre::Result<DiacResults, eyre::Error>>>, PositStatus>,
    ) -> Task<Message> {
        match msg {
            Poll::Pending(PositStatus::Pending) => {
                self.drawing = Poll::Pending(Progress::default());
                let img = self.img.clone();
                Task::stream(channel(1, async move |mut tx| {
                    let progress_callback = {
                        let tx = tx.clone();
                        move |progress| {
                            _ = tx.clone().try_send(Poll::Pending(progress));
                        }
                    };
                    let result =
                        spawn_blocking(move || position_diacs(&img, DATAPATH, progress_callback))
                            .await
                            .expect("blocking task to finish");
                    _ = tx.try_send(Poll::Ready(Arc::new(Mutex::new(result))));
                }))
                .map(Message::Draw)
            }
            Poll::Pending(msg) => {
                let spinner = self
                    .drawing
                    .as_pending()
                    .map_or(Spinner::default(), |progress| progress.spinner);
                self.drawing = Poll::Pending(Progress { spinner, status: msg });
                Task::none()
            }
            Poll::Ready(diac_res) => {
                let mut drawn_res = diac_res.lock().unwrap();
                let drawn_res = mem::replace(&mut *drawn_res, Err(eyre!("result taken")))
                    .map(|(positions, misses)| Drawn::new(positions, misses));
                self.drawing = Poll::Ready(drawn_res.map(Some));
                Task::none()
            }
        }
    }

    pub(crate) fn toolbar_view<'a>(&self) -> Element<'a, Message> {
        row([
            // blur
            slider(0..=25, self.blur, Message::SetBlur).width(FONT_SIZE * 3.0).into(),
            self.blurring
                .loading_btn()
                .on_press(Message::Blur(Poll::Pending(BlurStatus::Blurring)))
                .into(),
            // draw
            self.drawing
                .loading_btn()
                .on_press(Message::Draw(Poll::Pending(PositStatus::Pending)))
                .into(),
        ]
        .into_iter()
        .chain(
            // drawn
            self.drawing
                .as_ready_ok()
                .and_then(Option::as_ref)
                .map(|drawn| drawn.toolbar_view(self.img().clone()).map(Message::Drawn)),
        ))
        .align_y(Vertical::Center)
        .spacing(u32::from(PADDING))
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match &self.drawing {
            Poll::Pending(_) => iced::time::every(Duration::from_millis(16)).map(Message::Tick),
            Poll::Ready(Ok(Some(_))) => Drawn::subscription().map(Message::Drawn),
            Poll::Ready(_) => Subscription::none(),
        }
    }
}

fn position_diacs(
    img: &Img,
    datapath: &CStr,
    progress_callback: impl Fn(PositStatus),
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
