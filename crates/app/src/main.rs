// TODO replace boxed with OneOf

// On Windows platform, don't show a console when opening the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod box_view;
mod future;

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
    view::{ObjectFit, button, flex, image, portal, prose, sized_box, task_raw, worker, zstack},
};

use crate::{
    box_view::tbox,
    future::{Future, future_btn, spinner},
};

const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];
const FONT_SIZE: Length = Length::const_px(16.);

thread_local! {
    static TEAMIM_CTX: RefCell<Option<TeamimCtx>> = const { RefCell::new(None) };
}

#[derive(Debug)]
struct LoadedImage {
    img: Image,
    drawing: Future<(), ()>,
    request: Option<UnboundedSender<Image>>,
}

impl LoadedImage {
    fn new(img: Image) -> Self {
        Self { img, drawing: Future::Ready(Ok(())), request: None }
    }

    fn view(&mut self) -> impl WidgetView<Self> + use<> {
        fork(
            flex((
                future_btn(
                    &self.drawing,
                    "מצייר...",
                    "צייר טעמים",
                    |state: &mut Self| {
                        state
                            .request
                            .as_mut()
                            .expect("draw request sender to be set")
                            .send(state.img.clone())
                            .ok();
                    },
                ),
                // TODO: zoom
                portal(image(&self.img).fit(ObjectFit::FitWidth)),
            )),
            worker(
                move |proxy, mut recv| async move {
                    while let Some(img) = recv.recv().await {
                        proxy.message(Future::Pending(())).ok();
                        let result = Self::draw_teamim(&img);
                        proxy.message(Future::Ready(result)).ok();
                    }
                },
                |state: &mut Self, sender| state.request = Some(sender),
                |state: &mut Self, resp: Future<(), Blob<u8>>| {
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

const RED: Color = Color::from_rgb8(255, 0, 0);
fn err_prose<S, A>(msg: &eyre::Error) -> impl WidgetView<S, A> + use<S, A> {
    prose(msg.to_string()).text_color(RED)
}

fn image_from_path(path: impl AsRef<Path>) -> eyre::Result<Image> {
    let image = ImageReader::open(path)?.decode()?.into_rgba8();
    let width = image.width();
    let height = image.height();
    let data = image.into_vec();
    Ok(Image::new(Blob::new(Arc::new(data)), ImageFormat::Rgba8, width, height))
}

struct TaskList {
    img: Future<Arc<PathBuf>, Option<LoadedImage>>,
}
impl TaskList {
    fn view(&mut self) -> impl WidgetView<Self> + use<> {
        flex((
            button("בחר תמונה", Self::handle_img_select),
            match &mut self.img {
                Future::Pending(path) => {
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
                                        tracing::warn!(
                                            "Loading image from {path_str} failed: {err:?}"
                                        );
                                    }
                                    let _ = proxy.message(result);
                                }
                            }
                        },
                        move |state: &mut Self, image| {
                            state.img = Future::Ready(image.map(|img| Some(LoadedImage::new(img))));
                        },
                    );
                    fork(flex((spinner(), prose(path.to_string_lossy()))), task).boxed()
                }
                Future::Ready(Ok(Some(loaded))) => map_state(loaded.view(), |state: &mut Self| {
                    // TODO panic???
                    if let Future::Ready(Ok(Some(loaded))) = &mut state.img {
                        loaded
                    } else {
                        // error!("LoadedState rquested while state.img is {:?}", state.img);
                        // panic!("LoadedState rquested while state.img is {:?}", state.img)
                        // TODO
                        panic!()
                    }
                })
                .boxed(),
                Future::Ready(Ok(None)) => prose("לא נבחרה תמונה").boxed(),
                Future::Ready(Err(msg)) => err_prose(msg).boxed(),
            },
        ))
    }

    fn handle_img_select(&mut self) {
        self.img = FileDialog::new()
            .add_filter("תמונה", IMG_EXTS)
            .pick_file()
            .map_or(Future::Ready(Ok(None)), |path| Future::Pending(Arc::new(path)));
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
    let data = TaskList { img: Future::Ready(Ok(None)) };

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
