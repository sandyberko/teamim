mod strs;

use std::sync::Arc;

use cosmic::{
    Action, Application, Core, Element,
    app::{Settings, Task},
    iced::Length,
    widget::{Column, Image, Row, Space, button, image::Handle, text},
};

#[derive(Debug)]
pub(crate) enum JobState<Ready, Running = ()> {
    /// the has either not yet started or has already finished.
    Ready(Result<Ready, Arc<eyre::Report>>),
    Running(Running),
}

impl<Ready, Running> JobState<Ready, Running> {
    fn ready_ok(&self) -> Option<&Ready> {
        if let JobState::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SelectImage,
    ImageLoaded(Result<Option<Handle>, Arc<eyre::Report>>),
    Draw,
}

const PADDING: u16 = 5;

struct App {
    core: Core,
    img: JobState<Option<Handle>>,
}

impl Application for App {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "org.teamim.editor";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        let app = App { core, img: JobState::Ready(Ok(None)) };
        (app, Task::none())
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SelectImage => {
                if let JobState::Running(()) = self.img {
                    return Task::none();
                }

                self.img = JobState::Running(());
                Task::perform(select_image(), |res| {
                    Action::App(Message::ImageLoaded(res.map_err(Arc::new)))
                })
            }
            Message::ImageLoaded(image) => {
                self.img = JobState::Ready(image);
                Task::none()
            }
            Message::Draw => {
                // [TODO]
                Task::none()
            }
        }
    }
    fn view(&'_ self) -> Element<'_, Message> {
        let toolbar = Row::with_children([
            button::suggested(strs::SELECT_IMG).on_press(Message::SelectImage).into(),
            self.img
                .ready_ok()
                .and_then(Option::as_ref)
                .map_or(/* [HACK] */ Space::with_width(0).into(), |_| {
                    button::text(strs::DRAW_TEAMIM).on_press(Message::Draw).into()
                }),
        ])
        .spacing(PADDING)
        .width(Length::Fill)
        .into();
        let img: Element<Message> = match &self.img {
            JobState::Ready(Ok(Some(img))) => Image::new(img).into(),
            JobState::Ready(Ok(None)) => text(strs::NO_IMG).into(),
            // [TODO]
            JobState::Ready(Err(_)) => text("ERROR").into(),
            // [TODO]
            JobState::Running(()) => text("Loading...").into(),
        };
        Column::with_children([toolbar, img]).spacing(PADDING).padding(PADDING).into()
    }
}

#[allow(unused)]
mod repro {
    use cosmic::{
        Element, Renderer, Theme,
        iced::{Length, Rectangle, Size},
        iced_core::{
            Layout,
            layout::{Limits, Node},
            mouse::Cursor,
            renderer::Style,
            widget::Tree,
        },
        widget::Widget,
    };

    pub struct Btn<Message>(pub Element<'static, Message>);
    impl<Message> Widget<Message, Theme, Renderer> for Btn<Message> {
        fn children(&self) -> Vec<Tree> {
            vec![Tree::new(&self.0)]
        }
        fn size(&self) -> Size<Length> {
            Size::new(Length::Shrink, Length::Shrink)
        }

        fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
            self.0.as_widget().layout(&mut tree.children[0], renderer, limits)
        }

        fn draw(
            &self,
            tree: &Tree,
            renderer: &mut Renderer,
            theme: &Theme,
            style: &Style,
            layout: Layout<'_>,
            cursor: Cursor,
            viewport: &Rectangle,
        ) {
            self.0.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                &viewport.intersection(&layout.bounds()).unwrap_or_default(),
            );
        }
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

fn main() -> eyre::Result<()> {
    cosmic::app::run::<App>(Settings::default(), ())?;
    Ok(())
}
