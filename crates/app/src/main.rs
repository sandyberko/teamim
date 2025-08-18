// Copyright 2024 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

//! A to-do-list app, loosely inspired by todomvc.

// On Windows platform, don't show a console when opening the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod box_view;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use rfd::FileDialog;
use xilem::{
    core::{fork, lens}, masonry::properties::types::{AsUnit, Length}, view::{button, flex, image, portal, prose, sized_box, spinner, task, task_raw, zstack}, winit::error::EventLoopError, Blob, Color, EventLoop, EventLoopBuilder, Image, ImageFormat, WidgetView, WindowOptions, Xilem
};

use crate::box_view::tbox;

const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];
const FONT_SIZE: Length = Length::const_px(16.);

#[derive(Default)]
enum ImgState {
    #[default]
    None,
    Pending(Arc<PathBuf>),
    Loaded(eyre::Result<Image>),
}
const RED: Color = Color::from_rgb8(255, 0, 0);
impl ImgState {
    fn view(&mut self) -> impl WidgetView<Self> + use<> {
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
                                let result = image_from_path(&*path).await;
                                if let Err(err) = &result {
                                    tracing::warn!("Loading image from {path_str} failed: {err:?}");
                                }
                                let _ = proxy.message(result);
                            }
                        }
                    },
                    move |state: &mut Self, image| {
                        *state = ImgState::Loaded(image);
                    },
                );
                fork(
                    flex((
                        sized_box(spinner()).height(FONT_SIZE).width(FONT_SIZE),
                        prose(path.to_string_lossy()),
                    )),
                    task,
                )
                .boxed()
            }
            Self::Loaded(Ok(img)) => portal(image(img)).boxed(),
            Self::Loaded(Err(msg)) => prose(msg.to_string()).text_color(RED).boxed(),
        }
    }
}

async fn image_from_path(path: impl AsRef<Path>) -> eyre::Result<Image> {
    let bytes = ::tokio::fs::read(path).await?;
    let image = image::load_from_memory(&bytes)?.into_rgba8();
    let width = image.width();
    let height = image.height();
    let data = image.into_vec();
    Ok(Image::new(Blob::new(Arc::new(data)), ImageFormat::Rgba8, width, height))
}

#[derive(Default)]
struct TaskList {
    img: ImgState,
}
fn app_logic(_state: &mut TaskList) -> impl WidgetView<TaskList> + use<> {
    flex((
        button("בחר תמונה", |state: &mut TaskList| {
            state.img = FileDialog::new()
                .add_filter("תמונה", IMG_EXTS)
                .pick_file()
                .map_or(ImgState::default(), |path| ImgState::Pending(Arc::new(path)));
        }),
        lens(ImgState::view, |state: &mut TaskList| &mut state.img),
    ))
}

fn tboxes_example<State>() -> impl WidgetView<State> + use<State>
where
    State: Send + Sync + 'static,
{
    sized_box(zstack((tbox((0., 0.), (200., 50.)), tbox((0., 70.), (200., 50.)))))
        .width(200.px())
        .height(200.px())
}

fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let data = TaskList::default();

    let app = Xilem::new_simple(data, app_logic, WindowOptions::new("To Do MVC"));
    app.run_in(event_loop)
}

// Boilerplate code: Identical across all applications which support Android

#[expect(clippy::allow_attributes, reason = "No way to specify the condition")]
#[allow(dead_code, reason = "False positive: needed in not-_android version")]
// This is treated as dead code by the Android version of the example, but is actually live
// This hackery is required because Cargo doesn't care to support this use case, of one
// example which works across Android and desktop
fn main() -> Result<(), EventLoopError> {
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
