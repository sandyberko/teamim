use std::path::Path;

use eyre::WrapErr;
use teamim::TeamimCtx;
use xilem::{
    EventLoop, WindowOptions, Xilem, winit::platform::windows::EventLoopBuilderExtWindows,
};

use crate::{DrawIdle, LoadedImage, diff_boxes, image_read, job::JobState, view_ext::ViewExt};

#[test]
fn zstack_rtl_text_placement() -> eyre::Result<()> {
    let mut event_loop_builder = EventLoop::with_user_event();
    event_loop_builder.with_any_thread(true);

    let path = Path::new("../../assets/images/N5/007.jpg");
    let img = image_read(path)?;
    let mut ctx = TeamimCtx::new(c"../../assets/tessdata/")?;
    let modified = diff_boxes(&mut ctx, &img, |_| {})?;
    let state = LoadedImage {
        path: path.into(),
        img: img.clone(),
        drawing: JobState::Ready(Ok(DrawIdle::Modified(modified))),
        zoom: 0.5,
    };
    Xilem::new_simple(
        state,
        |state: &mut LoadedImage| state.img_view().map_state(|_| Box::leak(Box::new(()))),
        WindowOptions::new("טעמים"),
    )
    .run_in(event_loop_builder)
    .wrap_err("event loop error")
}
