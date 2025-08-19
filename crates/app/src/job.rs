use std::fmt::Debug;
use tracing::{Instrument, Level, debug, span};
use xilem::{
    TextAlign, ViewCtx, WidgetView,
    core::{MessageProxy, MessageResult, NoElement, View},
    masonry::properties::types::AsUnit,
    style::Style,
    tokio::sync::mpsc::UnboundedSender,
    view::{Label, MainAxisAlignment, button, flex_row, label, portal, sized_box, worker_raw},
};

use crate::{FONT_SIZE, RED, view_ext::ViewExt};

fn spinner<S: 'static, A: 'static>() -> impl WidgetView<S, A> + use<S, A> {
    sized_box(xilem::view::spinner()).height(FONT_SIZE).width(FONT_SIZE)
}

#[derive(Debug)]
/// - `TriggMsg` payload required to start the job.
/// - `Ready` - the job either hasn't started yet or has finished successfully.
pub(crate) struct Job<StartMsg, Ready> {
    sender: Option<UnboundedSender<StartMsg>>,
    state: JobState<Ready>,
    dbg_label: &'static str,
}

impl<StartMsg, Ready> Job<StartMsg, Ready> {
    pub(crate) fn new(ready: Ready) -> Self {
        Self { sender: None, state: JobState::Ready(Ok(ready)), dbg_label: "<job>" }
    }
    pub(crate) fn with_dbg_label(self, label: &'static str) -> Self {
        Self { dbg_label: label, ..self }
    }

    /// # Panics
    /// [`Self::worker`]'s view hasn't been registered yet.
    #[tracing::instrument(skip_all, fields(label = self.dbg_label))]
    pub(crate) fn start(&mut self, msg: StartMsg) {
        debug!("sending start message");
        self.sender.as_mut().expect("worker to be registered").send(msg).ok();
    }

    pub(crate) fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        self.state.ready_ok_mut()
    }

    pub(crate) fn handle_action(&mut self, job_state: JobState<Ready>) -> MessageResult<()> {
        self.state = job_state;
        MessageResult::Action(())
    }

    #[tracing::instrument(skip_all, fields(label = self.dbg_label))]
    pub(crate) fn button<State, Action, F, PedningTag, ReadyTag>(
        &self,
        pending_tag: PedningTag,
        ready_tag: ReadyTag,
        callback: F,
    ) -> impl WidgetView<State, Action> + use<State, Action, F, StartMsg, Ready, PedningTag, ReadyTag>
    where
        State: Send + Sync + 'static,
        Action: Send + Sync + 'static,
        Ready: Send + Sync + 'static,
        PedningTag: Into<Label>,
        ReadyTag: Into<Label>,
        F: Fn(&mut State) -> Action + Send + Sync + 'static,
    {
        debug!("button render: {:?}", self.state.debug_tag());
        job_btn(&self.state, pending_tag, ready_tag, callback)
    }

    #[tracing::instrument(skip_all, fields(label = self.dbg_label))]
    pub(crate) fn worker<F>(
        &self,
        callback: F,
    ) -> impl View<Self, JobState<Ready>, ViewCtx, Element = NoElement> + use<StartMsg, Ready, F>
    where
        StartMsg: Debug + Send + 'static,
        Ready: Debug + Send + 'static,
        F: Fn(&StartMsg) -> eyre::Result<Ready> + Clone + Send + Sync + 'static,
    {
        let label = self.dbg_label;
        worker_raw::<_, _, _, _, _, Option<UnboundedSender<StartMsg>>, _, _>(
            move |proxy: MessageProxy<JobState<Ready>>, mut recv| {
                let callback = callback.clone();
                async move {
                    while let Some(ready) = recv.recv().await {
                        debug!("start message received");
                        proxy.message(JobState::Running(())).ok();
                        let result = callback(&ready);
                        proxy.message(JobState::Ready(result)).ok();
                    }
                }
                .instrument(span!(Level::DEBUG, "app::job::worker", label))
            },
            |state, sender: UnboundedSender<StartMsg>| {
                *state = Some(sender);
            },
            |_, resp: JobState<Ready>| resp,
        )
        .map_state(|job: &mut Self| &mut job.sender)
    }
}

#[derive(Debug)]
pub(crate) enum JobState<Ready, Running = ()> {
    /// the has either not yet started or has alreadyfinished.
    Ready(eyre::Result<Ready>),
    Running(Running),
}

impl<Ready, Running> JobState<Ready, Running> {
    pub(crate) fn ready_ok_mut(&mut self) -> Option<&mut Ready> {
        match self {
            Self::Ready(Ok(ok)) => Some(ok),
            _ => None,
        }
    }

    fn debug_tag(&self) -> impl Debug {
        use std::fmt::{self, Debug, Formatter};
        struct TagDebug<'a, Ready, Running>(&'a JobState<Ready, Running>);
        impl<Ready, Running> Debug for TagDebug<'_, Ready, Running> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                match self.0 {
                    JobState::Ready(Ok(_)) => write!(f, "Ready(Ok(...))"),
                    JobState::Ready(Err(err)) => {
                        write!(f, "Ready(Err(")?;
                        write!(f, "{err:?}")?;
                        write!(f, "))")
                    }
                    JobState::Running(_) => write!(f, "Running(...)"),
                }
            }
        }
        TagDebug(self)
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
