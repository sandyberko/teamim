// TODO replace boxed with OneOf

// On Windows platform, don't show a console when opening the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod box_view;
mod job;
mod view_ext;

use std::{cell::RefCell, path::Path, sync::Arc};

use eyre::{OptionExt, WrapErr};
use image::{ImageBuffer, ImageReader, Rgb, Rgba, buffer::ConvertBuffer};
use rfd::FileDialog;
use teamim::{DiacMiss, PlaceOptions, TeamimCtx, leptonica_ext::PixBox};
use xilem::{
    Blob, Color, EventLoop, EventLoopBuilder, Image, ImageFormat, TextAlign, WidgetView,
    WindowOptions, Xilem,
    core::{MessageProxy, MessageResult, fork},
    masonry::properties::types::{AsUnit, Length},
    style::{Background, Padding, Style},
    tokio::sync::mpsc::UnboundedSender,
    view::{
        CrossAxisAlignment, MainAxisAlignment, ObjectFit, flex, flex_row, image, label, portal,
        prose, sized_box, task_raw, worker, zstack,
    },
};

use crate::{
    box_view::tbox,
    job::{JobState, job_btn},
    view_ext::ViewExt,
};

const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];
const FONT_SIZE: Length = Length::const_px(16.);
const PADDING: Padding = Padding::all(FONT_SIZE.get() * 0.2);

thread_local! {
    static TEAMIM_CTX: RefCell<Option<TeamimCtx>> = const { RefCell::new(None) };
}

#[derive(Debug)]
struct Modified {
    saving: JobState<()>,
    image: Image,
    misses: Vec<DiacMiss>,
}
impl Modified {
    fn new(image: Image, misses: Vec<DiacMiss>) -> Self {
        Self { saving: JobState::Ready(Ok(())), image, misses }
    }
}

#[derive(Debug)]
enum DrawIdle {
    Unmodified,
    Modified(Modified),
}

impl DrawIdle {
    fn modified_mut(&mut self) -> Option<&mut Modified> {
        match self {
            Self::Modified(modified) => Some(modified),
            Self::Unmodified => None,
        }
    }
}

type DrawingJob = JobState<DrawIdle>;

#[derive(Debug)]
enum Msg {
    Select(JobState<Image>),
    Draw(JobState<DrawIdle>),
    Save(JobState<()>),
    Other,
}

#[derive(Debug)]
struct LoadedImage {
    path: Arc<Path>,
    img: Image,
    drawing: DrawingJob,
}

impl LoadedImage {
    fn new(img: Image, path: Arc<Path>) -> Self {
        Self { img, path, drawing: JobState::Ready(Ok(DrawIdle::Unmodified)) }
    }
}

fn try_save((img, path): &(Image, Arc<Path>)) -> eyre::Result<()> {
    let mut dialog = FileDialog::new().set_title("שמור תמונה").add_filter("תמונה", IMG_EXTS);
    if let Some(dir) = path.parent() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(file_name) = path.file_name() {
        dialog = dialog.set_file_name(file_name.to_str().ok_or_eyre("שם הקובץ  לא תקין")?);
    }
    let path = dialog.save_file().ok_or_eyre("שמירה בוטלה")?;
    let img = ImageBuffer::<Rgba<u8>, _>::from_raw(img.width, img.height, img.data.data())
        .expect("`data` to be big enough for width * height");
    let format = image::ImageFormat::from_path(&path)?;
    let err_msg = "שמירה נכשלה";
    if format == image::ImageFormat::Jpeg {
        // FIXME: red diacs?
        ConvertBuffer::<ImageBuffer<Rgb<u8>, _>>::convert(&img)
            .save_with_format(&path, format)
            .wrap_err(err_msg)?;
    } else {
        img.save_with_format(&path, format).wrap_err(err_msg)?;
    }
    Ok(())
}

fn draw_teamim(img: &Image) -> eyre::Result<Modified> {
    TEAMIM_CTX.with_borrow_mut(|ctx| {
        if ctx.is_none() {
            *ctx = Some(TeamimCtx::new()?);
        }
        let ctx = ctx.as_mut().unwrap();

        let options = PlaceOptions::default();
        let mut data = img.data.data().to_vec();
        let misses = PixBox::from_rgba8_with(
            &mut data,
            img.width.try_into().unwrap(),
            img.height.try_into().unwrap(),
            |img| ctx.place_teamim_pix(img, options),
        )
        .unwrap_or_else(|err| Err(err.into()))?;
        let image = Image::new(Blob::new(Arc::new(data)), img.format, img.width, img.height);
        Ok(Modified::new(image, misses))
    })
}

const GRAY: Color = Color::from_rgb8(128, 128, 128);
const RED: Color = Color::from_rgb8(255, 0, 0);

fn image_read(path: impl AsRef<Path>) -> eyre::Result<Image> {
    let image = ImageReader::open(path)?.decode()?.into_rgba8();
    let width = image.width();
    let height = image.height();
    let data = image.into_vec();
    Ok(Image::new(Blob::new(Arc::new(data)), ImageFormat::Rgba8, width, height))
}

enum ChanMsg {
    Draw(Image),
    Save(Image, Arc<Path>),
}

struct TaskList {
    loading: JobState<Option<LoadedImage>, Arc<Path>>,
    sender: Option<UnboundedSender<ChanMsg>>,
}

impl TaskList {
    fn handle_img_select(&mut self) {
        self.loading = FileDialog::new()
            .set_title("בחר תמונה")
            .add_filter("תמונה", IMG_EXTS)
            .pick_file()
            .map_or(JobState::Ready(Ok(None)), |path| JobState::Running(path.into()));
    }

    fn view_pending(path: &Arc<Path>) -> impl WidgetView<Self> + use<> {
        let load_task = task_raw(
            {
                let path = path.clone();
                move |proxy| {
                    // TODO double clone?
                    let path = path.clone();
                    async move {
                        let img = image_read(&path).inspect_err(|err| {
                            tracing::warn!("Loading image from {} failed: {err:?}", path.display());
                        });
                        proxy.message((img, path)).ok();
                    }
                }
            },
            move |state: &mut Self, (image, path)| {
                state.loading = JobState::Ready(image.map(|img| Some(LoadedImage::new(img, path))));
            },
        );

        fork(
            flex(prose(path.to_string_lossy()).text_alignment(TextAlign::Center).text_color(GRAY)),
            load_task,
        )
    }

    fn update(&mut self, msg: MessageResult<Msg>) -> MessageResult<()> {
        let MessageResult::Action(action) = msg else { return msg.map(|_| ()) };
        match action {
            Msg::Select(job_state) => {
                if let JobState::Running(()) = job_state {
                    self.handle_img_select();
                }
                MessageResult::Action(())
            }
            Msg::Draw(job_state) => {
                let Some(loaded) = self.loading.ready_ok_mut().and_then(Option::as_mut) else {
                    return MessageResult::Stale;
                };
                match &job_state {
                    JobState::Running(()) => {
                        let Some(sender) = self.sender.as_ref() else {
                            return MessageResult::Stale;
                        };
                        sender.send(ChanMsg::Draw(loaded.img.clone())).ok();
                    }
                    JobState::Ready(Ok(DrawIdle::Modified(modified))) => {
                        loaded.img = modified.image.clone();
                    }
                    JobState::Ready(_) => (),
                }
                loaded.drawing = job_state;
                MessageResult::Action(())
            }
            Msg::Save(job_state) => {
                let Some(loaded) = self.loading.ready_ok_mut().and_then(Option::as_mut) else {
                    return MessageResult::Stale;
                };

                let Some(modified) = loaded.drawing.ready_ok_mut().and_then(DrawIdle::modified_mut)
                else {
                    return MessageResult::Stale;
                };

                if let JobState::Running(()) = job_state {
                    let Some(sender) = self.sender.as_ref() else {
                        return MessageResult::Stale;
                    };
                    sender
                        .send(ChanMsg::Save(modified.image.clone(), Arc::clone(&loaded.path)))
                        .ok();
                }

                modified.saving = job_state;
                MessageResult::Action(())
            }
            Msg::Other => MessageResult::Action(()),
        }
    }

    fn view(&mut self) -> impl WidgetView<Self> + use<> {
        fork(
            flex((
                // toolbar
                flex_row((
                    self.loading.ready_ok_mut().and_then(Option::as_mut).map(|loaded| {
                        (
                            loaded.drawing.ready_ok_mut().and_then(DrawIdle::modified_mut).map(
                                |modified| {
                                    // save
                                    job_btn(&modified.saving, "שמור...", "שמור", |_| {
                                        Msg::Save(JobState::Running(()))
                                    })
                                },
                            ),
                            // draw
                            job_btn(
                                &loaded.drawing,
                                "מצייר...",
                                "צייר טעמים",
                                |_| Msg::Draw(JobState::Running(())),
                            ),
                        )
                    }),
                    // select
                    job_btn(&self.loading, "טוען...", "בחר תמונה", |_| {
                        Msg::Select(JobState::Running(()))
                    }),
                ))
                .main_axis_alignment(MainAxisAlignment::End)
                .padding(PADDING)
                .boxed(),
                match &mut self.loading {
                    JobState::Running(path) => Self::view_pending(path).map_message(nop).boxed(),
                    JobState::Ready(Ok(Some(loaded))) => {
                        flex((
                            loaded.drawing.ready_ok_mut().and_then(DrawIdle::modified_mut).map(
                                |modified| {
                                    sized_box(
                                        flex_row(
                                            modified
                                                .misses
                                                .iter()
                                                .map(|miss| prose(miss.char_idx.to_string()))
                                                .collect::<Vec<_>>(),
                                        )
                                        .main_axis_alignment(MainAxisAlignment::Start)
                                        .padding(PADDING)
                                        .background(Background::Color(Color::from_rgba8(
                                            255, 200, 200, 200,
                                        )))
                                        .border(Color::from_rgb8(255, 0, 0), 2.)
                                        .corner_radius(8.),
                                    )
                                    .width(300.px())
                                    .height(200.px())
                                }
                            ),
                            // TODO: zoom
                            portal(image(&loaded.img).fit(ObjectFit::FitWidth)),
                        ))
                        .boxed()
                    }
                    JobState::Ready(Ok(None)) => label("לא נבחרה תמונה")
                        .text_alignment(TextAlign::Center)
                        .color(GRAY)
                        .map_message(nop)
                        .boxed(),
                    JobState::Ready(Err(_)) => prose("🖼️").map_message(nop).boxed(),
                },
            ))
            .cross_axis_alignment(CrossAxisAlignment::Fill),
            worker(
                move |proxy: MessageProxy<Msg>, mut recv| {
                    async move {
                        while let Some(msg) = recv.recv().await {
                            // debug!("start message received");
                            match msg {
                                ChanMsg::Draw(image) => {
                                    let result = draw_teamim(&image).map(DrawIdle::Modified);
                                    proxy.message(Msg::Draw(JobState::Ready(result))).ok();
                                }
                                ChanMsg::Save(image, path) => {
                                    let result = try_save(&(image, path));
                                    proxy.message(Msg::Save(JobState::Ready(result))).ok();
                                }
                            }
                        }
                    }
                },
                |app: &mut Self, sender: UnboundedSender<ChanMsg>| {
                    app.sender = Some(sender);
                },
                |_, resp: Msg| resp,
            ),
        )
        .map_message(Self::update)
    }
}

fn nop(_: &mut TaskList, msg: MessageResult<()>) -> MessageResult<Msg> {
    msg.map(|()| Msg::Other)
}

fn tboxes_example<State>() -> impl WidgetView<State> + use<State>
where
    State: Send + Sync + 'static,
{
    sized_box(zstack((tbox((0., 0.), (200., 50.)), tbox((0., 70.), (200., 50.)))))
        .width(200.px())
        .height(200.px())
}

fn tracing_init() -> eyre::Result<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::{non_blocking, rolling};
    use tracing_error::ErrorLayer;
    use tracing_subscriber::{
        EnvFilter, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    let (non_blocking_appender, guard) = non_blocking(rolling::daily("logs", "teamim-server"));
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

fn run(event_loop: EventLoopBuilder) -> eyre::Result<()> {
    let _ = tracing_init()?;

    let data = TaskList { loading: JobState::Ready(Ok(None)), sender: None };

    let app = Xilem::new_simple(data, TaskList::view, WindowOptions::new("טעמים"));
    app.run_in(event_loop).wrap_err("event loop error")
}

// Boilerplate code: Identical across all applications which support Android

#[expect(clippy::allow_attributes, reason = "No way to specify the condition")]
#[allow(dead_code, reason = "False positive: needed in not-_android version")]
// This is treated as dead code by the Android version of the example, but is actually live
// This hackery is required because Cargo doesn't care to support this use case, of one
// example which works across Android and desktop
fn main() -> eyre::Result<()> {
    run(EventLoop::with_user_event())
}
#[cfg(target_os = "android")]
// Safety: We are following `android_activity`'s docs here
#[expect(
    unsafe_code,
    reason = "We believe that there are no other declarations using this name in the compiled objects here"
)]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    let mut event_loop = EventLoop::with_user_event();
    event_loop.with_android_app(app);

    run(event_loop).expect("Can create app");
}
