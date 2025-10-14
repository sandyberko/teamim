use iced::{
    Element,
    advanced::image::Bytes,
    widget::{self, image::Handle},
};
use image::{RgbaImage, imageops::fast_blur};
use num_traits::AsPrimitive;

#[derive(Debug, Clone)]
pub(crate) struct Img {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Bytes,
}

impl Img {
    pub(crate) fn to_rgba(&self) -> RgbaImage {
        RgbaImage::from_raw(self.width, self.height, self.pixels.to_vec())
            .expect("`data` to be big enough for width * height")
    }
    pub(crate) fn from_rgba(buf: RgbaImage) -> Self {
        let (width, height) = buf.dimensions();
        let pixels = Bytes::from_owner(buf.into_raw());
        Self { width, height, pixels }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ImgHandle {
    img: Img,
    handle: Handle,
}

impl ImgHandle {
    pub(crate) fn img(&self) -> Img {
        self.img.clone()
    }

    pub(crate) fn blur(&self) -> Self {
        fast_blur(&self.img.to_rgba(), 17.0).into()
    }

    pub(crate) fn view<'a, Message>(&self, zoom: f32) -> Element<'a, Message> {
        widget::Image::new(&self.handle)
            .height(AsPrimitive::<f32>::as_(self.img.height) * zoom)
            .width(AsPrimitive::<f32>::as_(self.img.width) * zoom)
            .into()
    }
}

impl From<RgbaImage> for ImgHandle {
    fn from(buf: RgbaImage) -> Self {
        let img = Img::from_rgba(buf);
        let handle = Handle::from_rgba(img.width, img.height, Bytes::clone(&img.pixels));
        Self { img, handle }
    }
}
