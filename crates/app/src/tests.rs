use std::path::Path;

use eyre::WrapErr;
use teamim::DiacMiss;
use xilem::{
    EventLoop, WindowOptions, Xilem, winit::platform::windows::EventLoopBuilderExtWindows,
};

use crate::{DrawIdle, LoadedImage, Modified, image_read, job::JobState, view_ext::ViewExt};

#[test]
fn zstack_rtl_text_placement() -> eyre::Result<()> {
    let mut event_loop_builder = EventLoop::with_user_event();
    event_loop_builder.with_any_thread(true);

    let path = Path::new("../../assets/images/N5/007.jpg");
    let misses = vec![
        DiacMiss::new('ע', '׃', 7690, 674, 2248, "ץ"),
        DiacMiss::new('ך', '\u{5a5}', 7729, 883, 2248, "ר"),
        DiacMiss::new('נ', '\u{5a7}', 7833, 1321, 2248, "ת"),
        DiacMiss::new('ה', '׃', 7898, 1592, 2248, "ג"),
        DiacMiss::new('ע', '\u{5a7}', 8052, 2271, 2292, "ש"),
        DiacMiss::new('נ', '\u{591}', 8182, 2835, 2292, "ג"),
    ];
    let img = image_read(path)?;
    let state = LoadedImage {
        path: path.into(),
        img: img.clone(),
        drawing: JobState::Ready(Ok(DrawIdle::Modified(Modified {
            saving: JobState::Ready(Ok(())),
            image: img,
            misses,
            diff: vec![],
        }))),
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
