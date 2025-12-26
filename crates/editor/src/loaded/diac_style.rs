use iced::{Color, Element, Task, widget::text};
use iced_aw::widget::helpers::number_input;

use crate::{FONT_SIZE, MARGIN, loaded::color_picker, strs};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Color(color_picker::Message),
    Size(f32),
    Margin(f32),
}

pub(crate) struct State {
    pub(crate) color: color_picker::State,
    pub(crate) size: f32,
    pub(crate) margin: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            size: FONT_SIZE,
            margin: MARGIN,
            color: color_picker::State::new(Color::from_rgb8(u8::MAX, 0, 0)),
        }
    }
}

impl State {
    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Color(message) => return self.color.update(message).map(Message::Color),
            Message::Size(msg) => self.size = msg,
            Message::Margin(msg) => self.margin = msg,
        }
        Task::none()
    }

    pub(crate) fn toolbar_items<'a>(&self) -> impl IntoIterator<Item = Element<'a, Message>> {
        [
            // diac color
            self.color.view().map(Message::Color),
            // diac size
            number_input(&self.size, 1.0..254.0, Message::Size).into(),
            text(strs::SIZE).into(),
            // diac margin
            number_input(&self.margin, -245.0..254.0, Message::Margin).into(),
            text(strs::MARGIN).into(),
        ]
    }
}
