use iced::{Element, Task};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    SetText(String),
}

#[derive(Debug)]
pub(crate) struct State {
    text: String,
    number: f32,
}

impl State {
    pub(crate) fn new(number: f32) -> Self {
        Self { number, text: number.to_string() }
    }

    pub(crate) fn get(&self) -> f32 {
        self.number
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        iced::widget::text_input("", &self.text).on_input(Message::SetText).into()
    }

    pub(crate) fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SetText(text) => {
                if let Ok(number) = text.parse() {
                    self.text = text;
                    self.number = number;
                }
            }
        }
        Task::none()
    }
}
