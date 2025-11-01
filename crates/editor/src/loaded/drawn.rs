#[cfg(test)]
mod tests;

use iced::{ContentFit, Transformation, widget::mouse_area};
use image::imageops::overlay;
use teamim::diac::{self, DiacMiss};
use tokio::task::spawn_blocking;

use crate::{
    FONT_SIZE, IMG_EXTS, NamedImg, SaveStatus, diac_renderer, img::ImgHandle, stage, strs,
    task::Poll, with_tctx,
};
use {
    eyre::{OptionExt as _, WrapErr as _},
    iced::{
        Element, Point, Subscription, Task,
        futures::StreamExt,
        keyboard::{Key, key::Named, on_key_press},
        mouse::Interaction,
        stream::channel,
        widget::{button, text},
    },
    image::{ImageBuffer, ImageFormat, Rgb, Rgba, RgbaImage, buffer::ConvertBuffer},
    rfd::AsyncFileDialog,
    std::{ffi::CStr, ops::Deref, sync::Arc},
    tap::prelude::*,
    teamim::{PositStatus, tesseract_ext::bounding_box::Rect},
};

pub type PollDraw = Poll<Result<Vec<RenderedDiac>, Arc<eyre::Report>>, PositStatus>;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Save(Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>),
    Place(PlaceMsg),
}

#[derive(Debug, Clone)]
pub(crate) enum PlaceMsg {
    /// enters placing mode with the given miss' diacritic
    StartMode(usize),
    Commit(Point),
    Cancel,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderedDiac {
    diac: char,
    position: RenderedDiacPos,
    img: ImgHandle,
}

#[derive(Debug, Clone)]
pub(crate) enum RenderedDiacPos {
    Letter(Rect<u32>),
    Exact(Point),
    Miss(DiacMiss),
}

impl RenderedDiacPos {
    fn pos_f(&self) -> Option<Point> {
        #[expect(clippy::cast_precision_loss)]
        match self {
            RenderedDiacPos::Letter(rect) => {
                Some([rect.left, rect.top].map(|coord| coord as f32).into())
            }
            RenderedDiacPos::Exact(point) => Some(*point),
            RenderedDiacPos::Miss(_) => None,
        }
    }

    fn pos_i(&self) -> Option<Point<u32>> {
        match self {
            RenderedDiacPos::Letter(rect) => Some([rect.left, rect.top].into()),
            RenderedDiacPos::Exact(point) => Some(point.snap()),
            RenderedDiacPos::Miss(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct Drawn {
    diacs: Vec<RenderedDiac>,
    saving: Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>,
    placing: Option<usize>,
}

impl Drawn {
    pub fn new(diacs: Vec<RenderedDiac>) -> Self {
        Self { diacs, saving: Poll::Ready(Ok(())), placing: None }
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Save(msg) => match msg {
                // trigger
                Poll::Pending(SaveStatus::Trigger(img)) => {
                    return Task::stream(
                        channel(1, {
                            let positions = self.diacs.clone();
                            async move |mut tx| {
                                let report =
                                    |status| _ = tx.clone().try_send(Poll::Pending(status));
                                let result = save(img, positions, report).await.map_err(Arc::new);
                                _ = tx.try_send(Poll::Ready(result));
                            }
                        })
                        .map(Message::Save),
                    );
                }
                Poll::Pending(status) => {
                    self.saving = Poll::Pending(status);
                }
                Poll::Ready(msg) => {
                    self.saving = Poll::Ready(msg);
                }
            },
            Message::Place(msg) => match msg {
                PlaceMsg::StartMode(diac_idx) => {
                    self.placing = Some(diac_idx);
                }
                PlaceMsg::Commit(pos) => {
                    if let Some(diac_idx) = self.placing.take() {
                        let pos = Point::new(pos.x, pos.y);
                        self.diacs[diac_idx].position = RenderedDiacPos::Exact(pos);
                    }
                }
                PlaceMsg::Cancel => {
                    self.placing = None;
                }
            },
        }
        Task::none()
    }

    pub fn subscription() -> Subscription<Message> {
        on_key_press(|key, _| {
            (key == Key::Named(Named::Escape)).then_some(Message::Place(PlaceMsg::Cancel))
        })
    }

    pub fn toolbar_view<'a>(&self, img_to_save: NamedImg) -> Element<'a, Message> {
        self.saving
            .loading_btn()
            .on_press(Message::Save(Poll::Pending(SaveStatus::Trigger(img_to_save))))
            .into()
    }

    pub fn diac_view<'a>(&self, img: &ImgHandle) -> Element<'a, Message> {
        stage(self.diacs.iter().enumerate().filter_map(|(idx, pos)| {
            Some((
                mouse_area(pos.img.view())
                    .on_press(Message::Place(PlaceMsg::StartMode(idx)))
                    .into(),
                pos.position.pos_f()?,
            ))
        }))
        .handle(img.handle().clone())
        // .on_press(move |pos| Message::Place(PlaceMsg::Commit(pos)))
        // .interaction(if self.placing.is_some() {
        //     Interaction::Crosshair
        // } else {
        //     Interaction::default()
        // })
        .content_fit(ContentFit::Cover)
        .into()
    }

    pub(crate) fn misses_view(&self) -> Element<'_, Message> {
        // stage(self.diacs.iter().enumerate().filter_map(|(result_idx, result)| {
        //     let RenderedDiacPos::Miss(miss) = &result.position else {
        //         return None;
        //     };
        //     Some((
        //         button(text(&miss.missing_text).size(16))
        //             .on_press_maybe(
        //                 if let Some(diac_idx) = self.placing
        //                     && diac_idx == result_idx
        //                 {
        //                     None
        //                 } else {
        //                     Some(Message::Place(PlaceMsg::StartMode(result_idx)))
        //                 },
        //             )
        //             .into(),
        //         #[expect(clippy::cast_precision_loss)]
        //         Point::new(0.0, miss.top as f32),
        //     ))
        // }))
        // .into()
        text("TODO").into()
    }
}

async fn save(
    img: NamedImg,
    positions: Vec<RenderedDiac>,
    progress: impl Fn(SaveStatus),
) -> eyre::Result<()> {
    progress(SaveStatus::SelectingFile);
    // [TODO]
    let mut dialog = AsyncFileDialog::new().set_title(strs::SAVE).add_filter("image", IMG_EXTS);
    if let Some(dir) = img.path.parent() {
        dialog = dialog.set_directory(dir);
    }
    if let Some(file_name) = img.path.file_name() {
        dialog = dialog.set_file_name(file_name.to_str().ok_or_eyre("שם הקובץ לא תקין")?);
    }
    let file = dialog.save_file().await.ok_or_eyre("שמירה בוטלה")?;

    progress(SaveStatus::Rendering);
    let img = spawn_blocking(move || {
        let img = img.img.img();
        let (width, height) = img.dimensions();
        // [TODO] try not to clone
        let mut bottom = RgbaImage::from_raw(width, height, img.to_vec()).unwrap();
        for diac in positions {
            let Some(Point { x, y }) = diac.position.pos_i() else { continue };
            overlay(&mut bottom, &diac.img.img(), x.into(), y.into());
        }
        bottom
    })
    .await
    .expect("blocking task to finish");

    progress(SaveStatus::Writing);
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

pub(super) fn position_diacs(
    img: &ImageBuffer<Rgba<u8>, impl Deref<Target = [u8]>>,
    datapath: &CStr,
    progress_callback: impl Fn(PositStatus),
) -> eyre::Result<Vec<RenderedDiac>> {
    with_tctx(datapath, |ctx| {
        let positions = ctx.positions(img, progress_callback).wrap_err("place error")?;
        render_diacs(positions)
    })?
}

// [TODO] DRY
fn render_diacs(positions: Vec<(char, diac::Result)>) -> Result<Vec<RenderedDiac>, eyre::Error> {
    let mut renderer = diac_renderer::Renderer::new();
    positions
        .into_iter()
        .map(|(diac, position)| RenderedDiac {
            diac,
            position: match position {
                Ok(rect) => RenderedDiacPos::Letter(rect),
                Err(diac_miss) => RenderedDiacPos::Miss(diac_miss),
            },
            img: renderer.render(diac, FONT_SIZE).into(),
        })
        .collect::<Vec<_>>()
        .pipe(Ok)
}
