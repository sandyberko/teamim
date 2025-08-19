use xilem::{
    WidgetView,
    view::{Label, MainAxisAlignment, button, flex, flex_row, sized_box},
};

use crate::{FONT_SIZE, err_prose};

pub(crate) fn spinner<S: 'static, A: 'static>() -> impl WidgetView<S, A> + use<S, A> {
    sized_box(xilem::view::spinner()).height(FONT_SIZE).width(FONT_SIZE)
}

#[derive(Debug)]
pub(crate) enum Future<Pending, T> {
    Pending(Pending),
    Ready(eyre::Result<T>),
}

impl<Pending, T> Future<Pending, T> {
    pub(crate) fn map<U, F>(self, f: F) -> Future<Pending, U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Ready(result) => Future::Ready(result.map(f)),
            Self::Pending(pending) => Future::Pending(pending),
        }
    }
}

pub(crate) fn future_btn<Pending, Ready, State: 'static, Action: 'static>(
    state: &Future<Pending, Ready>,
    pending_tag: impl Into<Label>,
    ready_tag: impl Into<Label>,
    callback: impl Fn(&mut State) -> Action + Send + Sync + 'static,
) -> impl WidgetView<State, Action> {
    match state {
        Future::Ready(status) => {
            let btn = button(ready_tag, callback);
            match status {
                Ok(_) => btn.boxed(),
                Err(err) => flex((btn, err_prose(err))).boxed(),
            }
        }
        Future::Pending(_) => flex_row((spinner(), pending_tag.into()))
            .main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .must_fill_major_axis(false)
            .boxed(),
    }
}
