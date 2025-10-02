use std::{convert::identity, sync::Arc};

use editor::spinner::Spinner;
use iced::{
    advanced,
    alignment::Vertical,
    widget::{Button, button, row, text, text::IntoFragment},
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
                Some(text(label).into()),
                if let JobState::Running(spinner) = self { Some(spinner.view()) } else { None },
            ]
            .into_iter()
            .filter_map(identity))
            .spacing(6)
            .align_y(Vertical::Center),
        )
    }
}
