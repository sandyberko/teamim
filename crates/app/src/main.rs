// TODO replace boxed with OneOf

// On Windows platform, don't show a console when opening the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod box_view;

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
};

use eyre::WrapErr;
use image::ImageReader;
use rfd::FileDialog;
use teamim::{PlaceOptions, TeamimCtx, leptonica_ext::PixBox};
use tracing::{debug, error};
use xilem::{
    Blob, Color, EventLoop, EventLoopBuilder, Image, ImageFormat, WidgetView, WindowOptions, Xilem,
    core::{fork, lens, map_state},
    masonry::properties::types::{AsUnit, Length},
    tokio::sync::mpsc::UnboundedSender,
    view::{
        MainAxisAlignment, ObjectFit, button, flex, flex_row, image, portal, prose, sized_box,
        task_raw, worker, zstack,
    },
};

use crate::box_view::tbox;

const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];
const FONT_SIZE: Length = Length::const_px(16.);

fn spinner<S: 'static, A: 'static>() -> impl WidgetView<S, A> + use<S, A> {
    sized_box(xilem::view::spinner()).height(FONT_SIZE).width(FONT_SIZE)
}

#[derive(Default, Debug)]
enum ImgState {
    #[default]
    None,
    Pending(Arc<PathBuf>),
    Loaded(eyre::Result<LoadedImage>),
}

thread_local! {
    static TEAMIM_CTX: RefCell<Option<TeamimCtx>> = const { RefCell::new(None) };
}

#[derive(Debug)]
struct LoadedImage {
    img: Image,
    drawing: Future<()>,
    request: Option<UnboundedSender<Image>>,
}

impl LoadedImage {
    fn new(img: Image) -> Self {
        Self { img, drawing: Future::Ready(Ok(())), request: None }
    }

    fn view(&mut self) -> impl WidgetView<Self> + use<> {
        fork(
            flex((
                future_btn(&self.drawing, |state: &mut Self| {
                    state
                        .request
                        .as_mut()
                        .expect("draw request sender to be set")
                        .send(state.img.clone())
                        .ok();
                }),
                // TODO: zoom
                portal(image(&self.img).fit(ObjectFit::FitWidth)),
            )),
            worker(
                move |proxy, mut recv| async move {
                    while let Some(img) = recv.recv().await {
                        proxy.message(Future::Pending).ok();
                        let result = Self::draw_teamim(&img);
                        proxy.message(Future::Ready(result)).ok();
                    }
                },
                |state: &mut Self, sender| state.request = Some(sender),
                |state: &mut Self, resp: Future<Blob<u8>>| {
                    state.drawing = resp.map(|data| {
                        state.img.data = data;
                    });
                },
            ),
        )
        .boxed()
    }

    fn draw_teamim(img: &Image) -> eyre::Result<Blob<u8>> {
        TEAMIM_CTX.with_borrow_mut(|ctx| {
            if ctx.is_none() {
                *ctx = Some(TeamimCtx::new()?);
            }
            let ctx = ctx.as_mut().unwrap();

            let options = PlaceOptions::default();
            let mut data = img.data.data().to_vec();
            PixBox::from_rgba8_with(
                &mut data,
                img.width.try_into().unwrap(),
                img.height.try_into().unwrap(),
                |img| ctx.place_teamim_pix(img, options),
            )
            .unwrap_or_else(|err| Err(err.into()))?;
            Ok(Blob::new(Arc::new(data)))
        })
    }
}

#[derive(Debug)]
enum Future<T> {
    Ready(eyre::Result<T>),
    Pending,
}

impl<T> Future<T> {
    fn map<U, F>(self, f: F) -> Future<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Ready(result) => Future::Ready(result.map(f)),
            Self::Pending => Future::Pending,
        }
    }
}

fn future_btn<State: 'static, Action: 'static>(
    state: &Future<()>,
    callback: impl Fn(&mut State) -> Action + Send + Sync + 'static,
) -> impl WidgetView<State, Action> {
    match state {
        Future::Ready(status) => {
            let btn = button("צייר טעמים", callback);
            match status {
                Ok(()) => btn.boxed(),
                Err(err) => flex((btn, err_prose(err))).boxed(),
            }
        }
        Future::Pending => flex_row((spinner(), prose("מצייר...")))
            .main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .must_fill_major_axis(false)
            .boxed(),
    }
}

const RED: Color = Color::from_rgb8(255, 0, 0);
fn err_prose<S, A>(msg: &eyre::Error) -> impl WidgetView<S, A> + use<S, A> {
    prose(msg.to_string()).text_color(RED)
}

impl ImgState {
    fn view(&mut self) -> impl WidgetView<Self> + use<> {
        debug!("ImgState::view: {self:?}");
        match self {
            Self::None => prose("לא נבחרה תמונה").boxed(),
            Self::Pending(path) => {
                let task = task_raw(
                    {
                        let path = path.clone();
                        move |proxy| {
                            // TODO double clone?
                            let path = path.clone();
                            async move {
                                let path_str = path.display();
                                let result = image_from_path(&*path);
                                if let Err(err) = &result {
                                    tracing::warn!("Loading image from {path_str} failed: {err:?}");
                                }
                                let _ = proxy.message(result);
                            }
                        }
                    },
                    move |state: &mut Self, image| {
                        *state = ImgState::Loaded(image.map(LoadedImage::new));
                    },
                );
                fork(flex((spinner(), prose(path.to_string_lossy()))), task).boxed()
            }
            Self::Loaded(Ok(loaded)) => map_state(loaded.view(), |img_state: &mut Self| {
                // TODO panic???
                if let Self::Loaded(Ok(loaded)) = img_state {
                    loaded
                } else {
                    error!("LoadedState rquested while img_state is {img_state:?}");
                    panic!("LoadedState rquested while img_state is {img_state:?}")
                }
            })
            .boxed(),
            Self::Loaded(Err(msg)) => err_prose(msg).boxed(),
        }
    }
}

fn image_from_path(path: impl AsRef<Path>) -> eyre::Result<Image> {
    let image = ImageReader::open(path)?.decode()?.into_rgba8();
    let width = image.width();
    let height = image.height();
    let data = image.into_vec();
    Ok(Image::new(Blob::new(Arc::new(data)), ImageFormat::Rgba8, width, height))
}

struct TaskList {
    img: ImgState,
}
impl TaskList {
    fn view(_: &mut Self) -> impl WidgetView<Self> + use<> {
        flex((
            button("בחר תמונה", Self::handle_img_select),
            lens(ImgState::view, |state: &mut Self| &mut state.img),
        ))
    }

    fn handle_img_select(&mut self) {
        self.img = FileDialog::new()
            .add_filter("תמונה", IMG_EXTS)
            .pick_file()
            .map_or(ImgState::default(), |path| ImgState::Pending(Arc::new(path)));
    }
}

fn tboxes_example<State>() -> impl WidgetView<State> + use<State>
where
    State: Send + Sync + 'static,
{
    sized_box(zstack((tbox((0., 0.), (200., 50.)), tbox((0., 70.), (200., 50.)))))
        .width(200.px())
        .height(200.px())
}

fn run(event_loop: EventLoopBuilder) -> eyre::Result<()> {
    let data = TaskList { img: ImgState::default() };

    let app = Xilem::new_simple(data, TaskList::view, WindowOptions::new("To Do MVC"));
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
