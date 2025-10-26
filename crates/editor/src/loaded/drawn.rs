#[cfg(test)]
mod tests;

use crate::{
    FONT_SIZE, IMG_EXTS, NamedImg, SaveStatus, diac_renderer, img::ImgHandle, loaded::Transform,
    stage, strs, task::Poll, with_tctx,
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
    teamim::{DiacResultKind, PositStatus, tesseract_ext::bounding_box::Rect},
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
    Commit(Point, Transform),
    Cancel,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderedDiac {
    diac: char,
    kind: DiacResultKind,
    img: ImgHandle,
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
                            let result = save(img, &*positions, report).await.map_err(Arc::new);
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
                PlaceMsg::Commit(pos, transform) => {
                    if let Some(diac_idx) = self.placing.take() {
                        self.diacs[diac_idx].kind = DiacResultKind::Pos(Rect {
                            left: ((pos.x - transform.scroll_offset.x) / transform.zoom) as u32,
                            // [TODO]
                            bottom: 0,
                            // [TODO]
                            right: 0,
                            top: ((pos.y - transform.scroll_offset.y) / transform.zoom) as u32,
                        });
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

    pub fn diac_view(&self, opts: Transform) -> Element<'_, Message> {
        let Transform { scroll_offset, zoom } = opts;
        stage(self.diacs.iter().filter_map(|pos| {
            let DiacResultKind::Pos(rect) = pos.kind else { return None };
            Some((
                pos.img.view(zoom),
                #[expect(clippy::cast_precision_loss)]
                [rect.left, rect.top].map(|coord| coord as f32 * zoom).into(),
            ))
        }))
        .on_press(move |pos| {
            Message::Place(PlaceMsg::Commit(pos, Transform { scroll_offset, zoom }))
        })
        .interaction(if self.placing.is_some() {
            Interaction::Crosshair
        } else {
            Interaction::default()
        })
        .into()
    }

    pub(crate) fn misses_view(&self, zoom: f32) -> Element<'_, Message> {
        stage(self.diacs.iter().enumerate().filter_map(|(result_idx, result)| {
            let DiacResultKind::Miss(miss) = &result.kind else {
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

async fn save<'d>(
    img: NamedImg,
    positions: impl IntoIterator<Item = &'d RenderedDiac>,
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
    let img = img.img.img();
    let (width, height) = img.dimensions();
    // [TODO] try not to clone
    let mut img = RgbaImage::from_raw(width, height, img.to_vec()).unwrap();
    overlay_diacs(positions, &mut img);

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
        let DiacResultKind::Pos(rect) = diac.kind else { continue };
        overlay(img, &diac.img.img(), rect.left.into(), rect.top.into());
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
        .map(|(diac, kind)| RenderedDiac {
            diac,
            kind,
            img: renderer.render(diac, FONT_SIZE).into(),
        })
        .collect::<Vec<_>>()
        .pipe(Ok)
}
