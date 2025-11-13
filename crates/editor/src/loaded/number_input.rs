use {
    crate::{ICON_FONT, PADDING},
    iced::{
        Element, Length, Task, color,
        widget::{button, column, row, text, text::LineHeight, text_input, text_input::Side},
    },
    tap::Conv,
};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    SetText(String),
    Increase,
    Decrease,
}

#[derive(Debug)]
pub(crate) struct State<T> {
    text: String,
    number: T,
}

impl<T> State<T>
where
    T: Copy + ToString,
{
    pub(crate) fn new(number: T) -> Self {
        Self { number, text: number.to_string() }
    }

    pub(crate) fn get(&self) -> T {
        self.number
    }
}

impl State<f32> {
    pub(crate) fn view(&self) -> Element<'_, Message> {
        row([
            text_input("", &self.text)
                .width(Length::Fixed(60.0))
                .icon(text_input::Icon {
                    font: ICON_FONT,
                    code_point: 'T',
                    size: None,
                    spacing: PADDING.into(),
                    side: Side::Right,
                })
                .on_input(Message::SetText)
                .into(),
            column([("^", Message::Increase), ("v", Message::Decrease)].map(|(icon, msg)| {
                button(text(icon).line_height(LineHeight::Relative(1.0)).font(ICON_FONT))
                    .class(Box::new(button::background) as button::StyleFn<_>)
                    .padding(0)
                    .on_press(msg)
                    .into()
            }))
            .conv::<Element<_>>()
            .explain(color!(0xff, 0, 0)),
        ])
        .into()
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SetText(text) => {
                if let Ok(number) = text.parse() {
                    self.text = text;
                    self.number = number;
                }
            }
            Message::Increase => {
                self.number += 1.0;
                self.text = self.number.to_string();
            }
            Message::Decrease => {
                self.number -= 1.0;
                self.text = self.number.to_string();
            }
        }
        Task::none()
    }
}
