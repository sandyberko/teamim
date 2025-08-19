use xilem::core::{
    MapMessage, MapState, MessageResult, View, ViewPathTracker, map_message, map_state,
};

pub(crate) trait ViewExt<State, Action, Context>: View<State, Action, Context>
where
    State: 'static,
    Action: 'static,
    Context: ViewPathTracker + 'static,
{
    /// See [`::xilem::core::map_state`]
    fn map_state<ParentState, F>(
        self,
        f: F,
    ) -> MapState<Self, F, ParentState, State, Action, Context>
    where
        Self: Sized,
        ParentState: 'static,
        F: Fn(&mut ParentState) -> &mut State + 'static,
    {
        map_state(self, f)
    }

    /// See [`::xilem::core::map_message`]
    fn map_message<ParentAction, F>(
        self,
        map_fn: F,
    ) -> MapMessage<Self, State, ParentAction, Action, Context, F>
    where
        Self: Sized,
        ParentAction: 'static,
        F: Fn(&mut State, MessageResult<Action>) -> MessageResult<ParentAction> + 'static,
    {
        map_message(self, map_fn)
    }
}

impl<V, State, Action, Context> ViewExt<State, Action, Context> for V
where
    V: View<State, Action, Context>,
    State: 'static,
    Action: 'static,
    Context: ViewPathTracker + 'static,
{
}
