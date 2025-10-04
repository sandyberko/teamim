#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod diac_renderer;
mod loaded;
mod strs;
mod task;

#[cfg(test)]
mod tests;

use eyre::{OptionExt as _, WrapErr as _, bail, eyre};
use iced::{
    Element, Length, Point, Subscription, Task, Vector,
    advanced::image::Bytes,
    keyboard::{Key, key::Named, on_key_press},
    mouse::Interaction,
    widget::{
        self, Column, button,
        image::Handle,
        mouse_area,
        scrollable::{AbsoluteOffset, Direction, Scrollbar, Viewport},
        text,
    },
};
use image::{
    ImageBuffer, ImageFormat, ImageReader, Rgb, Rgba, RgbaImage, buffer::ConvertBuffer,
    imageops::fast_blur,
};
use num_traits::{AsPrimitive, ToPrimitive as _};
use rfd::AsyncFileDialog;
use std::{
    cell::RefCell,
    convert::identity,
    ffi::CStr,
    mem,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use editor::{spinner::Spinner, stage};
use teamim::{DiacMiss, DiacPos, DrawProgress, TeamimCtx, leptonica_ext::PixBox};

use crate::{
    loaded::LoadedImage,
    task::{Poll, TryPoll},
};

#[derive(Debug, Clone)]
enum Message {
    Tick(Instant),
    SelectImage,
    ImageLoaded(Arc<Mutex<eyre::Result<Option<loaded::LoadedImage>>>>),
    Loaded(loaded::Message),
    Scroll(Viewport),
}

impl From<loaded::Message> for Message {
    fn from(value: loaded::Message) -> Self {
        Message::Loaded(value)
    }
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

type ImgState = TryPoll<Option<loaded::LoadedImage>, Spinner>;
const FONT_SIZE: f32 = 72.0;

struct App {
    img: ImgState,
    scroll_offset: AbsoluteOffset,
}

impl Default for App {
    fn default() -> Self {
        Self { img: Poll::Ready(Ok(None)), scroll_offset: Default::default() }
    }
}

impl App {
    fn ticker_sub(&self) -> Subscription<Message> {
        let ticker = iced::time::every(Duration::from_millis(16)).map(Message::Tick);
        let loaded = match &self.img {
            TryPoll::Pending(_) => return ticker,
            TryPoll::Ready(Ok(Some(loaded))) => loaded,
            TryPoll::Ready(_) => return Subscription::none(),
        };
        let drawn = match &loaded.drawing {
            TryPoll::Pending(_) => return ticker,
            TryPoll::Ready(Ok(Some(drawn))) => drawn,
            TryPoll::Ready(_) => return Subscription::none(),
        };
        let () = match &drawn.saving {
            Poll::Pending(_) => return ticker,
            Poll::Ready(_) => return Subscription::none(),
        };
    }
    fn toolbar<'a>(&'_ self) -> Element<'a, Message> {
        let children = [
            // select
            Some(self.img.loading_btn(strs::SELECT_IMG).on_press(Message::SelectImage).into()),
            // draw
            self.img
                .ready_ok()
                .and_then(Option::as_ref)
                .map(|loaded| loaded.draw_tools().map(Message::Loaded)),
            // save
            self.drawn().map(|drawn| {
                drawn.saving.loading_btn(strs::SAVE).on_press(loaded::Message::Save.into()).into()
            }),
            // [DEBUG]
            self.drawn()
                .and_then(|drawn| drawn.place_diac.ready_ok()?.as_ref()?.position)
                .map(|pos| widget::text(format!("{pos}")).into()),
        ];

        widget::row(children.into_iter().filter_map(identity))
            .spacing(PADDING as u32)
            .width(Length::Fill)
            .into()
    }

    fn content_view<'a>(&'a self) -> Element<'a, Message> {
        let TryPoll::Ready(ready) = &self.img else {
            return widget::text(strs::LOADING).into();
        };
        let Ok(ready) = ready else {
            // [TODO]
            return widget::text(strs::ERROR).into();
        };
        let Some(loaded) = ready else {
            return widget::text(strs::NO_IMG_SELECTED).into();
        };

        if let Some(drawn) = loaded.drawing.ready_ok().and_then(Option::as_ref) {
            drawn_content_view(drawn, loaded.zoom, self.scroll_offset)
        } else {
            loaded.img.view(loaded.zoom)
        }
    }

    fn drawn(&self) -> Option<&Drawn> {
        self.img.ready_ok().and_then(|loaded| loaded.as_ref()?.drawing.ready_ok()?.as_ref())
    }

    fn view<'a>(&'a self) -> Element<'a, Message> {
        let img = self.content_view();
        let scroll_dir =
            Direction::Both { vertical: Scrollbar::new(), horizontal: Scrollbar::new() };
        Column::with_children([
            self.toolbar(),
            widget::scrollable(img).on_scroll(Message::Scroll).direction(scroll_dir).into(),
        ])
        .spacing(PADDING as u32)
        .padding(PADDING)
        .into()
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Tick(now) => {
                if let TryPoll::Pending(spinner) = &mut self.img {
                    spinner.tick(now);
                }
                Task::none()
            }
            Message::SelectImage => {
                if let TryPoll::Pending(_) = self.img {
                    return Task::none();
                }

                self.img = TryPoll::Pending(Spinner::new());
                Task::future(async move {
                    let loaded = select_image().await;
                    Message::ImageLoaded(Arc::new(Mutex::new(loaded)))
                })
            }
            Message::ImageLoaded(res) => {
                let mut res = res.lock().unwrap();
                let res = mem::replace(&mut *res, Err(eyre!("message taken")));
                self.img = Poll::Ready(res);
                Task::none()
            }
            Message::Loaded(msg) => {
                let Some(loaded) = self.img.ready_ok_mut().and_then(Option::as_mut) else {
                    return Task::none();
                };
                loaded.update(msg).map(Message::Loaded)
            }
            Message::Scroll(viewport) => {
                self.scroll_offset = viewport.absolute_offset();
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.ticker_sub(),
            self.drawn().and_then(|drawn| drawn.place_diac.ready_ok()).map_or(
                Subscription::none(),
                |_| {
                    on_key_press(|key, _| {
                        (key == Key::Named(Named::Escape))
                            .then_some(loaded::Message::PlaceCancel.into())
                    })
                },
            ),
        ])
    }
}

fn drawn_content_view<'a>(
    drawn: &'a Drawn,
    zoom: f32,
    scroll_offset: AbsoluteOffset,
) -> Element<'a, Message> {
    let place = drawn.place_diac.ready_ok().and_then(Option::as_ref);
    widget::row([
        // misses
        stage(drawn.misses.iter().enumerate().map(|(miss_idx, miss)| {
            (
                button(text(miss.missing_text.as_ref()).size(48.0 * zoom))
                    .on_press_maybe(
                        if let Some(place) = drawn.place_diac.ready_ok().and_then(Option::as_ref)
                            && place.miss_idx == miss_idx
                        {
                            None
                        } else {
                            Some(loaded::Message::PlaceMode(miss_idx).into())
                        },
                    )
                    .into(),
                Point::new(0.0, AsPrimitive::<f32>::as_(miss.top) * zoom),
            )
        }))
        .into(),
        // image
        mouse_area(stage(
            [
                Some((drawn.img.view(zoom), Point::ORIGIN)),
                drawn.place_diac.ready_ok().and_then(Option::as_ref).and_then(|place| {
                    let position = place.position?;
                    Some((
                        // [TODO] color(black)
                        mouse_area(text(place.diac.as_ref()).size(FONT_SIZE))
                            .interaction(Interaction::Crosshair)
                            .on_move(move |offset| {
                                loaded::Message::PlaceMove(
                                    position + Vector::new(offset.x, offset.y),
                                )
                                .into()
                            })
                            .on_press(loaded::Message::Place { scroll_offset }.into())
                            .into(),
                        position,
                    ))
                }),
            ]
            .into_iter()
            .filter_map(identity),
        ))
        .on_move(|pos| loaded::Message::PlaceMove(pos).into())
        .on_press(loaded::Message::Place { scroll_offset }.into())
        .interaction(if place.is_some() { Interaction::Crosshair } else { Interaction::default() })
        .into(),
    ])
    .into()
}

async fn select_image() -> eyre::Result<Option<loaded::LoadedImage>> {
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

fn load_image(path: impl AsRef<Path>) -> eyre::Result<loaded::LoadedImage> {
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
    Ok(loaded::LoadedImage::new(img))
}

#[derive(Debug, Clone)]
struct Img {
    path: Arc<Path>,
    width: u32,
    height: u32,
    pixels: Bytes,
    handle: Handle,
}

impl Img {
    fn view<'a>(&'_ self, zoom: f32) -> Element<'a, Message> {
        widget::Image::new(&self.handle)
            .height(AsPrimitive::<f32>::as_(self.height) * zoom)
            .width(AsPrimitive::<f32>::as_(self.width) * zoom)
            .into()
    }

    fn blur(&self) -> Self {
        // [TODO] reduce clones
        let buf =
            RgbaImage::from_raw(self.width, self.height, self.pixels.to_vec()).expect("valid Img");
        let pixels = Bytes::from_owner(fast_blur(&buf, 17.0).into_raw());
        let handle = Handle::from_rgba(self.width, self.height, pixels.clone());
        Img { pixels, handle, ..self.clone() }
    }

    async fn save(self) -> eyre::Result<()> {
        // [TODO]
        let mut dialog = AsyncFileDialog::new().set_title(strs::SAVE).add_filter("image", IMG_EXTS);
        if let Some(dir) = self.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file_name) = self.path.file_name() {
            dialog = dialog.set_file_name(file_name.to_str().ok_or_eyre("שם הקובץ לא תקין")?);
        }
        let file = dialog.save_file().await.ok_or_eyre("שמירה בוטלה")?;
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, self.pixels)
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

    // [TODO] reduce clones
    pub(crate) fn draw_teamim(
        self,
        datapath: &'static CStr,
        progress_callback: impl Fn(DrawProgress) + Send + 'static,
    ) -> eyre::Result<Drawn> {
        let Img { width, height, pixels, .. } = self;
        let mut buf = pixels.to_vec();

        let (positions, misses) =
            PixBox::from_rgba8_with(&mut buf, width.try_into()?, height.try_into()?, |img| {
                with_tctx(datapath, |ctx| ctx.positions(img, progress_callback))
            })?
            .unwrap_or_else(|err| Err(err.into()))?;

        let mut img = ImageBuffer::from_raw(width, height, buf).ok_or_eyre("expected valid img")?;
        // [TODO] cache, parellelize
        for pos in &positions {
            let mut buf = [0; 4];
            let text = pos.letter.encode_utf8(&mut buf);
            diac_renderer::draw_text(&mut img, [pos.rect.left, pos.rect.top], text, FONT_SIZE);
        }

        let pixels = Bytes::from(img.into_raw());
        let handle = Handle::from_rgba(width, height, pixels.clone());
        let img = Img { pixels, handle, ..self };
        Ok(Drawn::new(img, positions, misses))
    }
}

#[derive(Debug, Clone)]
struct Place {
    miss_idx: usize,
    diac: Arc<str>,
    position: Option<Point>,
}

#[derive(Debug)]
struct Drawn {
    img: Img,
    positions: Vec<DiacPos>,
    misses: Vec<DiacMiss>,
    saving: Poll<Result<(), Arc<eyre::ErrReport>>, Spinner>,
    place_diac: TryPoll<Option<Place>, Spinner>,
}

impl Drawn {
    fn new(img: Img, positions: Vec<DiacPos>, misses: Vec<DiacMiss>) -> Self {
        Self {
            img,
            positions,
            misses,
            saving: Poll::Ready(Ok(())),
            place_diac: Poll::Ready(Ok(None)),
        }
    }

    fn place_diac(&mut self, scroll_offset: AbsoluteOffset, zoom: f32) -> eyre::Result<()> {
        let Some(place) = self.place_diac.ready_ok_mut().and_then(Option::as_mut) else {
            bail!("stale")
        };
        let Some(&position) = place.position.as_ref() else { bail!("stale") };
        let x = ((position.x + scroll_offset.x) / zoom)
            .to_i32()
            .ok_or_eyre("Failed to convert x coordinate to i32")?;
        let y = ((position.y + scroll_offset.y) / zoom)
            .to_i32()
            .ok_or_eyre("Failed to convert y coordinate to i32")?;

        // [TODO]
        let mut pixels = self.img.pixels.to_vec();

        let mut img =
            ImageBuffer::<Rgba<u8>, _>::from_raw(self.img.width, self.img.height, pixels.as_mut())
                .ok_or_eyre("failed to convert to image")?;

        let mut buf = [0; 4];
        let text = self.misses[place.miss_idx].diacritic.encode_utf8(&mut buf);
        diac_renderer::draw_text(&mut img, [x, y], text, FONT_SIZE);

        let pixels = Bytes::from(pixels);
        let handle = Handle::from_rgba(self.img.width, self.img.height, pixels.clone());
        self.img = Img { pixels, handle, ..self.img.clone() };
        self.misses.remove(place.miss_idx);
        Ok(())
    }
}

fn view(state: &'_ App) -> Element<'_, Message> {
    App::view(state)
}
fn main() -> eyre::Result<()> {
    iced::application(App::default, App::update, view)
        .subscription(App::subscription)
        .title(strs::TITLE)
        .run()?;
    Ok(())
}
