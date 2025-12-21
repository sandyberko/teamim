mod color_picker;
mod drawn;

use {
    crate::{
        FONT_SIZE, NamedImg, PADDING, diac_renderer, img::ImgHandle, loaded::drawn::RenderedDiac,
        strs, task::Poll, with_tctx,
    },
    drawn::Drawn,
    iced::{
        Color, Element, Task,
        alignment::Vertical,
        stream::channel,
        widget::{
            row,
            text::{self, IntoFragment},
        },
    },
    iced_aw::widget::helpers::number_input,
    std::{borrow::Cow, sync::Arc},
    tap::prelude::*,
    teamim::{DATAPATH, PositStatus},
    tokio::task::spawn_blocking,
};

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
    BlurSigma(f32),
    Blur(Poll<ImgHandle, BlurStatus>),
    DiacSize(f32),
    DiacColor(color_picker::Message),
}

pub(crate) struct LoadedImage {
    img: NamedImg,

    diac_size: f32,
    diac_color: color_picker::State,
    drawing: Poll<Result<Option<Drawn>, Arc<eyre::Report>>, PositStatus>,

    blur_sigma: f32,
    blurring: Poll<Option<ImgHandle>, BlurStatus>,
}

impl LoadedImage {
    pub(crate) fn new(img: NamedImg) -> Self {
        Self {
            img,
            blur_sigma: 17.0,
            drawing: Poll::Ready(Ok(None)),
            blurring: Poll::Ready(None),

            diac_size: FONT_SIZE,
            diac_color: color_picker::State::new(Color::from_rgb8(u8::MAX, 0, 0)),
        }
    }

    fn drawn_mut(&mut self) -> Option<&mut Drawn> {
        if let Poll::Ready(Ok(Some(drawn))) = &mut self.drawing { Some(drawn) } else { None }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Draw(msg) => return self.render_diacs(msg),
            Message::Drawn(msg) => {
                return self
                    .drawn_mut()
                    .map_or(Task::none(), |drawn| drawn.update(msg))
                    .map(Message::Drawn);
            }
            Message::BlurSigma(msg) => self.blur_sigma = msg,
            Message::Blur(msg) => match msg {
                Poll::Pending(BlurStatus::Blurring) => {
                    self.blurring = Poll::Pending(BlurStatus::Blurring);
                    let img = self.img.img.clone();
                    let simga = self.blur_sigma;

                    return Task::future(async move {
                        spawn_blocking(move || {
                            let blurred = img.blur(simga);
                            Message::Blur(Poll::Ready(blurred))
                        })
                        .await
                        .expect("blocking task to finish")
                    });
                }
                Poll::Ready(blurred) => self.blurring = Poll::Ready(Some(blurred.clone())),
            },
            Message::DiacSize(msg) => self.diac_size = msg,
            Message::DiacColor(msg) => return self.diac_color.update(msg).map(Message::DiacColor),
        }
        Task::none()
    }
    pub fn view(&'_ self) -> Element<'_, Message> {
        self.drawing.as_ready_ok().and_then(Option::as_ref).map_or_else(
            || self.img.img.view(),
            |drawn| drawn.diac_view(self.img()).map(Message::Drawn),
        )
    }

    fn img(&self) -> &ImgHandle {
        self.blurring.as_ready().and_then(Option::as_ref).unwrap_or(&self.img.img)
    }

    #[tracing::instrument(skip(self))]
    fn render_diacs(&mut self, msg: drawn::PollDraw) -> Task<Message> {
        match msg {
            Poll::Pending(PositStatus::Pending) => {
                self.drawing = Poll::Pending(PositStatus::Pending);
                let img = self.img.img.img();
                let diac_size = self.diac_size;
                let diac_color = self.diac_color.get_rgb8();
                Task::stream(channel(1, async move |mut tx| {
                    let progress_callback = {
                        let tx = tx.clone();
                        move |progress| {
                            _ = tx.clone().try_send(Poll::Pending(progress));
                        }
                    };
                    let result = spawn_blocking({
                        let tx = tx.clone();
                        move || {
                            let positions =
                                with_tctx(DATAPATH, |ctx| ctx.positions(&img, progress_callback))??;

                            _ = tx.clone().try_send(Poll::Pending(PositStatus::Rendering));
                            // TODO persist
                            let mut renderer = diac_renderer::Renderer::new();
                            Ok(positions
                                .into_iter()
                                .map(|(letter, diac, pos)| {
                                    let map = pos.map(|rect| {
                                        renderer.position(letter, diac, rect, diac_size)
                                    });
                                    let img = renderer.render(diac, diac_size, diac_color).into();
                                    RenderedDiac::new(letter, diac, map, img)
                                })
                                .collect())
                        }
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

    pub(crate) fn toolbar_view(&self) -> Element<'_, Message> {
        row([
            // blur
            number_input(&self.blur_sigma, 0.0..25.0, Message::BlurSigma).into(),
            self.blurring
                .loading_btn()
                .on_press(Message::Blur(Poll::Pending(BlurStatus::Blurring)))
                .into(),
            // diac color
            self.diac_color.view().map(Message::DiacColor),
            // diac size
            number_input(&self.diac_size, 1.0..254.0, Message::DiacSize).into(),
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
