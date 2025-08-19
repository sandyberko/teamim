use xilem::{
    WidgetView,
    masonry::properties::types::AsUnit,
    view::{Label, MainAxisAlignment, button, flex, flex_row, sized_box},
};

use crate::{FONT_SIZE, err_prose};

fn spinner<S: 'static, A: 'static>() -> impl WidgetView<S, A> + use<S, A> {
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

    pub(crate) fn ready_ok(&self) -> Option<&T> {
        match self {
            Self::Ready(Ok(ok)) => Some(ok),
            _ => None,
        }
    }
}

pub(crate) fn future_btn<Pending, Ready, State, Action, PedningTag, ReadyTag, F>(
    state: &Future<Pending, Ready>,
    pending_tag: PedningTag,
    ready_tag: ReadyTag,
    callback: F,
) -> impl WidgetView<State, Action> + use<Pending, Ready, State, Action, PedningTag, ReadyTag, F>
where
    State: 'static,
    Action: 'static,
    PedningTag: Into<Label>,
    ReadyTag: Into<Label>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    match state {
        Future::Ready(status) => {
            flex((button(ready_tag, callback), status.as_ref().err().map(|err| err_prose(err))))
                .boxed()
        }
        // TODO: use exact button layout
        Future::Pending(_) => sized_box(
            flex_row((spinner(), pending_tag.into()))
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .must_fill_major_axis(false),
        )
        .height((FONT_SIZE.get() * 2.0).px())
        .boxed(),
    }
}
