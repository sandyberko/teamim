use std::{borrow::Cow, sync::Arc};

use editor::spinner::Spinner;
use iced::{
    Padding, advanced,
    alignment::Vertical,
    core::text::IntoFragment,
    widget::{Button, Space, button, row, text},
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
    pub(crate) fn ready_ok(&self) -> Option<&Ready> {
        if let JobState::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
    pub(crate) fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        if let JobState::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
}

impl<Ready> JobState<Ready, Spinner> {
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
                text(label).into(),
                if let JobState::Running(spinner) = self {
                    spinner.view()
                } else {
                    // [HACK]
                    Space::with_width(0).into()
                },
            ])
            .spacing(6)
            .align_y(Vertical::Center),
        )
    }
}
