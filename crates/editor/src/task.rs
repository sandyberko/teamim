use std::{convert::identity, sync::Arc};

use editor::spinner::Spinner;
use iced::{
    advanced,
    alignment::Vertical,
    widget::{Button, button, row, text, text::IntoFragment},
};

#[derive(Debug, Clone)]
pub(crate) enum Poll<Ready, Pending = ()> {
    Ready(Result<Ready, Arc<eyre::Report>>),
    Pending(Pending),
}

impl<Ready: Default, Pending> Default for Poll<Ready, Pending> {
    fn default() -> Self {
        Poll::Ready(Ok(Ready::default()))
    }
}

impl<Ready, Pending> Poll<Ready, Pending> {
    pub(crate) fn ready_ok(&self) -> Option<&Ready> {
        if let Poll::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
    pub(crate) fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        if let Poll::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
}

impl<Ready> Poll<Ready, Spinner> {
    pub(crate) fn loading_btn<'a, Message, Theme, Renderer>(
        &'_ self,
        label: impl IntoFragment<'a>,
    ) -> Button<'a, Message, Theme, Renderer>
    where
        Message: Clone + 'a,
        Renderer:
            advanced::Renderer + advanced::text::Renderer + iced_renderer::geometry::Renderer + 'a,
        Theme: button::Catalog + text::Catalog + 'a,
    {
        button(
            row([
                Some(text(label).into()),
                if let Poll::Pending(spinner) = self { Some(spinner.view()) } else { None },
            ]
            .into_iter()
            .filter_map(identity))
            .spacing(6)
            .align_y(Vertical::Center),
        )
    }
}
