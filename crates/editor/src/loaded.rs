mod drawn;

use crate::{FONT_SIZE, NamedImg, PADDING, img::ImgHandle, strs, task::Poll};
use drawn::Drawn;
use iced::{
    Element, Task,
    alignment::Vertical,
    stream::channel,
    widget::{
        row, slider, stack,
        text::{self, IntoFragment},
    },
};
use std::{borrow::Cow, iter::once, sync::Arc};
use tap::prelude::*;
use teamim::{DATAPATH, PositStatus};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Copy, Default)]
pub enum BlurStatus {
    #[default]
    Blurring,
}

impl<Ready> IntoFragment<'static> for &Poll<Ready, BlurStatus> {
    fn into_fragment(self) -> text::Fragment<'static> {
        Cow::Borrowed(strs::BLUR)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Draw(drawn::PollDraw),
    Drawn(drawn::Message),
    SetBlur(u32),
    Blur(Poll<ImgHandle, BlurStatus>),
}

#[derive(Debug)]
pub(crate) struct LoadedImage {
    img: NamedImg,
    zoom: f32,
    blur: u32,
    blurring: Poll<Option<ImgHandle>, BlurStatus>,
    drawing: Poll<Result<Option<Drawn>, Arc<eyre::Report>>, PositStatus>,
}

impl LoadedImage {
    pub(crate) fn new(img: NamedImg) -> Self {
        Self {
            img,
            zoom: 0.4,
            blur: 0,
            drawing: Poll::Ready(Ok(None)),
            blurring: Poll::Ready(None),
        }
    }
    fn drawn_mut(&mut self) -> Option<&mut Drawn> {
        if let Poll::Ready(Ok(Some(drawn))) = &mut self.drawing { Some(drawn) } else { None }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Draw(msg) => self.render_diacs(msg),
            Message::Drawn(msg) => {
                self.drawn_mut().map_or(Task::none(), |drawn| drawn.update(msg)).map(Message::Drawn)
            }
            Message::SetBlur(blur) => {
                self.blur = blur;
                Task::none()
            }
            Message::Blur(msg) => match msg {
                Poll::Pending(BlurStatus::Blurring) => {
                    self.blurring = Poll::Pending(BlurStatus::Blurring);
                    let img = self.img.img.clone();
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
    pub fn view(&'_ self) -> Element<'_, Message> {
        let drawn = self.drawing.as_ready_ok().and_then(Option::as_ref);
        row(Iterator::chain(
            drawn.iter().map(|drawn| drawn.misses_view(self.zoom).map(Message::Drawn)),
            [stack(Iterator::chain(
                once(self.img().view(self.zoom)),
                drawn.iter().map(|drawn| drawn.diac_view(self.zoom).map(Message::Drawn)),
            ))
            .into()],
        ))
        .into()
    }

    fn img(&self) -> &ImgHandle {
        self.blurring.as_ready().and_then(Option::as_ref).unwrap_or(&self.img.img)
    }

    fn render_diacs(&mut self, msg: drawn::PollDraw) -> Task<Message> {
        match msg {
            Poll::Pending(PositStatus::Pending) => {
                self.drawing = Poll::Pending(PositStatus::Pending);
                let img = self.img.img.img();
                Task::stream(channel(1, async move |mut tx| {
                    let progress_callback = {
                        let tx = tx.clone();
                        move |progress| {
                            _ = tx.clone().try_send(Poll::Pending(progress));
                        }
                    };
                    let result = spawn_blocking(move || {
                        drawn::position_diacs(&img, DATAPATH, progress_callback)
                    })
                    .await
                    .expect("blocking task to finish");
                    _ = tx.try_send(Poll::Ready(result.map_err(Arc::new)));
                }))
                .map(Message::Draw)
            }
            Poll::Pending(msg) => {
                self.drawing = Poll::Pending(msg);
                Task::none()
            }
            Poll::Ready(diac_res) => {
                self.drawing = diac_res.map(Drawn::new).map(Some).pipe(Poll::Ready);
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
            self.drawing.as_ready_ok().and_then(Option::as_ref).map(|drawn| {
                drawn
                    .toolbar_view(NamedImg { path: self.img.path.clone(), img: self.img().clone() })
                    .map(Message::Drawn)
            }),
        ))
        .align_y(Vertical::Center)
        .spacing(u32::from(PADDING))
        .into()
    }
}
