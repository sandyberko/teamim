use {
    crate::{FONT_SIZE, PADDING, strs},
    iced::{
        Alignment, Color, Element, Length, Task,
        widget::{Button, Row, Text, container, row, space, text},
    },
    iced_aw::helpers::color_picker,
};

#[derive(Clone, Debug)]
pub(crate) enum Message {
    Choose,
    Submit(Color),
    Cancel,
}

#[derive(Debug)]
pub(crate) struct State {
    color: Color,
    show_picker: bool,
}

impl State {
    pub(crate) fn new(color: Color) -> Self {
        Self { color, show_picker: false }
    }

    pub(crate) fn get_rgb8(&self) -> [u8; 3] {
        let [r, g, b, _] = self.color.into_rgba8();
        [r, g, b]
    }

    #[expect(clippy::needless_pass_by_value)]
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Choose => {
                self.show_picker = true;
            }
            Message::Submit(color) => {
                self.color = color;
                self.show_picker = false;
            }
            Message::Cancel => {
                self.show_picker = false;
            }
        }
        Task::none()
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        color_picker(
            self.show_picker,
            self.color,
            Button::new(
                row([
                    text(strs::COLOR).into(),
                    container(space())
                        .width(32)
                        .height(16)
                        .style(|_| container::background(self.color))
                        .into(),
                ])
                .spacing(u32::from(PADDING)),
            )
            .on_press(Message::Choose),
            Message::Cancel,
            Message::Submit,
        )
        .into()
    }
}
