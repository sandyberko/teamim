mod color_picker;
pub(crate) mod recognize;

use {
    crate::{
        NamedImg,
        img::ImgHandle,
        strs,
        task::{Poll, loading_btn},
    },
    editor::stage,
    iced::{Element, Task, futures::FutureExt},
    iced_aw::widget::helpers::number_input,
    std::iter::chain,
    teamim::{DATAPATH, TeamimCtx},
    tokio::task::spawn_blocking,
};

#[derive(Debug, Clone, Copy, Default)]
pub enum BlurStatus {
    #[default]
    Blurring,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    BlurSigma(f32),
    Blur(Poll<ImgHandle, BlurStatus>),

    Recognize(recognize::SuperMsg),
}

pub(crate) struct LoadedImage {
    img: NamedImg,

    recognizing: Option<recognize::SuperState>,

    blur_sigma: f32,
    blurring: Poll<Option<ImgHandle>, BlurStatus>,
}

impl LoadedImage {
    pub(crate) fn new(img: NamedImg) -> Self {
        Self { img, blur_sigma: 17.0, recognizing: None, blurring: Poll::Ready(None) }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
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
            Message::Recognize(msg) => match msg {
                Poll::Pending(msg) => match msg {
                    Poll::Pending(msg) => {
                        self.recognizing = Some(Poll::Pending(msg));
                        return Task::perform(
                            {
                                let img = self.img.img.img();
                                spawn_blocking(move || {
                                    // TODO reuse
                                    let mut ctx =
                                        TeamimCtx::new(DATAPATH).map_err(|err| "ERROR")?;
                                    let positions =
                                        ctx.positions(&img, |_| {}).map_err(|err| "ERROR")?;
                                    Ok(positions)
                                })
                                .map(|res| res.expect("blocking task to complete"))
                            },
                            |ready| Message::Recognize(Poll::Pending(Poll::Ready(ready))),
                        );
                    }
                    Poll::Ready(msg) => {
                        self.recognizing = Some(Poll::Ready(msg.map(recognize::State::new)));
                    }
                },
                Poll::Ready(msg) => {
                    let Some(Poll::Ready(Ok(recognized))) = &mut self.recognizing else {
                        return Task::none();
                    };
                    return recognized.update(msg).map(|msg| Message::Recognize(Poll::Ready(msg)));
                }
            },
        }
        Task::none()
    }
    pub fn view(&'_ self) -> Element<'_, Message> {
        if let Some(recognizing) = &self.recognizing
            && let Some(recognized) = recognizing.as_ready_ok()
        {
            recognized.view(self.img()).map(|msg| Message::Recognize(Poll::Ready(msg)))
        } else {
            stage([]).handle(self.img().handle().clone()).into()
        }
    }

    fn img(&self) -> &ImgHandle {
        self.blurring.as_ready().and_then(Option::as_ref).unwrap_or(&self.img.img)
    }

    pub(crate) fn toolbar_items(&self) -> impl IntoIterator<Item = Element<'_, Message>> {
        chain(
            self.recognizing
                .as_ref()
                .and_then(|recognizing| recognizing.as_ready_ok())
                .into_iter()
                .flat_map(|recognized| {
                    recognized
                        .toolbar_items(NamedImg {
                            path: self.img.path.clone(),
                            img: self.img().clone(),
                        })
                        .into_iter()
                        .map(|item| item.map(|msg| Message::Recognize(Poll::Ready(msg))))
                }),
            [
                // recognize
                loading_btn(&self.recognizing, strs::RECOGNIZE)
                    .on_press(Message::Recognize(Poll::Pending(Poll::Pending(recognize::Status))))
                    .into(),
                // blur
                number_input(&self.blur_sigma, 0.0..25.0, Message::BlurSigma).into(),
                self.blurring
                    .loading_btn()
                    .on_press(Message::Blur(Poll::Pending(BlurStatus::Blurring)))
                    .into(),
            ],
        )
    }
}
