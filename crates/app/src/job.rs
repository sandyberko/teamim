use std::fmt::Debug;
use xilem::{
    TextAlign, WidgetView,
    masonry::properties::types::AsUnit,
    style::Style,
    view::{Label, MainAxisAlignment, button, flex_row, label, portal, sized_box},
};

use crate::{FONT_SIZE, RED};

fn spinner<S: 'static, A: 'static>() -> impl WidgetView<S, A> + use<S, A> {
    sized_box(xilem::view::spinner()).height(FONT_SIZE).width(FONT_SIZE)
}

#[derive(Debug)]
pub(crate) enum JobState<Ready, Running = ()> {
    /// the has either not yet started or has alreadyfinished.
    Ready(eyre::Result<Ready>),
    Running(Running),
}

impl<Ready, Running> JobState<Ready, Running> {
    pub(crate) fn ready_ok(&self) -> Option<&Ready> {
        match self {
            Self::Ready(Ok(ok)) => Some(ok),
            _ => None,
        }
    }

    pub(crate) fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        match self {
            Self::Ready(Ok(ok)) => Some(ok),
            _ => None,
        }
    }
}

pub(crate) fn job_btn<Pending, Ready, State, Action, PedningTag, ReadyTag, F>(
    state: &JobState<Pending, Ready>,
    pending_tag: PedningTag,
    ready_tag: ReadyTag,
    callback: F,
) -> impl WidgetView<State, Action> + use<Pending, Ready, State, Action, PedningTag, ReadyTag, F>
where
    State: Send + Sync + 'static,
    Action: Send + Sync + 'static,
    PedningTag: Into<Label>,
    ReadyTag: Into<Label>,
    F: Fn(&mut State) -> Action + Send + Sync + 'static,
{
    match state {
        JobState::Ready(status) => flex_row((
            status.as_ref().err().map(|err| {
                (
                    sized_box(portal(
                        label(err.to_string()).color(RED).text_alignment(TextAlign::Right),
                    ))
                    .width((FONT_SIZE.get() * 10.0).px())
                    .height((FONT_SIZE.get() * 2.0).px()),
                    label("⚠️"),
                )
            }),
            button(ready_tag, callback),
        ))
        .boxed(),
        // TODO: use exact button layout
        JobState::Running(_) => sized_box(
            flex_row((spinner(), pending_tag.into()))
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .must_fill_major_axis(false),
        )
        .height((FONT_SIZE.get() * 2.0).px())
        .boxed(),
    }
}
