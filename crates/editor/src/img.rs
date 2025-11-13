use iced::{
    Element,
    advanced::image::Bytes,
    widget::{self, image::Handle},
};
use image::{ImageBuffer, Rgba, RgbaImage, imageops::fast_blur};

#[derive(Debug, Clone)]
pub(crate) struct ImgHandle {
    img: ImageBuffer<Rgba<u8>, Bytes>,
    handle: Handle,
}

impl ImgHandle {
    pub(crate) fn img(&self) -> ImageBuffer<Rgba<u8>, Bytes> {
        self.img.clone()
    }

    pub(crate) fn handle(&self) -> &Handle {
        &self.handle
    }

    pub(crate) fn blur(&self, simga: f32) -> Self {
        let (width, height) = self.img.dimensions();
        // [TODO] don't clone
        let img = RgbaImage::from_vec(width, height, self.img.to_vec()).unwrap();
        fast_blur(&img, simga).into()
    }

    pub(crate) fn view<'a, Message>(&self) -> Element<'a, Message> {
        let (width, height) = self.img.dimensions();
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation
        )]
        widget::Image::new(&self.handle)
            .width((width as f32) as u32)
            .height((height as f32) as u32)
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
