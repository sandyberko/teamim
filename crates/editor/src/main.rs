#[allow(unused)]
mod repro;

mod strs;

#[cfg(test)]
mod tests;

use std::{
    borrow::Cow,
    cell::RefCell,
    ffi::CStr,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cosmic::{
    Action, Application, Core, Element, Task,
    app::{self, Settings},
    iced::{
        ContentFit, Length, Padding, Point, Subscription, alignment::Vertical, mouse::Interaction,
    },
    iced_core::image::Bytes,
    iced_futures,
    iced_widget::scrollable::{Direction, Scrollbar},
    task,
    widget::{
        Column, Image, Row, Space,
        button::{self},
        image::Handle,
        mouse_area, popover, scrollable, text,
    },
};
use eyre::{OptionExt as _, WrapErr as _};
use image::{ImageBuffer, ImageFormat, ImageReader, Rgb, Rgba, buffer::ConvertBuffer};
use num_traits::ToPrimitive as _;
use rfd::AsyncFileDialog;

use editor::{spinner::Spinner, stage::Stage};
use teamim::{
    DATAPATH, DiacMiss, DrawProgress, PlaceOptions, TeamimCtx, glyph::YERAH_BEN_YOMO,
    leptonica_ext::PixBox,
};

#[derive(Debug, Clone)]
pub(crate) enum JobState<Ready, Running = ()> {
    /// the has either not yet started or has already finished.
    Ready(Result<Ready, Arc<eyre::Report>>),
    Running(Running),
}

impl<Ready: Default, Running> Default for JobState<Ready, Running> {
    fn default() -> Self {
        JobState::Ready(Ok(Ready::default()))
    }
}

impl<Ready, Running> JobState<Ready, Running> {
    fn ready_ok(&self) -> Option<&Ready> {
        if let JobState::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
    fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        if let JobState::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Tick(Instant),
    SelectImage,
    ImageLoaded(Result<Option<LoadedImage>, Arc<eyre::Report>>),
    Draw(JobState<Drawn, DrawProgress>),
    Save,
    Saved(Result<(), Arc<eyre::Report>>),
    /// enters placing mode with the given miss' diacritic
    PlaceMode(usize),
    /// places discritic at [`Place::position`]
    Place,
    PlaceMove(Point),
}

const PADDING: u16 = 5;
const IMG_EXTS: &[&str] = &["jpg", "jpeg", "png"];

fn with_tctx<T, F>(datapath: &CStr, f: F) -> eyre::Result<T>
where
    F: FnOnce(&mut TeamimCtx) -> T,
{
    thread_local! {
        static TEAMIM_CTX: RefCell<Option<TeamimCtx>> = const { RefCell::new(None) };
    }
    TEAMIM_CTX.with_borrow_mut(|ctx| {
        let ctx = {
            if ctx.is_none() {
                *ctx = Some(TeamimCtx::new(datapath)?);
            }

            ctx.as_mut().unwrap()
        };
        Ok(f(ctx))
    })
}

type ImgState = JobState<Option<LoadedImage>, Spinner>;

struct App {
    core: Core,
    img: ImgState,
}

impl Application for App {
    type Executor = cosmic::executor::multi::Executor;

    type Flags = ImgState;

    type Message = Message;

    const APP_ID: &'static str = "org.teamim.editor";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, img: Self::Flags) -> (Self, app::Task<Message>) {
        let app = App { core, img };
        (app, Task::none())
    }

    fn subscription(&self) -> Subscription<Message> {
        let ticker = cosmic::iced::time::every(Duration::from_millis(16)).map(Message::Tick);
        let loaded = match &self.img {
            JobState::Running(_) => return ticker,
            JobState::Ready(Ok(Some(loaded))) => loaded,
            JobState::Ready(_) => return Subscription::none(),
        };
        let drawn = match &loaded.drawing {
            JobState::Running(_) => return ticker,
            JobState::Ready(Ok(Some(drawn))) => drawn,
            JobState::Ready(_) => return Subscription::none(),
        };
        let () = match &drawn.saving {
            JobState::Running(_) => return ticker,
            JobState::Ready(_) => return Subscription::none(),
        };
    }

    #[expect(clippy::too_many_lines)]
    fn update(&mut self, msg: Message) -> app::Task<Message> {
        match msg {
            Message::Tick(now) => {
                if let JobState::Running(spinner) = &mut self.img {
                    spinner.tick(now);
                }
                app::Task::none()
            }
            Message::SelectImage => {
                if let JobState::Running(_) = self.img {
                    return Task::none();
                }

                self.img = JobState::Running(Spinner::new());
                task::future(async move {
                    let loaded = select_image().await.map_err(Arc::new);
                    Message::ImageLoaded(loaded)
                })
            }
            Message::ImageLoaded(res) => {
                self.img = JobState::Ready(res);
                Task::none()
            }
            Message::Draw(JobState::Running(DrawProgress::Pending)) => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                loaded.drawing = JobState::Running((Spinner::new(), DrawProgress::default()));
                let loaded = loaded.clone();
                task::stream(iced_futures::stream::channel(0, |mut tx| async move {
                    let result = tokio::task::spawn_blocking({
                        let tx = Mutex::new(tx.clone());
                        move || {
                            loaded
                                .draw_teamim(DATAPATH, PlaceOptions::default(), {
                                    move |progress| {
                                        _ = tx
                                            .lock()
                                            .unwrap()
                                            .try_send(JobState::Running(progress));
                                    }
                                })
                                .map_err(Arc::new)
                        }
                    })
                    .await
                    .expect("blocking task to finish");
                    _ = tx.try_send(JobState::Ready(result));
                }))
                .map(Message::Draw)
                .map(Action::App)
            }
            Message::Draw(JobState::Running(progress)) => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };

                loaded.drawing = JobState::Running((Spinner::new(), progress));
                Task::none()
            }
            Message::Draw(JobState::Ready(drawn_res)) => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };

                loaded.drawing = JobState::Ready(drawn_res.map(Some));
                Task::none()
            }
            Message::Save => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                drawn.saving = JobState::Running(Spinner::new());
                let drawn = drawn.clone();
                task::future(async move {
                    let result = drawn.save().await.map_err(Arc::new);
                    Message::Saved(result)
                })
            }
            Message::Saved(saved) => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                drawn.saving = JobState::Ready(saved);
                Task::none()
            }
            Message::PlaceMode(miss_idx) => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };

                let diac = load_image("../../assets/glyphs/yerah_ben_yomo.tif").unwrap().img;
                drawn.place_diac =
                    JobState::Ready(Ok(Some(Place { miss_idx, diac, position: None })));
                Task::none()
            }
            Message::Place => {
                let Some(drawn) = self.drawn() else { return Task::none() };
                let Some(place) = drawn.place_diac.ready_ok().and_then(Option::as_ref) else {
                    return task::none();
                };
                let Some(&position) = place.position.as_ref() else { return task::none() };
                let drawn = drawn.clone();
                let place = place.clone();
                task::future(async move {
                    let res = tokio::task::spawn_blocking(move || {
                        place_diac(&place, position, drawn).map_err(Arc::new)
                    })
                    .await
                    .expect("blocking task to finish");
                    Message::Draw(JobState::Ready(res))
                })
            }
            Message::PlaceMove(point) => {
                let Some(drawn) = self.drawn_mut() else { return Task::none() };
                let Some(place) = drawn.place_diac.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                place.position = Some(point);
                Task::none()
            }
        }
    }
    fn view(&'_ self) -> Element<'_, Message> {
        let img = self.img_view();
        // [TODO] this breaks RTL 😭
        let scroll_dir =
            Direction::Both { vertical: Scrollbar::new(), horizontal: Scrollbar::new() };
        Column::with_children([self.toolbar(), scrollable(img).direction(scroll_dir).into()])
            .spacing(PADDING)
            .padding(PADDING)
            .into()
    }
}

fn place_diac(place: &Place, position: Point, drawn: Drawn) -> eyre::Result<Drawn> {
    let mut data = drawn.img.pixels.to_vec();
    PixBox::from_rgba8_with(
        &mut data,
        drawn.img.width.try_into()?,
        drawn.img.height.try_into()?,
        move |pix| {
            YERAH_BEN_YOMO.pix.with(move |diac| {
                pix.render_img(
                    PixBox::clone(diac),
                    position.x.to_i32().ok_or_eyre("Failed to convert x coordinate to i32")?,
                    position.y.to_i32().ok_or_eyre("Failed to convert y coordinate to i32")?,
                )
            })
        },
    )??;
    let misses = drawn
        .misses
        .into_iter()
        .enumerate()
        .filter_map(|(i, miss)| (i != place.miss_idx).then_some(miss))
        .collect();
    let pixels = Bytes::from(data);
    let handle = Handle::from_rgba(drawn.img.width, drawn.img.height, pixels.clone());
    let img = Img { pixels, handle, ..drawn.img };

    Ok(Drawn {
        img,
        misses,
        saving: JobState::Ready(Ok(())),
        place_diac: JobState::Ready(Ok(None)),
    })
}

impl App {
    fn toolbar(&'_ self) -> Element<'_, Message> {
        Row::with_children([
            // select
            self.loading_btn(strs::SELECT_IMG, &self.img).on_press(Message::SelectImage).into(),
            // draw
            self.img.ready_ok().and_then(Option::as_ref).map_or(
                /* [HACK] */ Space::with_width(0).into(),
                |loaded| {
                    let (label, state) = match loaded.drawing.clone() {
                        JobState::Running((spinner, progress)) => {
                            (strs::draw_progress(progress), JobState::Running(spinner))
                        }
                        JobState::Ready(ready) => (strs::DRAW_TEAMIM, JobState::Ready(ready)),
                    };

                    self.loading_btn(label, &state)
                        .on_press(Message::Draw(JobState::Running(DrawProgress::Pending)))
                        .into()
                },
            ),
            // save
            self.drawn().map_or(/* [HACK] */ Space::with_width(0).into(), |drawn| {
                self.loading_btn(strs::SAVE, &drawn.saving).on_press(Message::Save).into()
            }),
        ])
        .spacing(PADDING)
        .width(Length::Fill)
        .into()
    }

    fn loading_btn<'a, Ready>(
        &'_ self,
        label: impl Into<Cow<'a, str>> + 'a,
        state: &JobState<Ready, Spinner>,
    ) -> button::Button<'a, Message> {
        let theme = self.core.system_theme().cosmic();
        button::custom(
            Row::with_children([
                text(label).into(),
                if let JobState::Running(spinner) = state {
                    Element::from(*spinner)
                } else {
                    // [HACK]
                    Space::with_width(0).into()
                },
            ])
            .padding(Padding::from([0, theme.space_s()]))
            .spacing(theme.space_xxxs())
            .align_y(Vertical::Center),
        )
        .class(button::ButtonClass::Suggested)
    }

    fn img_view(&'_ self) -> Element<'_, Message> {
        let JobState::Ready(ready) = &self.img else {
            return text(strs::LOADING).into();
        };
        let Ok(ready) = ready else {
            // [TODO]
            return text(strs::ERROR).into();
        };
        let Some(loaded) = ready else {
            return text(strs::NO_IMG_SELECTED).into();
        };

        if let Some(drawn) = loaded.drawing.ready_ok().and_then(Option::as_ref) {
            let element = Row::with_children([
                drawn
                    .misses
                    .iter()
                    .enumerate()
                    .map(|(miss_idx, miss)| {
                        #[expect(clippy::cast_precision_loss)]
                        (
                            button::text(miss.missing_text.as_ref())
                                .font_size(48)
                                .on_press(Message::PlaceMode(miss_idx))
                                .into(),
                            Point::new(0.0, miss.top as f32),
                        )
                    })
                    .collect::<Stage<Message>>()
                    .into(),
                Image::new(&drawn.img.handle).content_fit(ContentFit::None).into(),
            ])
            .into();

            if let Some(place) = drawn.place_diac.ready_ok().and_then(Option::as_ref) {
                let element = if let Some(position) = place.position {
                    popover(element)
                        .popup(
                            mouse_area(Image::new(place.diac.handle.clone()))
                                .interaction(Interaction::Crosshair)
                                .on_move(Message::PlaceMove)
                                .on_press(Message::Place),
                        )
                        .position(popover::Position::Point(position))
                        .into()
                } else {
                    element
                };

                mouse_area(element)
                    .on_move(Message::PlaceMove)
                    .on_press(Message::Place)
                    .interaction(Interaction::Crosshair)
                    .into()
            } else {
                element
            }
        } else {
            Image::new(&loaded.img.handle).content_fit(ContentFit::None).into()
        }
    }

    fn drawn(&self) -> Option<&Drawn> {
        self.img.ready_ok().and_then(|loaded| loaded.as_ref()?.drawing.ready_ok()?.as_ref())
    }

    fn drawn_mut(&mut self) -> Option<&mut Drawn> {
        self.img.ready_ok_mut().and_then(|loaded| loaded.as_mut()?.drawing.ready_ok_mut()?.as_mut())
    }
}

async fn select_image() -> eyre::Result<Option<LoadedImage>> {
    // [TODO]
    let Some(picked_file) = AsyncFileDialog::new()
        .set_title(strs::SELECT_IMG)
        .add_filter("image", IMG_EXTS)
        .pick_file()
        .await
    else {
        return Ok(None);
    };

    let loaded = tokio::task::spawn_blocking(move || load_image(picked_file.path()))
        .await
        .expect("blocking task to finish")?;

    Ok(Some(loaded))
}

fn load_image(path: impl AsRef<Path>) -> eyre::Result<LoadedImage> {
    let path = path.as_ref().into();
    let image = ImageReader::open(&path)?.decode()?.into_rgba8();
    let width = image.width();
    let height = image.height();
    let pixels = Bytes::from(image.into_raw());
    let img = Img {
        path,
        width,
        height,
        pixels: pixels.clone(),
        handle: Handle::from_rgba(width, height, pixels),
    };
    Ok(LoadedImage { img, drawing: JobState::Ready(Ok(None)) })
}

#[derive(Debug, Clone)]
struct Img {
    path: Arc<Path>,
    width: u32,
    height: u32,
    pixels: Bytes,
    handle: Handle,
}

#[derive(Debug, Clone)]
struct LoadedImage {
    img: Img,
    drawing: JobState<Option<Drawn>, (Spinner, DrawProgress)>,
}

impl LoadedImage {
    fn draw_teamim(
        self,
        datapath: &'static CStr,
        options: PlaceOptions,
        progress_callback: impl Fn(DrawProgress) + Send + 'static,
    ) -> eyre::Result<Drawn> {
        let mut data = self.img.pixels.to_vec();

        let misses = PixBox::from_rgba8_with(
            &mut data,
            self.img.width.try_into()?,
            self.img.height.try_into()?,
            |img| with_tctx(datapath, |ctx| ctx.place_teamim_pix(img, options, progress_callback)),
        )?
        .unwrap_or_else(|err| Err(err.into()))?;

        let pixels = Bytes::from(data);
        let handle = Handle::from_rgba(self.img.width, self.img.height, pixels.clone());
        let img = Img { pixels, handle, ..self.img };
        Ok(Drawn {
            img,
            misses,
            saving: JobState::Ready(Ok(())),
            place_diac: JobState::Ready(Ok(None)),
        })
    }
}

#[derive(Debug, Clone)]
struct Place {
    miss_idx: usize,
    diac: Img,
    position: Option<Point>,
}

#[derive(Debug, Clone)]
struct Drawn {
    img: Img,
    misses: Vec<DiacMiss>,
    saving: JobState<(), Spinner>,
    place_diac: JobState<Option<Place>, Spinner>,
}

impl Drawn {
    async fn save(self) -> eyre::Result<()> {
        // [TODO]
        let mut dialog = AsyncFileDialog::new().set_title(strs::SAVE).add_filter("image", IMG_EXTS);
        if let Some(dir) = self.img.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file_name) = self.img.path.file_name() {
            dialog = dialog.set_file_name(file_name.to_str().ok_or_eyre("שם הקובץ לא תקין")?);
        }
        let file = dialog.save_file().await.ok_or_eyre("שמירה בוטלה")?;
        let img =
            ImageBuffer::<Rgba<u8>, _>::from_raw(self.img.width, self.img.height, self.img.pixels)
                .expect("`data` to be big enough for width * height");
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
}

fn main() -> eyre::Result<()> {
    cosmic::app::run::<App>(Settings::default(), JobState::default())?;
    Ok(())
}
