
use iced::Element;

use crate::loaded::color_picker;

enum Message {}
struct State {
    color: color_picker::State,
    size: f32,
    margin: f32,
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
    fn toolbar_view(&self) -> Element<'_, Message> {
        row([
            // diac color
            self.diac_style.color.view().map(Message::DiacColor),
            // diac size
            number_input(&self.diac_style.size, 1.0..254.0, Message::DiacSize).into(),
            text(strs::SIZE).into(),
            // diac margin
            number_input(&self.diac_style.margin, -245.0..254.0, Message::DiacMargin).into(),
            text(strs::MARGIN).into(),
        ])
    }
}
