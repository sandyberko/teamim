use xilem::core::{
    MapMessage, MapState, MessageResult, View, ViewPathTracker, map_action, map_state,
};

pub(crate) trait ViewExt<State, Action, Context>: View<State, Action, Context>
where
    State: 'static,
    Action: 'static,
    Context: ViewPathTracker + 'static,
{
    #[expect(clippy::type_complexity)]
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
        Self: Sized,
        ParentAction: 'static,
        F: Fn(&mut State, Action) -> ParentAction + 'static,
    {
        map_action(self, map_fn)
    }

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
}

impl<V, State, Action, Context> ViewExt<State, Action, Context> for V
where
    V: View<State, Action, Context>,
    State: 'static,
    Action: 'static,
    Context: ViewPathTracker + 'static,
{
}
