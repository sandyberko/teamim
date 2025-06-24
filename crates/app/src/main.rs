// Copyright 2024 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

//! A to-do-list app, loosely inspired by todomvc.

// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

mod box_view;

use masonry::kurbo::Size;
use xilem::view::{Axis, PointerButton, Split, button, checkbox, flex, sized_box, textbox, zstack};
use xilem::winit::error::EventLoopError;
use xilem::{Affine, EventLoop, EventLoopBuilder, InsertNewline, WidgetView, Xilem};

use crate::box_view::TBox;

struct Task {
    description: String,
    done: bool,
}

struct TaskList {
    next_task: String,
    tasks: Vec<Task>,
}

impl TaskList {
    fn add_task(&mut self) {
        if self.next_task.is_empty() {
            return;
        }
        self.tasks.push(Task { description: std::mem::take(&mut self.next_task), done: false });
    }
}

fn foo(x: impl WidgetView<TaskList>) {}

fn app_logic(task_list: &mut TaskList) -> impl WidgetView<TaskList> + use<> {
    
    
    sized_box(zstack((TBox::new((0., 0.), (200., 50.)), TBox::new((0., 70.), (200., 50.)))))
        .width(200.)
        .height(200.)
}

fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let data = TaskList {
        // Add a placeholder task for Android, whilst the
        next_task: "My Next Task".into(),
        tasks: vec![
            Task { description: "Buy milk".into(), done: false },
            Task { description: "Buy eggs".into(), done: true },
            Task { description: "Buy bread".into(), done: false },
        ],
    };

    let app = Xilem::new(data, app_logic);
    app.run_windowed(event_loop, "Xilem Example".to_owned())
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
