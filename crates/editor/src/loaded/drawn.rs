#[cfg(test)]
mod tests;

use {
    crate::{IMG_EXTS, NamedImg, PADDING, SaveStatus, img::ImgHandle, stage, strs, task::Poll},
    editor::stage::Overlay,
    eyre::{OptionExt as _, WrapErr as _},
    iced::{ContentFit, Font},
    iced::{Element, Point, Task, futures::StreamExt, stream::channel},
    image::imageops::overlay,
    image::{ImageBuffer, ImageFormat, Rgb, RgbaImage, buffer::ConvertBuffer},
    rfd::AsyncFileDialog,
    std::sync::Arc,
    teamim::PositStatus,
    teamim::diac::DiacMiss,
    tokio::task::spawn_blocking,
};

pub type PollDraw = Poll<Result<Vec<RenderedDiac>, Arc<eyre::Report>>, PositStatus>;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Save(Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>),
    MoveDiac(usize, Point),
}

#[derive(Debug, Clone)]
pub(crate) struct RenderedDiac {
    letter: char,
    diac: char,
    position: Result<Point, DiacMiss>,
    img: ImgHandle,
}

impl RenderedDiac {
    pub(crate) fn new(
        letter: char,
        diac: char,
        position: Result<Point, DiacMiss>,
        img: ImgHandle,
    ) -> Self {
        Self { letter, diac, position, img }
    }
}

#[derive(Debug)]
pub struct Drawn {
    diacs: Vec<RenderedDiac>,
    saving: Poll<Result<(), Arc<eyre::ErrReport>>, SaveStatus>,
}

impl Drawn {
    pub fn new(diacs: Vec<RenderedDiac>) -> Self {
        Self { diacs, saving: Poll::Ready(Ok(())) }
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
            Message::MoveDiac(diac_idx, pos) => self.diacs[diac_idx].position = Ok(pos),
        }
        Task::none()
    }

    pub fn toolbar_view<'a>(&self, img_to_save: NamedImg) -> Element<'a, Message> {
        self.saving
            .loading_btn()
            .on_press(Message::Save(Poll::Pending(SaveStatus::Trigger(img_to_save))))
            .into()
    }

    pub fn diac_view<'a>(&self, img: &ImgHandle) -> Element<'a, Message> {
        stage(self.diacs.iter().map(|diac| match &diac.position {
            Ok(pos) => Overlay::image(*pos, diac.img.handle().clone()),
            Err(miss) =>
            {
                #[expect(clippy::cast_precision_loss)]
                Overlay::text(
                    Point::new(img.img().width() as f32 + f32::from(PADDING), miss.top as f32),
                    miss.missing_text.clone(),
                )
            }
        }))
        .handle(img.handle().clone())
        .on_move(Message::MoveDiac)
        .content_fit(ContentFit::None)
        .font(Font::with_name("Guttman Stam"))
        .font_size(48.0)
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
            let Point { x, y } = pos.snap();
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
