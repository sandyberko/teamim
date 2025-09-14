#[allow(unused)]
mod repro;

mod strs;

use std::{cell::RefCell, path::Path, sync::Arc};

use cosmic::{
    Action, Application, Core, Element, Task,
    app::{self, Settings},
    iced::{ContentFit, Length, Point},
    iced_core::image::Bytes,
    iced_wgpu::graphics::image::image_rs::{
        ImageBuffer, ImageFormat, ImageReader, Rgb, Rgba, buffer::ConvertBuffer,
    },
    iced_widget::scrollable::{Direction, Scrollbar},
    widget::{Column, Image, Row, Space, button, image::Handle, scrollable, text},
};
use eyre::{OptionExt as _, WrapErr as _};
use rfd::AsyncFileDialog;

use editor::stage::Stage;
use teamim::{DATAPATH, DiacMiss, DrawProgress, PlaceOptions, TeamimCtx, leptonica_ext::PixBox};

#[derive(Debug, Clone)]
pub(crate) enum JobState<Ready, Running = ()> {
    /// the has either not yet started or has already finished.
    Ready(Result<Ready, Arc<eyre::Report>>),
    Running(Running),
}

impl<Ready, Running> JobState<Ready, Running> {
    fn ready_ok(&self) -> Option<&Ready> {
        if let JobState::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
    fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        if let JobState::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
}

#[derive(Debug, Clone)]
enum Message {
    SelectImage,
    ImageLoaded(Result<Option<LoadedImage>, Arc<eyre::Report>>),
    Draw,
    Drawn(Result<Drawn, Arc<eyre::Report>>),
    Save,
    Saved(Result<(), Arc<eyre::Report>>),
}

const PADDING: u16 = 5;
const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];

thread_local! {
    static TEAMIM_CTX: RefCell<Option<TeamimCtx>> = const { RefCell::new(None) };
}

struct App {
    core: Core,
    img: JobState<Option<LoadedImage>>,
}

impl Application for App {
    type Executor = cosmic::executor::multi::Executor;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = "org.teamim.editor";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, app::Task<Message>) {
        let app = App { core, img: JobState::Ready(Ok(None)) };
        (app, Task::none())
    }

    fn update(&mut self, msg: Message) -> app::Task<Message> {
        match msg {
            Message::SelectImage => {
                if let JobState::Running(()) = self.img {
                    return Task::none();
                }

                self.img = JobState::Running(());
                app::Task::perform(select_image(), |res| {
                    Action::App(Message::ImageLoaded(res.map_err(Arc::new)))
                })
            }
            Message::ImageLoaded(res) => {
                self.img = JobState::Ready(res);
                Task::none()
            }
            Message::Draw => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                loaded.drawing = JobState::Running(());
                Task::perform(
                    loaded.clone().draw_teamim(PlaceOptions::default(), |_| { /* [TODO] */ }),
                    |res| Action::App(Message::Drawn(res.map_err(Arc::new))),
                )
            }
            Message::Drawn(drawn) => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };

                loaded.drawing = JobState::Ready(drawn.map(Some));
                Task::none()
            }
            Message::Save => {
                let Some(drawn) = self
                    .img
                    .ready_ok_mut()
                    .and_then(|loaded| loaded.as_mut()?.drawing.ready_ok_mut()?.as_mut())
                else {
                    return Task::none();
                };
                drawn.saving = JobState::Running(());
                Task::perform(drawn.clone().save(), |res| {
                    Action::App(Message::Saved(res.map_err(Arc::new)))
                })
            }
            Message::Saved(saved) => {
                let Some(drawn) = self
                    .img
                    .ready_ok_mut()
                    .and_then(|loaded| loaded.as_mut()?.drawing.ready_ok_mut()?.as_mut())
                else {
                    return Task::none();
                };
                drawn.saving = JobState::Ready(saved);
                Task::none()
            }
        }
    }
    fn view(&'_ self) -> Element<'_, Message> {
        let img = self.img_view();
        // [TODO] this breaks RTL 😭
        // let scroll_dir =
        //     Direction::Both { vertical: Scrollbar::new(), horizontal: Scrollbar::new() };
        Column::with_children([self.toolbar(), scrollable(img).into()])
            .spacing(PADDING)
            .padding(PADDING)
            .into()
    }
}

impl App {
    fn toolbar(&'_ self) -> Element<'_, Message> {
        Row::with_children([
            // select
            button::suggested(strs::SELECT_IMG).on_press(Message::SelectImage).into(),
            // draw
            self.img
                .ready_ok()
                .and_then(Option::as_ref)
                .map_or(/* [HACK] */ Space::with_width(0).into(), |_| {
                    button::text(strs::DRAW_TEAMIM).on_press(Message::Draw).into()
                }),
            // save
            self.img
                .ready_ok()
                .and_then(|loaded| loaded.as_ref()?.drawing.ready_ok()?.as_ref())
                .map_or(/* [HACK] */ Space::with_width(0).into(), |_| {
                    button::text(strs::SAVE).on_press(Message::Save).into()
                }),
        ])
        .spacing(PADDING)
        .width(Length::Fill)
        .into()
    }

    fn img_view(&'_ self) -> Element<'_, Message> {
        let JobState::Ready(ready) = &self.img else {
            return text(strs::LOADING).into();
        };
        let Ok(ready) = ready else {
            // [TODO]
            return text(strs::ERROR).into();
        };
        let Some(loaded) = ready else {
            return text(strs::NO_IMG_SELECTED).into();
        };

        if let Some(drawn) = loaded.drawing.ready_ok().and_then(Option::as_ref) {
            Row::with_children([
                drawn
                    .misses
                    .iter()
                    .map(|miss| {
                        #[expect(clippy::cast_precision_loss)]
                        (
                            text("FOOO" /* miss.missing_text.as_ref() */) /* .size(48.0) */
                                .into(),
                            Point::new(0.0, miss.top as f32),
                        )
                    })
                    .collect::<Stage<Message>>()
                    .into(),
                Image::new(&drawn.img.handle).content_fit(ContentFit::None).into(),
            ])
            .into()
        } else {
            Image::new(&loaded.img.handle).content_fit(ContentFit::None).into()
        }
    }
}

async fn select_image() -> eyre::Result<Option<LoadedImage>> {
    // [TODO]
    let Some(picked_file) = AsyncFileDialog::new()
        .set_title(strs::SELECT_IMG)
        .add_filter("image", IMG_EXTS)
        .pick_file()
        .await
    else {
        return Ok(None);
    };

    let image = ImageReader::open(picked_file.path())?.decode()?.into_rgba8();
    let width = image.width();
    let height = image.height();
    let pixels = Bytes::from(image.into_raw());
    let path = picked_file.path().into();
    let img = Img {
        path,
        width,
        height,
        pixels: pixels.clone(),
        handle: Handle::from_rgba(width, height, pixels),
    };
    Ok(Some(LoadedImage { img, drawing: JobState::Ready(Ok(None)) }))
}

#[derive(Debug, Clone)]
struct Img {
    path: Arc<Path>,
    width: u32,
    height: u32,
    pixels: Bytes,
    handle: Handle,
}

#[derive(Debug, Clone)]
struct LoadedImage {
    img: Img,
    drawing: JobState<Option<Drawn>>,
}

impl LoadedImage {
    async fn draw_teamim(
        self,
        options: PlaceOptions,
        progress_callback: impl Fn(DrawProgress) + Send + 'static,
    ) -> eyre::Result<Drawn> {
        tokio::task::spawn_blocking(move || {
            TEAMIM_CTX.with_borrow_mut(|ctx| -> eyre::Result<_> {
                let ctx = {
                    if ctx.is_none() {
                        *ctx = Some(TeamimCtx::new(DATAPATH)?);
                    }

                    ctx.as_mut().unwrap()
                };

                let mut data = self.img.pixels.to_vec();

                let misses = PixBox::from_rgba8_with(
                    &mut data,
                    self.img.width.try_into()?,
                    self.img.height.try_into()?,
                    |img| ctx.place_teamim_pix(img, options, progress_callback),
                )
                .unwrap_or_else(|err| Err(err.into()))?;

                let pixels = Bytes::from(data);
                let handle = Handle::from_rgba(self.img.width, self.img.height, pixels.clone());
                let img = Img { pixels, handle, ..self.img };
                Ok(Drawn { img, misses, saving: JobState::Ready(Ok(())) })
            })
        })
        .await
        .expect("blocking task to finish")
    }
}

#[derive(Debug, Clone)]
struct Drawn {
    img: Img,
    misses: Vec<DiacMiss>,
    saving: JobState<()>,
}

impl Drawn {
    async fn save(self) -> eyre::Result<()> {
        // [TODO]
        let mut dialog = AsyncFileDialog::new().set_title(strs::SAVE).add_filter("image", IMG_EXTS);
        if let Some(dir) = self.img.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file_name) = self.img.path.file_name() {
            dialog = dialog.set_file_name(file_name.to_str().ok_or_eyre("שם הקובץ לא תקין")?);
        }
        let file = dialog.save_file().await.ok_or_eyre("שמירה בוטלה")?;
        let img =
            ImageBuffer::<Rgba<u8>, _>::from_raw(self.img.width, self.img.height, self.img.pixels)
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

fn main() -> eyre::Result<()> {
    cosmic::app::run::<App>(Settings::default(), ())?;
    Ok(())
}
