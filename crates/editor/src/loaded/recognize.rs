#[path = "diac_style.rs"]
mod diac_style;

#[path = "position.rs"]
mod position;

#[path = "drawn.rs"]
mod drawn;

use {
    crate::{
        NamedImg, Poll, diac_renderer,
        img::ImgHandle,
        loaded::recognize::drawn::{Drawn, RenderedDiac},
    },
    editor::stage,
    iced::{Element, Task},
    std::{iter::chain, sync::Arc},
    teamim::{DiacRect, PositStatus},
};

pub(crate) type SuperState = Poll<Result<State, String>, Status>;

pub(crate) type SuperMsg = Poll<Message, Poll<Result<Vec<DiacRect>, String>, Status>>;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Status;

#[derive(Clone, Debug)]
pub(crate) enum Message {
    Draw(drawn::PollDraw),
    Drawn(drawn::Message),
    DiacStyle(diac_style::Message),
}

pub(crate) struct State {
    rects: Vec<DiacRect>,
    drawing: Poll<Result<Option<drawn::Drawn>, Arc<eyre::Report>>, PositStatus>,

    diac_style: diac_style::State,
}

impl State {
    pub(crate) fn new(positions: Vec<DiacRect>) -> Self {
        Self {
            rects: positions,
            drawing: Poll::Ready(Ok(None)),
            diac_style: diac_style::State::default(),
        }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Draw(msg) => {
                // TODO offload?

                let position_opts = self.diac_style.position_opts;
                let diac_color = self.diac_style.color.get_rgb8();

                // TODO simplify, split?
                self.drawing = Poll::Ready(Ok(Some(Drawn::from_rects(
                    &self.rects,
                    position_opts,
                    diac_color,
                ))));
            }
            Message::Drawn(msg) => {
                let Poll::Ready(Ok(Some(drawn))) = &mut self.drawing else {
                    return Task::none();
                };
                return drawn.update(msg).map(Message::Drawn);
            }
            Message::DiacStyle(msg) => return self.diac_style.update(msg).map(Message::DiacStyle),
        }
        Task::none()
    }

    pub(crate) fn view(&'_ self, img: &ImgHandle) -> Element<'_, Message> {
        if let Some(drawn) = self.drawing.as_ready_ok().and_then(Option::as_ref) {
            drawn.diac_view(img, &self.rects).map(Message::Drawn)
        } else {
            stage([]).handle(img.handle().clone()).into()
        }
    }

    pub fn toolbar_items<'a>(
        &self,
        img_to_save: NamedImg,
    ) -> impl IntoIterator<Item = Element<'a, Message>> {
        chain(
            self.drawing
                .as_ready_ok()
                .and_then(Option::as_ref)
                .map(|drawn| {
                    drawn
                        .toolbar_items(img_to_save, self.diac_style.position_opts)
                        .into_iter()
                        .map(|item| item.map(Message::Drawn))
                })
                .into_iter()
                .flatten(),
            [self
                .drawing
                .loading_btn()
                .on_press(Message::Draw(Poll::Pending(PositStatus::Pending)))
                .into()],
        )
        .chain(self.diac_style.toolbar_items().into_iter().map(|item| item.map(Message::DiacStyle)))
    }
}
