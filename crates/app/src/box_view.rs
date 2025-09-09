mod widget;

use xilem::{
    Pod, Vec2, ViewCtx, WidgetView,
    core::{MessageResult, View, ViewMarker},
    masonry::kurbo::Size,
    view::transformed,
};

use self::widget::{TBox, TBoxState};

pub(crate) fn tbox<State>(pos: impl Into<Vec2>, size: impl Into<Size>) -> impl WidgetView<State>
where
    State: Send + Sync + 'static,
{
    let (pos, size) = (pos.into(), size.into());
    let child = TBoxView { initial: TBoxState { size } };
    transformed(child).translate(pos)
}

pub(crate) struct TBoxView {
    initial: TBoxState,
}

impl ViewMarker for TBoxView {}
impl<State, Action> View<State, Action, ViewCtx> for TBoxView
where
    State: 'static,
    Action: 'static,
{
    type Element = Pod<TBox>;

    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _app_state: &mut State) -> (Self::Element, Self::ViewState) {
        ctx.with_leaf_action_widget(|_ctx| Pod::new(TBox::new(self.initial)))
    }

    fn rebuild(
        &self,
        _prev: &Self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        _element: xilem::core::Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) {
    }
    fn teardown(
        &self,
        _view_state: &mut Self::ViewState,
        _ctx: &mut ViewCtx,
        _element: xilem::core::Mut<'_, Self::Element>,
    ) {
    }

    fn message(
        &self,
        _view_state: &mut Self::ViewState,
        _message: &mut xilem::core::MessageContext,
        _element: xilem::core::Mut<'_, Self::Element>,
        _app_state: &mut State,
    ) -> xilem::core::MessageResult<Action> {
        MessageResult::Nop
    }
}
