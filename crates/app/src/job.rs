use std::fmt::Debug;
use xilem::{
    TextAlign, ViewCtx, WidgetView,
    core::{MessageProxy, NoElement, View},
    masonry::properties::types::AsUnit,
    style::Style,
    tokio::sync::mpsc::UnboundedSender,
    view::{Label, MainAxisAlignment, button, flex_row, label, portal, sized_box, worker},
};

use crate::{FONT_SIZE, RED};

fn spinner<S: 'static, A: 'static>() -> impl WidgetView<S, A> + use<S, A> {
    sized_box(xilem::view::spinner()).height(FONT_SIZE).width(FONT_SIZE)
}

#[derive(Debug)]
/// - `TriggMsg` payload required to start the job.
/// - `Ready` - the job either hasn't started yet or has finished successfully.
pub(crate) struct FutureSender<StartMsg, Ready> {
    pub(crate) sender: Option<UnboundedSender<StartMsg>>,
    pub(crate) state: Job<Ready>,
}

impl<TriggMsg, Ready> FutureSender<TriggMsg, Ready> {
    pub(crate) fn new(ready: Ready) -> Self {
        Self { sender: None, state: Job::Ready(Ok(ready)) }
    }

    /// TODO `View::Action = Option<Ok>` but it should be just `Ok`
    /// - `Ok` is returned by the callback and recieved by `on_response`.
    pub(crate) fn worker<ReadyMsg>(
        callback: impl Fn(&TriggMsg) -> eyre::Result<ReadyMsg> + Copy + Send + Sync + 'static,
        on_response: impl Fn(&ReadyMsg) -> Ready + 'static,
    ) -> impl View<Self, Option<ReadyMsg>, ViewCtx, Element = NoElement>
    where
        TriggMsg: Debug + Send + 'static,
        ReadyMsg: Debug + Send + 'static,
    {
        worker::<_, _, Job<ReadyMsg>, _, TriggMsg, Self, _, _>(
            move |proxy: MessageProxy<Job<ReadyMsg>>, mut recv| async move {
                while let Some(ready) = recv.recv().await {
                    proxy.message(Job::Running(())).ok();
                    let result = callback(&ready);
                    proxy.message(Job::Ready(result)).ok();
                }
            },
            |fut: &mut Self, sender: UnboundedSender<TriggMsg>| fut.sender = Some(sender),
            move |fut: &mut Self, resp: Job<ReadyMsg>| match resp {
                Job::Ready(Ok(ok)) => {
                    fut.state = Job::Ready(Ok(on_response(&ok)));
                    Some(ok)
                }
                resp => {
                    fut.state = resp.map(|_| unreachable!());
                    None
                }
            },
        )
    }
}

#[derive(Debug)]
pub(crate) enum Job<Ready, Running = ()> {
    /// the has either not yet started or has alreadyfinished.
    Ready(eyre::Result<Ready>),
    Running(Running),
}

impl<Ready, Running> Job<Ready, Running> {
    /// Map the `Ready::Ok` variant.
    pub(crate) fn map<U, F>(self, f: F) -> Job<U, Running>
    where
        F: FnOnce(Ready) -> U,
    {
        match self {
            Self::Ready(result) => Job::Ready(result.map(f)),
            Self::Running(pending) => Job::Running(pending),
        }
    }

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

pub(crate) fn future_btn<Pending, Ready, State, Action, PedningTag, ReadyTag, F>(
    state: &Job<Pending, Ready>,
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
        Job::Ready(status) => flex_row((
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
        Job::Running(_) => sized_box(
            flex_row((spinner(), pending_tag.into()))
                .main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .must_fill_major_axis(false),
        )
        .height((FONT_SIZE.get() * 2.0).px())
        .boxed(),
    }
}
