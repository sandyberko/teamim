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
use teamim::{PlaceOptions, TeamimCtx, leptonica_ext::PixBox};
use tracing::error;
use xilem::{
    Blob, Color, EventLoop, EventLoopBuilder, Image, ImageFormat, TextAlign, WidgetView,
    WindowOptions, Xilem,
    core::{fork, map_state},
    masonry::properties::types::{AsUnit, Length},
    style::{Padding, Style},
    view::{
        CrossAxisAlignment, MainAxisAlignment, ObjectFit, flex, flex_row, image, portal, prose,
        sized_box, task_raw, zstack,
    },
};

use crate::{
    box_view::tbox,
    job::{FutureSender, Job, future_btn},
    view_ext::ViewExt,
};

const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];
const FONT_SIZE: Length = Length::const_px(16.);
const PADDING: Padding = Padding::all(FONT_SIZE.get() * 0.2);

thread_local! {
    static TEAMIM_CTX: RefCell<Option<TeamimCtx>> = const { RefCell::new(None) };
}

#[derive(Debug)]
enum ModifiedState {
    Unmodified,
    Modified { saving: FutureSender<(Image, Arc<Path>), ()> },
}

impl ModifiedState {
    pub(crate) fn new_modified() -> Self {
        Self::Modified { saving: FutureSender::new(()) }
    }

    fn modified_mut(&mut self) -> Option<&mut FutureSender<(Image, Arc<Path>), ()>> {
        match self {
            Self::Modified { saving } => Some(saving),
            Self::Unmodified => None,
        }
    }
}

#[derive(Debug)]
struct LoadedImage {
    path: Arc<Path>,
    img: Image,
    drawing: FutureSender<Image, ModifiedState>,
}

impl LoadedImage {
    fn new(img: Image, path: Arc<Path>) -> Self {
        Self { img, path, drawing: FutureSender::new(ModifiedState::Unmodified) }
    }

    fn view(&mut self) -> impl WidgetView<Self> + use<> {
        let draw_worker =
            FutureSender::worker(Self::draw_teamim, |_| ModifiedState::new_modified())
                .map_state::<Self, _>(|loaded_img| &mut loaded_img.drawing)
                .map_action(|loaded_img, data| {
                    let Some(data) = data else { return };
                    data.clone_into(&mut loaded_img.img.data);
                });

        let err_msg = "saving when image is modified";
        let save_worker = FutureSender::worker(Self::try_save, |()| ())
            .map_state::<Self, _>(|loaded_image| {
                loaded_image
                    .drawing
                    .state
                    .ready_ok_mut()
                    .and_then(ModifiedState::modified_mut)
                    .expect(err_msg)
            })
            .map_action(|loaded_img, done| {
                if done.is_some() {
                    *loaded_img.drawing.state.ready_ok_mut().expect(err_msg) =
                        ModifiedState::Unmodified;
                }
            });

        fork(
            // TODO: zoom
            portal(image(&self.img).fit(ObjectFit::FitWidth)),
            (
                draw_worker,
                self.drawing
                    .state
                    .ready_ok_mut()
                    .and_then(ModifiedState::modified_mut)
                    .map(|_| save_worker),
            ),
        )
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

struct TaskList {
    img: Job<Option<LoadedImage>, Arc<Path>>,
}
impl TaskList {
    fn handle_draw(&mut self) {
        let Job::Ready(Ok(Some(loaded))) = &mut self.img else {
            error!("Drawing button pressed while no image is loaded");
            return;
        };
        loaded
            .drawing
            .sender
            .as_mut()
            .expect("draw request sender to be set")
            .send(loaded.img.clone())
            .ok();
    }

    fn handle_save_req(&mut self) {
        let Some(img) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
            error!("Saving button pressed when image is not modified");
            return;
        };

        let payload = (img.img.clone(), Arc::clone(&img.path));

        let Some(saving) = img.drawing.state.ready_ok_mut().and_then(ModifiedState::modified_mut)
        else {
            error!("Saving button pressed when image is not modified");
            return;
        };

        saving.sender.as_mut().expect("save request sender to be set").send(payload).ok();
    }

    fn handle_img_select(&mut self) {
        self.img = FileDialog::new()
            .set_title("בחר תמונה")
            .add_filter("תמונה", IMG_EXTS)
            .pick_file()
            .map_or(Job::Ready(Ok(None)), |path| Job::Running(path.into()));
    }

    fn view_pending(path: &Arc<Path>) -> impl WidgetView<Self> + use<> {
        let draw_task = task_raw(
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
                state.img = Job::Ready(image.map(|img| Some(LoadedImage::new(img, path))));
            },
        );

        fork(
            flex(prose(path.to_string_lossy()).text_alignment(TextAlign::Center).text_color(GRAY)),
            draw_task,
        )
    }

    fn view(&mut self) -> impl WidgetView<Self> + use<> {
        flex((
            // toolbar
            flex_row((
                // save
                self.img.ready_ok_mut().and_then(|img| {
                    let saving = img.as_mut()?.drawing.state.ready_ok_mut()?.modified_mut()?;
                    Some(future_btn(&saving.state, "שומר...", "שמור", Self::handle_save_req))
                }),
                // draw
                self.img.ready_ok().and_then(Option::as_ref).map(|loaded| {
                    future_btn(&loaded.drawing.state, "מצייר...", "צייר טעמים", Self::handle_draw)
                }),
                // select
                future_btn(&self.img, "טוען...", "בחר תמונה", Self::handle_img_select),
            ))
            .main_axis_alignment(MainAxisAlignment::End)
            .padding(PADDING)
            .boxed(),
            match &mut self.img {
                Job::Running(path) => Self::view_pending(path).boxed(),
                Job::Ready(Ok(Some(loaded))) => map_state(loaded.view(), |state: &mut Self| {
                    // TODO panic???
                    if let Job::Ready(Ok(Some(loaded))) = &mut state.img {
                        loaded
                    } else {
                        // error!("LoadedState rquested while state.img is {:?}", state.img);
                        // panic!("LoadedState rquested while state.img is {:?}", state.img)
                        // TODO
                        panic!()
                    }
                })
                .boxed(),
                Job::Ready(Ok(None)) => prose("לא נבחרה תמונה")
                    .text_alignment(TextAlign::Center)
                    .text_color(GRAY)
                    .boxed(),
                Job::Ready(Err(_)) => prose("🖼️").boxed(),
            },
        ))
        .cross_axis_alignment(CrossAxisAlignment::Fill)
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
    let data = TaskList { img: Job::Ready(Ok(None)) };

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
