use maud::{html, Markup};
use teamim::training_diff::{DiffOp, Div};

#[must_use]
pub fn render_div(val: Div) -> Markup {
    html! {
        div #box-container style={"width: "(val.width)"px; height: "(val.height)"px;"} {
            @for tess_box in val.tess_box {
                tess-box top=(tess_box.rect.top) left=(tess_box.rect.left) right=(tess_box.rect.right) bottom=(tess_box.rect.bottom) {
                    @for op in tess_box.value {
                        @match op {
                            DiffOp::Equal(value) => (value),
                            DiffOp::Insert { err } => insert err=(err) {},
                            DiffOp::Delete(value) => delete { (value) },
                        }
                    }
                }
            }
        }
    }
}
