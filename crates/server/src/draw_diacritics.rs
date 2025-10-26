use {
    axum::{Router, response::IntoResponse, routing::get},
    maud::{DOCTYPE, Markup, html},
};

pub(crate) fn router() -> Router {
    Router::new().route("/", get(get_index))
}

fn page(title: Option<&str>, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="he" dir="rtl" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "טעמים" @if let Some(t) = title { " | " (t) } }
                // script src="build/index.js" type="module" async {}
                link rel="stylesheet" href="site.css";
            }
            body {
                header {}
                main #main { (body) }
            }
        }
    }
}
// 169.jpg
async fn get_index() -> impl IntoResponse {
    page(
        Some("צייר"),
        html! {
            form method="post" enctype="multipart/form-data" {
                input type="file" name="image" accept="image/*" required;
                fieldset {
                    legend { "טישטוש" }
                    input type="number" name="blur" value="0" step="1";
                }
                fieldset {
                    legend { "חדות" }
                    input type="number" name="contrast" value="0.0" step="0.1";
                }
                fieldset {
                    label {
                        input type="checkbox" name="debug_boxes" value="true";
                        span { "ריבועים" }
                    }
                    label {
                        input type="checkbox" name="inline_diacs" value="true";
                        span { "מקף וסוף-פסוק" }
                    }
                }
                input type="submit" value="צייר טעמים";
            }
        },
    )
}
