use accesskit::{Node, Role};
use masonry::{
    core::{Widget, WidgetPod},
    kurbo::{Insets, Size},
    properties::Padding,
    widgets,
};
use smallvec::SmallVec;
use smallvec::smallvec;
use std::marker::PhantomData;
use xilem::{
    core::{View, ViewMarker}, Affine, Color, Pod, Vec2, ViewCtx
};

struct TBoxState {
    pos: Vec2,
    size: Size,
}

pub(crate) struct TBox {
    inner: WidgetPod<dyn Widget>,
    state: TBoxState,
}

type TextState = ();

impl TBox {
    pub(crate) fn new(pos: impl Into<Vec2>, size: impl Into<Size>) -> Self {
        let (pos, size) = (pos.into(), size.into());
        let textbox = widgets::Textbox::new("TEXT");
        Self {
            inner: WidgetPod::new_with_transform(Box::new(textbox), Affine::translate(pos)),
            state: TBoxState { pos, size },
        }
    }
}

const TEXTBOX_MARGIN: Padding = Padding::horizontal(2.0);

impl Widget for TBox {
    fn register_children(&mut self, ctx: &mut masonry::core::RegisterCtx) {
        ctx.register_child(&mut self.inner);
    }

    fn layout(
        &mut self,
        ctx: &mut masonry::core::LayoutCtx,
        _props: &mut masonry::core::PropertiesMut<'_>,
        bc: &masonry::core::BoxConstraints,
    ) -> masonry::kurbo::Size {
        self.state.size
    }

    fn paint(
        &mut self,
        ctx: &mut masonry::core::PaintCtx,
        _props: &masonry::core::PropertiesRef<'_>,
        scene: &mut masonry::vello::Scene,
    ) {
        let size = ctx.size();
        let border_width = 1.0;
        let outline_rect = size.to_rect().inset(Insets::new(
            -TEXTBOX_MARGIN.left - border_width / 2.,
            -TEXTBOX_MARGIN.top - border_width / 2.,
            -TEXTBOX_MARGIN.left - border_width / 2.,
            -TEXTBOX_MARGIN.bottom - border_width / 2.,
        ));
        masonry::util::stroke(scene, &outline_rect, Color::WHITE, border_width);
    }

    fn accessibility_role(&self) -> Role {
        todo!()
    }

    fn accessibility(
        &mut self,
        ctx: &mut masonry::core::AccessCtx,
        _props: &masonry::core::PropertiesRef<'_>,
        node: &mut Node,
    ) {
        todo!()
    }

    fn children_ids(&self) -> SmallVec<[masonry::core::WidgetId; 16]> {
        smallvec![self.inner.id()]
    }
}

struct TBoxView {
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

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        ctx.with_leaf_action_widget(|ctx| {
            ctx.new_pod(TBox::new(self.initial.pos, self.initial.size))
        })
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: xilem::core::Mut<'_, Self::Element>,
    ) {
        todo!()
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: xilem::core::Mut<'_, Self::Element>,
    ) {
        todo!()
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[xilem::core::ViewId],
        message: xilem::core::DynMessage,
        app_state: &mut State,
    ) -> xilem::core::MessageResult<Action, xilem::core::DynMessage> {
        todo!()
    }
}
