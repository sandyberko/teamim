#[path = "diac_style.rs"]
mod diac_style;

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
    teamim::{DiacPosition, PositStatus},
};

pub(crate) type SuperState = Poll<Result<State, String>, Status>;

pub(crate) type SuperMsg = Poll<Message, Poll<Result<Vec<DiacPosition>, String>, Status>>;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Status;

#[derive(Clone, Debug)]
pub(crate) enum Message {
    Draw(drawn::PollDraw),
    Drawn(drawn::Message),
    DiacStyle(diac_style::Message),
}

pub(crate) struct State {
    positions: Vec<DiacPosition>,
    drawing: Poll<Result<Option<drawn::Drawn>, Arc<eyre::Report>>, PositStatus>,

    diac_style: diac_style::State,
}

impl State {
    pub(crate) fn new(positions: Vec<DiacPosition>) -> Self {
        Self { positions, drawing: Poll::Ready(Ok(None)), diac_style: diac_style::State::default() }
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Draw(msg) => {
                // TODO offload?

                let diac_size = self.diac_style.size;
                let diac_margin = self.diac_style.margin;
                let diac_color = self.diac_style.color.get_rgb8();

                let mut renderer = diac_renderer::Renderer::new();
                // TODO simplify, split?
                let diacs =
                    if let Some(rendered) = self.drawing.as_ready_ok().and_then(Option::as_ref) {
                        rendered
                            .diacs
                            .iter()
                            .map(|&RenderedDiac { letter, diac, ref position, .. }| {
                                let img = renderer.render(diac, diac_size, diac_color).into();
                                RenderedDiac::new(letter, diac, position.clone(), img)
                            })
                            .collect()
                    } else {
                        self.positions
                            .iter()
                            .map(|&(letter, diac, ref pos)| {
                                // TODO
                                let position = pos.clone().map(|rect| {
                                    diac_renderer::Renderer::position(
                                        letter,
                                        diac,
                                        rect,
                                        diac_size,
                                        diac_margin,
                                    )
                                });
                                let img = renderer.render(diac, diac_size, diac_color).into();
                                RenderedDiac::new(letter, diac, position, img)
                            })
                            .collect()
                    };
                self.drawing = Poll::Ready(Ok(Some(Drawn::new(diacs))));
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
            drawn.diac_view(img).map(Message::Drawn)
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
                .map(|drawn| drawn.toolbar_view(img_to_save).map(Message::Drawn)),
            [self
                .drawing
                .loading_btn()
                .on_press(Message::Draw(Poll::Pending(PositStatus::Pending)))
                .into()],
        )
        .chain(self.diac_style.toolbar_items().into_iter().map(|item| item.map(Message::DiacStyle)))
    }
}
