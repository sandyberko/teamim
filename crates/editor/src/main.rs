#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod diac_renderer;
mod img;
mod loaded;
mod strs;
mod task;

use {
    crate::{img::ImgHandle, loaded::LoadedImage, task::Poll},
    editor::{GUTTMAN, stage},
    iced::{
        Element, Length, Task,
        widget::{
            column, row, text,
            text::{Fragment, IntoFragment},
        },
    },
    iced_aw::ICED_AW_FONT_BYTES,
    image::ImageReader,
    rfd::AsyncFileDialog,
    std::{borrow::Cow, cell::RefCell, ffi::CStr, path::Path, sync::Arc},
    teamim::TeamimCtx,
};

#[derive(Debug, Clone)]
enum Message {
    SelectImage,
    ImageLoaded(Result<Option<NamedImg>, String>),
    Loaded(loaded::Message),
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
                *ctx = Some(TeamimCtx::new(datapath).inspect_err(|err| tracing::error!("{err}"))?);
            }

            ctx.as_mut().unwrap()
        };
        Ok(f(ctx))
    })
}

const FONT_SIZE: f32 = 72.0;
const MARGIN: f32 = (FONT_SIZE * 0.4).round();

// [TODO]
#[derive(Default)]
struct SelectProgress {}
struct App {
    img: Poll<Result<Option<loaded::LoadedImage>, String>, SelectProgress>,
}

impl Default for App {
    fn default() -> Self {
        Self { img: Poll::Ready(Ok(None)) }
    }
}

impl App {
    fn toolbar(&self) -> Element<'_, Message> {
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
                    let loaded = select_image().await.map_err(|err| err.to_string());
                    Message::ImageLoaded(loaded)
                })
            }
            Message::ImageLoaded(res) => {
                self.img = Poll::Ready(res.map(|img| img.map(LoadedImage::new)));
                Task::none()
            }
            Message::Loaded(msg) => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                loaded.update(msg).map(Message::Loaded)
            }
        }
    }
}

async fn select_image() -> eyre::Result<Option<NamedImg>> {
    // [TODO]
    let Some(picked_file) = AsyncFileDialog::new()
        .set_title(strs::SELECT_IMG)
        .add_filter("image", IMG_EXTS)
        .pick_file()
        .await
    else {
        return Ok(None);
    };

    let loaded = tokio::task::spawn_blocking(move || NamedImg::open(picked_file.path()))
        .await
        .expect("blocking task to finish")?;

    Ok(Some(loaded))
}

#[derive(Debug, Clone)]
struct NamedImg {
    path: Arc<Path>,
    img: ImgHandle,
}

impl NamedImg {
    pub(crate) fn open(path: impl Into<Arc<Path>>) -> eyre::Result<Self> {
        let path = path.into();
        let img = ImageReader::open(&path)?.decode()?.into_rgba8().into();
        Ok(Self { path, img })
    }
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
    let _guard = tracing_init()?;

    let boot_fn = App::default;
    run_app(boot_fn)?;
    Ok(())
}

fn tracing_init() -> eyre::Result<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::{non_blocking, rolling};
    use tracing_error::ErrorLayer;
    use tracing_subscriber::{
        EnvFilter, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    let (non_blocking_appender, guard) = non_blocking(rolling::daily("logs", "teamim-editor"));
    let file_layer = fmt::layer().with_ansi(false).with_writer(non_blocking_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    Registry::default()
        .with(fmt::layer().pretty().with_writer(std::io::stderr))
        .with(file_layer)
        .with(ErrorLayer::default())
        .with(env_filter)
        .init();

    color_eyre::install()?;

    Ok(guard)
}

fn run_app(boot_fn: impl Fn() -> App + 'static) -> eyre::Result<()> {
    fn view(state: &'_ App) -> Element<'_, Message> {
        App::view(state)
    }
    iced::application(boot_fn, App::update, view)
        .font(GUTTMAN)
        .font(ICED_AW_FONT_BYTES)
        .title(strs::TITLE)
        .run()?;
    Ok(())
}
