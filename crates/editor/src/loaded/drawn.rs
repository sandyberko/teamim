#[cfg(test)]
mod tests;

use iced::{Transformation, widget::mouse_area};
use teamim::DiacMiss;
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
    image::{
        ImageBuffer, ImageFormat, Rgb, Rgba, RgbaImage, buffer::ConvertBuffer, imageops::overlay,
    },
    num_traits::AsPrimitive,
    rfd::AsyncFileDialog,
    std::{ffi::CStr, ops::Deref, sync::Arc},
    tap::prelude::*,
    teamim::{DiacPos, PositStatus, tesseract_ext::bounding_box::Rect},
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
    Commit(Point, f32),
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
                Poll::Pending(SaveStatus::Trigger(img)) => Task::stream(
                    channel(1, {
                        let positions = self.diacs.clone();
                        async move |mut tx| {
                            let report = |status| _ = tx.clone().try_send(Poll::Pending(status));
                            let result = save(img, positions, report).await.map_err(Arc::new);
                            _ = tx.try_send(Poll::Ready(result));
                        }
                    })
                    .map(Message::Save),
                ),
                Poll::Pending(status) => {
                    self.saving = Poll::Pending(status);
                    Task::none()
                }
                Poll::Ready(msg) => {
                    self.saving = Poll::Ready(msg);
                    Task::none()
                }
            },
            Message::Place(msg) => match msg {
                PlaceMsg::StartMode(diac_idx) => {
                    self.placing = Some(diac_idx);
                    Task::none()
                }
                PlaceMsg::Commit(pos, zoom) => {
                    if let Some(diac_idx) = self.placing.take() {
                        let pos = Point::new(pos.x / zoom, pos.y / zoom);
                        self.diacs[diac_idx].position = RenderedDiacPos::Exact(pos);
                    }
                    Task::none()
                }
                PlaceMsg::Cancel => {
                    self.placing = None;
                    Task::none()
                }
            },
        }
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

    pub fn diac_view(&self, zoom: f32) -> Element<'_, Message> {
        stage(self.diacs.iter().enumerate().filter_map(|(idx, pos)| {
            Some((
                mouse_area(pos.img.view(zoom))
                    .on_press(Message::Place(PlaceMsg::StartMode(idx)))
                    .into(),
                pos.position.pos_f()? * Transformation::scale(zoom),
            ))
        }))
        .on_press(move |pos| Message::Place(PlaceMsg::Commit(pos, zoom)))
        .interaction(if self.placing.is_some() {
            Interaction::Crosshair
        } else {
            Interaction::default()
        })
        .into()
    }

    pub(crate) fn misses_view(&self, zoom: f32) -> Element<'_, Message> {
        stage(self.diacs.iter().enumerate().filter_map(|(result_idx, result)| {
            let RenderedDiacPos::Miss(miss) = &result.position else {
                return None;
            };
            Some((
                button(text(&miss.missing_text).size(48.0 * zoom))
                    .on_press_maybe(
                        if let Some(diac_idx) = self.placing
                            && diac_idx == result_idx
                        {
                            None
                        } else {
                            Some(Message::Place(PlaceMsg::StartMode(result_idx)))
                        },
                    )
                    .into(),
                Point::new(0.0, AsPrimitive::<f32>::as_(miss.top) * zoom),
            ))
        }))
        .into()
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
        let mut img = RgbaImage::from_raw(width, height, img.to_vec()).unwrap();
        overlay_diacs(&positions, &mut img);
        img
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

fn overlay_diacs<'d>(positions: impl IntoIterator<Item = &'d RenderedDiac>, img: &mut RgbaImage) {
    for diac in positions {
        let Some(Point { x, y }) = diac.position.pos_i() else { continue };
        overlay(img, &diac.img.img(), x.into(), y.into());
    }
}

pub(super) fn render_diacs(
    img: &ImageBuffer<Rgba<u8>, impl Deref<Target = [u8]>>,
    datapath: &CStr,
    progress_callback: impl Fn(PositStatus),
) -> eyre::Result<Vec<RenderedDiac>> {
    let mut renderer = diac_renderer::Renderer::new();
    with_tctx(datapath, |ctx| ctx.positions(img, progress_callback).wrap_err("place error"))??
        .into_iter()
        .map(|(diac, position)| RenderedDiac {
            diac,
            position: match position {
                DiacPos::Pos(rect) => RenderedDiacPos::Letter(rect),
                DiacPos::Miss(diac_miss) => RenderedDiacPos::Miss(diac_miss),
            },
            img: renderer.render(diac, FONT_SIZE).into(),
        })
        .collect::<Vec<_>>()
        .pipe(Ok)
}
