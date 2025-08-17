use smallvec::{SmallVec, smallvec};
use xilem::{
    Color,
    masonry::{
        accesskit::{Node, Role},
        core::{
            AccessCtx, BoxConstraints, LayoutCtx, NewWidget, NoAction, PaintCtx, PropertiesMut,
            PropertiesRef, RegisterCtx, Widget, WidgetId, WidgetPod,
        },
        kurbo::{Insets, Point, Size},
        properties::Padding,
        util,
        vello::Scene,
        widgets::TextInput,
    },
};

#[derive(Clone, Copy)]
pub(crate) struct TBoxState {
    pub(crate) size: Size,
}

pub(crate) struct TBox {
    inner: WidgetPod<TextInput>,
    state: TBoxState,
}

type TextState = ();

impl TBox {
    pub(crate) fn new(state: TBoxState) -> Self {
        let textbox = TextInput::new("TEXT");
        Self {
            // TODO inner: WidgetPod::new_with_transform(Box::new(textbox), Affine::translate(pos)),
            inner: NewWidget::new(textbox).to_pod(),
            state,
        }
    }
}

const TEXTBOX_MARGIN: Padding = Padding::horizontal(2.0);

impl Widget for TBox {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx) {
        ctx.register_child(&mut self.inner);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        ctx.run_layout(&mut self.inner, bc);
        ctx.place_child(&mut self.inner, Point::ORIGIN);
        self.state.size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let size = ctx.size();
        let border_width = 1.0;
        let outline_rect = size.to_rect().inset(Insets::new(
            -TEXTBOX_MARGIN.left - border_width / 2.,
            -TEXTBOX_MARGIN.top - border_width / 2.,
            -TEXTBOX_MARGIN.left - border_width / 2.,
            -TEXTBOX_MARGIN.bottom - border_width / 2.,
        ));
        util::stroke(scene, &outline_rect, Color::WHITE, border_width);
    }

    fn accessibility_role(&self) -> Role {
        Role::TextInput
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx,
        _props: &PropertiesRef<'_>,
        _node: &mut Node,
    ) {
        // TODO
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        smallvec![self.inner.id()]
    }
}
