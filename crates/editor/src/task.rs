use std::iter::once;

use editor::spinner::Spinner;
use iced::{
    alignment::Vertical,
    theme::Base,
    widget::{Button, button, row, text, text::IntoFragment},
};

pub(crate) type TryPoll<Ready, Pending = ()> = Poll<Result<Ready, eyre::Report>, Pending>;

#[derive(Debug, Clone)]
pub(crate) enum Poll<Ready, Pending = ()> {
    Ready(Ready),
    Pending(Pending),
}

impl<Ready, Pending> Poll<Ready, Pending> {
    pub fn as_pending(&self) -> Option<&Pending> {
        if let Self::Pending(pending) = self { Some(pending) } else { None }
    }

    pub fn as_mut_pending(&mut self) -> Option<&mut Pending> {
        if let Self::Pending(pending) = self { Some(pending) } else { None }
    }

    pub(crate) fn as_ready(&self) -> Option<&Ready> {
        if let Self::Ready(v) = self { Some(v) } else { None }
    }
}

impl<Ready: Default, Pending> Default for Poll<Ready, Pending> {
    fn default() -> Self {
        Poll::Ready(Ready::default())
    }
}

impl<Ready, Err, Pending> Poll<Result<Ready, Err>, Pending> {
    pub(crate) fn as_ready_ok(&self) -> Option<&Ready> {
        if let Poll::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
    pub(crate) fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        if let Poll::Ready(Ok(ready)) = self { Some(ready) } else { None }
    }
}

impl<Ready, Status> Poll<Ready, Status> {
    pub(crate) fn loading_btn<'a, Message>(&'_ self) -> Button<'a, Message>
    where
        for<'s> &'s Self: IntoFragment<'a>,
        Message: Clone + 'a,
    {
        button(
            row(once(text(self).into()).chain(self.as_pending().map(|_| Spinner::new().into())))
                .spacing(6)
                .align_y(Vertical::Center),
        )
    }
}
