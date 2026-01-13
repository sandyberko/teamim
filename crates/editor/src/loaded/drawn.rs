#[cfg(test)]
mod tests;

use {
    crate::{
        IMG_EXTS, NamedImg, PADDING, SaveStatus,
        diac_renderer::{self, DiacPositionOpts},
        img::ImgHandle,
        stage, strs,
        task::Poll,
    },
    editor::stage::Overlay,
    eyre::{OptionExt as _, WrapErr as _},
    iced::{
        ContentFit, Element, Font, Point, Rectangle, Size, Task,
        futures::StreamExt,
        stream::channel,
        widget::{button, text, toggler},
    },
    image::{ImageBuffer, ImageFormat, Rgb, RgbaImage, buffer::ConvertBuffer, imageops::overlay},
    rfd::AsyncFileDialog,
    std::{iter::zip, sync::Arc},
    teamim::{DiacRect, PositStatus, diac::DiacMiss, tesseract_ext::bounding_box::Rect},
    tokio::task::spawn_blocking,
};

#[derive(Debug, Clone)]
pub(crate) struct Position {
    letter: Option<Rect<u32>>,
    diac: Point,
}

impl Position {
    pub(crate) fn new(letter: Rect<u32>, diac: Point) -> Self {
        Self { letter: Some(letter), diac }
    }
    pub(crate) fn new_miss(diac: Point) -> Self {
        Self { letter: None, diac }
    }
}

pub type PollDraw = Poll<Result<Vec<RenderedDiac>, Arc<eyre::Report>>, PositStatus>;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Position(DiacPositionOpts),
    MoveDiac(usize, Point),
    Save(Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>),
    ShowRects(bool),
}

#[derive(Debug, Clone)]
pub(crate) struct RenderedDiac {
    pub(crate) letter: char,
    pub(crate) diac: char,
    pub(crate) position: Result<Position, DiacMiss>,
    img: ImgHandle,
}

impl RenderedDiac {
    pub(crate) fn new(
        letter: char,
        diac: char,
        position: Result<Position, DiacMiss>,
        img: ImgHandle,
    ) -> Self {
        Self { letter, diac, position, img }
    }
}

#[derive(Debug)]
pub struct Drawn {
    pub(crate) diacs: Vec<RenderedDiac>,
    show_rects: bool,
    saving: Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>,
}

impl Drawn {
    pub fn new(diacs: Vec<RenderedDiac>) -> Self {
        Self { diacs, show_rects: false, saving: Poll::Ready(Ok(())) }
    }

    pub(crate) fn from_rects(
        rects: &[DiacRect],
        opts: DiacPositionOpts,
        diac_color: [u8; 3],
    ) -> Self {
        let mut renderer = diac_renderer::Renderer::new();
        let diacs = rects
            .iter()
            .map(|&(letter, diac, ref pos)| {
                // TODO
                let position = pos.clone().map(|letter_pos| {
                    let diac_pos =
                        diac_renderer::Renderer::position(letter, diac, letter_pos, opts);
                    Position::new(letter_pos, diac_pos)
                });
                let img = renderer.render(diac, opts.font_size, diac_color).into();
                RenderedDiac::new(letter, diac, position, img)
            })
            .collect();
        Self::new(diacs)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ShowRects(msg) => self.show_rects = msg,
            Message::Position(opts) => {
                for diac in &mut self.diacs {
                    let Ok(position) = &mut diac.position else { continue };
                    let Some(letter_bounds) = position.letter else { continue };
                    position.diac = diac_renderer::Renderer::position(
                        diac.letter,
                        diac.diac,
                        letter_bounds,
                        opts,
                    );
                }
            }
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
            Message::MoveDiac(diac_idx, new_pos) => match &mut self.diacs[diac_idx].position {
                Ok(cur_pos) => cur_pos.diac = new_pos,
                cur_pos @ Err(..) => *cur_pos = Ok(Position::new_miss(new_pos)),
            },
        }
        Task::none()
    }

    pub fn toolbar_items<'a>(
        &self,
        img_to_save: NamedImg,
        diac_pos_opts: DiacPositionOpts,
    ) -> impl IntoIterator<Item = Element<'a, Message>> {
        [
            self.saving
                .loading_btn()
                .on_press(Message::Save(Poll::Pending(SaveStatus::Trigger(img_to_save))))
                .into(),
            toggler(self.show_rects).on_toggle(Message::ShowRects).into(),
            text(strs::SHOW_RECTS).into(),
            button(text(strs::POSITION)).on_press(Message::Position(diac_pos_opts)).into(),
        ]
    }

    pub fn diac_view<'a>(&self, img: &ImgHandle, rects: &[DiacRect]) -> Element<'a, Message> {
        stage(zip(&self.diacs, rects).map(|(diac, (_, _, rect))| match &diac.position {
            Ok(pos) => {
                // TODO
                let rect = rect.as_ref().ok().copied().unwrap_or(Rect::new(0, 0, 0, 0));
                #[expect(clippy::cast_precision_loss)]
                let rect = Rectangle::new(
                    Point::new(rect.left as _, rect.top as _),
                    Size::new(rect.width() as _, rect.height() as _),
                );
                Overlay::image(rect, pos.diac, diac.img.handle().clone())
            }
            Err(miss) =>
            {
                #[expect(clippy::cast_precision_loss)]
                Overlay::text(
                    Point::new(img.img().width() as f32 + f32::from(PADDING), miss.top as f32),
                    diac.img.handle().clone(),
                    miss.missing_text.clone(),
                )
            }
        }))
        .handle(img.handle().clone())
        .on_move(Message::MoveDiac)
        .content_fit(ContentFit::None)
        .font(Font::with_name("Guttman Stam"))
        .font_size(48.0)
        .show_rects(self.show_rects)
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
        let mut bottom = RgbaImage::from_raw(width, height, img.to_vec()).unwrap();
        for diac in positions {
            let Ok(pos) = diac.position else { continue };
            let Point { x, y } = pos.diac.snap();
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
