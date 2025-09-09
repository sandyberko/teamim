use xilem::core::{
    MapMessage, MapState, MessageResult, View, ViewPathTracker, map_action, map_message, map_state,
};

pub(crate) trait ViewExt<State, Action, Context>: View<State, Action, Context>
where
    Self: Sized,
    State: 'static,
    Action: 'static,
    Context: ViewPathTracker + 'static,
{
    /// See [`::xilem::core::map_action`]
    fn map_action<ParentAction, F>(
        self,
        map_fn: F,
    ) -> MapMessage<
        Self,
        State,
        ParentAction,
        Action,
        Context,
        impl Fn(&mut State, MessageResult<Action>) -> MessageResult<ParentAction> + 'static,
    >
    where
        ParentAction: 'static,
        F: Fn(&mut State, Action) -> ParentAction + 'static,
    {
        map_action(self, map_fn)
    }
    /// See [`::xilem::core::map_message`]
    fn map_message<ParentAction, F>(
        self,
        map_fn: F,
    ) -> MapMessage<Self, State, ParentAction, Action, Context, F>
    where
        ParentAction: 'static,
        F: Fn(&mut State, MessageResult<Action>) -> MessageResult<ParentAction> + 'static,
    {
        map_message(self, map_fn)
    }
    fn map_state<ParentState, F>(
        self,
        map_fn: F,
    ) -> MapState<Self, F, ParentState, State, Action, Context>
    where
        ParentState: 'static,
        F: Fn(&mut ParentState) -> &mut State + 'static,
    {
        map_state(self, map_fn)
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
