use iced::{
    Element,
    advanced::image::Bytes,
    widget::{self, image::Handle},
};
use image::{ImageBuffer, Rgba, RgbaImage, imageops::fast_blur};
use num_traits::AsPrimitive;

#[derive(Debug, Clone)]
pub(crate) struct ImgHandle {
    img: ImageBuffer<Rgba<u8>, Bytes>,
    handle: Handle,
}

impl ImgHandle {
    pub(crate) fn img(&self) -> ImageBuffer<Rgba<u8>, Bytes> {
        self.img.clone()
    }

    pub(crate) fn blur(&self) -> Self {
        let (width, height) = self.img.dimensions();
        // [TODO] don't clone
        let img = RgbaImage::from_vec(width, height, self.img.to_vec()).unwrap();
        fast_blur(&img, 17.0).into()
    }

    pub(crate) fn view<'a, Message>(&self, zoom: f32) -> Element<'a, Message> {
        widget::Image::new(&self.handle)
            .height(AsPrimitive::<f32>::as_(self.img.height()) * zoom)
            .width(AsPrimitive::<f32>::as_(self.img.width()) * zoom)
            .into()
    }
}

impl From<RgbaImage> for ImgHandle {
    fn from(buf: RgbaImage) -> Self {
        let (width, height) = buf.dimensions();
        let buf = Bytes::from_owner(buf.into_raw());
        let img = ImageBuffer::from_raw(width, height, Bytes::clone(&buf)).unwrap();
        let handle = Handle::from_rgba(width, height, buf);
        Self { img, handle }
    }
}
