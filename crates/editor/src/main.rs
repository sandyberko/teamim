#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod diac_renderer;
mod loaded;
mod strs;
mod task;

use eyre::{OptionExt as _, WrapErr as _, eyre};
use iced::{
    Element, Length, Settings, Subscription, Task,
    advanced::image::Bytes,
    widget::{
        self, Column,
        image::Handle,
        scrollable::{AbsoluteOffset, Direction, Scrollbar, Viewport},
        text::{Fragment, IntoFragment},
    },
};
use image::{
    ImageBuffer, ImageFormat, ImageReader, Rgb, Rgba, RgbaImage, buffer::ConvertBuffer,
    imageops::fast_blur,
};
use num_traits::AsPrimitive;
use rfd::AsyncFileDialog;
use std::{
    borrow::Cow,
    cell::RefCell,
    ffi::CStr,
    mem,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use editor::{spinner::Spinner, stage};
use teamim::TeamimCtx;

use crate::task::{Poll, Progress, TryPoll};

#[derive(Debug, Clone)]
enum Message {
    Tick(Instant),
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
    img: TryPoll<Option<loaded::LoadedImage>, Progress<SelectProgress>>,
    scroll_offset: AbsoluteOffset,
}

impl Default for App {
    fn default() -> Self {
        Self { img: Poll::Ready(Ok(None)), scroll_offset: AbsoluteOffset::default() }
    }
}

impl App {
    fn ticker_sub(&self) -> Subscription<Message> {
        match &self.img {
            Poll::Pending(_) => iced::time::every(Duration::from_millis(16)).map(Message::Tick),
            Poll::Ready(Ok(Some(loaded))) => loaded.subscription().map(Message::Loaded),
            Poll::Ready(_) => Subscription::none(),
        }
    }
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

        widget::row(children.into_iter().flatten())
            .spacing(u32::from(PADDING))
            .width(Length::Fill)
            .into()
    }

    fn content_view(&self) -> Element<'_, Message> {
        let TryPoll::Ready(ready) = &self.img else {
            return widget::text(strs::LOADING).into();
        };
        let Ok(ready) = ready else {
            // [TODO]
            return widget::text(strs::ERROR).into();
        };
        let Some(loaded) = ready else {
            return widget::text(strs::NO_IMG_SELECTED).into();
        };

        loaded.view(self.scroll_offset).map(Message::Loaded)
    }

    fn view(&self) -> Element<'_, Message> {
        let img = self.content_view();
        let scroll_dir =
            Direction::Both { vertical: Scrollbar::new(), horizontal: Scrollbar::new() };
        Column::with_children([
            self.toolbar(),
            widget::scrollable(img).on_scroll(Message::Scroll).direction(scroll_dir).into(),
        ])
        .spacing(u32::from(PADDING))
        .padding(PADDING)
        .into()
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick(now) => {
                if let TryPoll::Pending(progress) = &mut self.img {
                    progress.spinner.tick(now);
                }
                Task::none()
            }
            Message::SelectImage => {
                if let TryPoll::Pending(_) = self.img {
                    return Task::none();
                }

                self.img = Poll::Pending(Progress::default());
                Task::future(async move {
                    let loaded = select_image().await;
                    Message::ImageLoaded(Arc::new(Mutex::new(loaded)))
                })
            }
            Message::ImageLoaded(res) => {
                let mut res = res.lock().unwrap();
                let res = mem::replace(&mut *res, Err(eyre!("message taken")));
                self.img = Poll::Ready(res);
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

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.ticker_sub(),
            self.img
                .as_ready_ok()
                .and_then(Option::as_ref)
                .map_or(Subscription::none(), |loaded| loaded.subscription().map(Message::Loaded)),
        ])
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
    let image = ImageReader::open(&path)?.decode()?.into_rgba8();
    let width = image.width();
    let height = image.height();
    let pixels = Bytes::from(image.into_raw());
    let img = Img {
        path,
        width,
        height,
        pixels: pixels.clone(),
        handle: Handle::from_rgba(width, height, pixels),
    };
    Ok(loaded::LoadedImage::new(img))
}

#[derive(Debug, Clone)]
struct Img {
    path: Arc<Path>,
    width: u32,
    height: u32,
    pixels: Bytes,
    handle: Handle,
}

impl Img {
    fn view<'a, Message>(&self, zoom: f32) -> Element<'a, Message> {
        widget::Image::new(&self.handle)
            .height(AsPrimitive::<f32>::as_(self.height) * zoom)
            .width(AsPrimitive::<f32>::as_(self.width) * zoom)
            .into()
    }

    fn blur(&self) -> Self {
        // [TODO] reduce clones
        let buf =
            RgbaImage::from_raw(self.width, self.height, self.pixels.to_vec()).expect("valid Img");
        let pixels = Bytes::from_owner(fast_blur(&buf, 17.0).into_raw());
        let handle = Handle::from_rgba(self.width, self.height, pixels.clone());
        Img { pixels, handle, ..self.clone() }
    }

    async fn save(self, progress: impl Fn(SaveStatus)) -> eyre::Result<()> {
        progress(SaveStatus::Rendering);
        // [TODO] render

        progress(SaveStatus::SelectingFile);
        // [TODO]
        let mut dialog = AsyncFileDialog::new().set_title(strs::SAVE).add_filter("image", IMG_EXTS);
        if let Some(dir) = self.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file_name) = self.path.file_name() {
            dialog = dialog.set_file_name(file_name.to_str().ok_or_eyre("שם הקובץ לא תקין")?);
        }
        let file = dialog.save_file().await.ok_or_eyre("שמירה בוטלה")?;

        progress(SaveStatus::Writing);
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, self.pixels)
            .expect("`data` to be big enough for width * height");
        let format = ImageFormat::from_path(file.path())?;
        if format == ImageFormat::Jpeg {
            // [FIXME]: [red] diacs?
            ConvertBuffer::<ImageBuffer<Rgb<u8>, _>>::convert(&img)
                .save_with_format(file.path(), format)
                .wrap_err(strs::SAVE_FAILED)?;
        } else {
            img.save_with_format(file.path(), format).wrap_err(strs::SAVE_FAILED)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SaveStatus {
    Trigger(Img),
    Rendering,
    SelectingFile,
    Writing,
}
impl<Ready> IntoFragment<'static> for &Poll<Ready, Progress<SaveStatus>> {
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
    const GUTTMAN: &[u8] = include_bytes!("../../../assets/fonts/Guttman_Stam.ttf");
    fn view(state: &'_ App) -> Element<'_, Message> {
        App::view(state)
    }
    iced::application(boot_fn, App::update, view)
        .subscription(App::subscription)
        .settings(Settings { fonts: vec![Cow::Borrowed(GUTTMAN)], ..Default::default() })
        .title(strs::TITLE)
        .run()?;
    Ok(())
}
