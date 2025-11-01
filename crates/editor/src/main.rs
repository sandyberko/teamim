#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod diac_renderer;
mod img;
mod loaded;
mod strs;
mod task;

use eyre::eyre;
use iced::{
    Element, Length, Task,
    widget::{
        Column, column, row, scrollable,
        scrollable::{AbsoluteOffset, Direction, Scrollbar, Viewport},
        text,
        text::{Fragment, IntoFragment},
    },
};
use image::ImageReader;
use rfd::AsyncFileDialog;
use std::{
    borrow::Cow,
    cell::RefCell,
    ffi::CStr,
    mem,
    path::Path,
    sync::{Arc, Mutex},
};

use editor::stage;
use teamim::TeamimCtx;

use crate::{img::ImgHandle, task::Poll};

#[derive(Debug, Clone)]
enum Message {
    SelectImage,
    ImageLoaded(Arc<Mutex<eyre::Result<Option<loaded::LoadedImage>>>>),
    Loaded(loaded::Message),
    Scroll(Viewport),
}

impl From<loaded::Message> for Message {
    fn from(value: loaded::Message) -> Self {
        Message::Loaded(value)
    }
}

const PADDING: u16 = 5;
const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];

fn with_tctx<T, F>(datapath: &CStr, f: F) -> eyre::Result<T>
where
    F: FnOnce(&mut TeamimCtx) -> T,
{
    thread_local! {
        static TEAMIM_CTX: RefCell<Option<TeamimCtx>> = const { RefCell::new(None) };
    }
    TEAMIM_CTX.with_borrow_mut(|ctx| {
        let ctx = {
            if ctx.is_none() {
                *ctx = Some(TeamimCtx::new(datapath)?);
            }

            ctx.as_mut().unwrap()
        };
        Ok(f(ctx))
    })
}

const FONT_SIZE: f32 = 72.0;

// [TODO]
#[derive(Default)]
struct SelectProgress {}
struct App {
    img: Poll<Result<Option<loaded::LoadedImage>, String>, SelectProgress>,
    scroll_offset: AbsoluteOffset,
}

impl Default for App {
    fn default() -> Self {
        Self { img: Poll::Ready(Ok(None)), scroll_offset: AbsoluteOffset::default() }
    }
}

impl App {
    fn toolbar<'a>(&'_ self) -> Element<'a, Message> {
        let children = [
            // select
            Some(self.img.loading_btn().on_press(Message::SelectImage).into()),
            // draw
            self.img
                .as_ready_ok()
                .and_then(Option::as_ref)
                .map(|loaded| loaded.toolbar_view().map(Message::Loaded)),
        ];

        row(children.into_iter().flatten()).spacing(u32::from(PADDING)).width(Length::Fill).into()
    }

    fn content_view(&self) -> Element<'_, Message> {
        let Poll::Ready(ready) = &self.img else {
            return text(strs::LOADING).into();
        };
        let ready = match ready {
            Ok(ready) => ready,
            Err(msg) => {
                return column([text(strs::ERROR).into(), text(msg).into()]).into();
            }
        };
        let Some(loaded) = ready else {
            return text(strs::NO_IMG_SELECTED).into();
        };

        loaded.view().map(Message::Loaded)
    }

    fn view(&self) -> Element<'_, Message> {
        column([self.toolbar(), self.content_view()])
            .spacing(u32::from(PADDING))
            .padding(PADDING)
            .into()
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::SelectImage => {
                if let Poll::Pending(_) = self.img {
                    return Task::none();
                }

                self.img = Poll::Pending(SelectProgress::default());
                Task::future(async move {
                    let loaded = select_image().await;
                    Message::ImageLoaded(Arc::new(Mutex::new(loaded)))
                })
            }
            Message::ImageLoaded(res) => {
                let mut res = res.lock().unwrap();
                let res = mem::replace(&mut *res, Err(eyre!("message taken")));
                self.img = Poll::Ready(res.map_err(|err| err.to_string()));
                Task::none()
            }
            Message::Loaded(msg) => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                loaded.update(msg).map(Message::Loaded)
            }
            Message::Scroll(viewport) => {
                self.scroll_offset = viewport.absolute_offset();
                Task::none()
            }
        }
    }
}

async fn select_image() -> eyre::Result<Option<loaded::LoadedImage>> {
    // [TODO]
    let Some(picked_file) = AsyncFileDialog::new()
        .set_title(strs::SELECT_IMG)
        .add_filter("image", IMG_EXTS)
        .pick_file()
        .await
    else {
        return Ok(None);
    };

    let loaded = tokio::task::spawn_blocking(move || load_image(picked_file.path()))
        .await
        .expect("blocking task to finish")?;

    Ok(Some(loaded))
}

fn load_image(path: impl AsRef<Path>) -> eyre::Result<loaded::LoadedImage> {
    let path = path.as_ref().into();
    let img = ImageReader::open(&path)?.decode()?.into_rgba8().into();
    Ok(loaded::LoadedImage::new(NamedImg { path, img }))
}

#[derive(Debug, Clone)]
struct NamedImg {
    // [TODO] private
    pub(crate) path: Arc<Path>,
    img: ImgHandle,
}

#[derive(Debug, Clone)]
pub(crate) enum SaveStatus {
    Trigger(NamedImg),
    Rendering,
    SelectingFile,
    Writing,
}
impl<Ready> IntoFragment<'static> for &Poll<Ready, SaveStatus> {
    fn into_fragment(self) -> Fragment<'static> {
        Cow::Borrowed(strs::SAVE)
    }
}

fn main() -> eyre::Result<()> {
    let boot_fn = App::default;
    run_app(boot_fn)?;
    Ok(())
}

fn run_app(boot_fn: impl Fn() -> App + 'static) -> eyre::Result<()> {
    fn view(state: &'_ App) -> Element<'_, Message> {
        App::view(state)
    }
    iced::application(boot_fn, App::update, view).title(strs::TITLE).run()?;
    Ok(())
}
