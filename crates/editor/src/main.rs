mod loading_btn;
mod strs;

use std::sync::Arc;

use iced::{
    Element, Task,
    widget::{Image, button, column, image::Handle, text},
};

use crate::loading_btn::JobState;

#[derive(Debug, Clone)]
enum Message {
    SelectImage,
    ImageLoaded(Result<Option<Handle>, Arc<eyre::Report>>),
}

struct App {
    img: JobState<Option<Handle>>,
}

impl Default for App {
    fn default() -> Self {
        Self { img: JobState::Ready(Ok(None)) }
    }
}

impl App {
    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SelectImage => {
                if let JobState::Running(()) = self.img {
                    return Task::none();
                }

                self.img = JobState::Running(());
                Task::perform(select_image(), |res| Message::ImageLoaded(res.map_err(Arc::new)))
            }
            Message::ImageLoaded(image) => {
                self.img = JobState::Ready(image);
                Task::none()
            }
        }
    }
    fn view(&'_ self) -> Element<'_, Message> {
        let select_btn: Element<Message> =
            button(strs::SELECT_IMG).on_press(Message::SelectImage).into();
        let img: Element<Message> = match &self.img {
            JobState::Ready(Ok(Some(img))) => Image::new(img).into(),
            JobState::Ready(Ok(None)) => text(strs::NO_IMG).into(),
            // TODO
            JobState::Ready(Err(_)) => text("ERROR").into(),
            // TODO
            JobState::Running(()) => text("Loading...").into(),
        };
        column![select_btn, img].into()
    }
}

async fn select_image() -> eyre::Result<Option<Handle>> {
    let Some(picked_file) =
        rfd::AsyncFileDialog::new().set_title("Open a text file...").pick_file().await
    else {
        return Ok(None);
    };

    Ok(Some(Handle::from_path(picked_file.path())))
}

fn main() -> iced::Result {
    iced::run("A cool counter", App::update, App::view)
}
