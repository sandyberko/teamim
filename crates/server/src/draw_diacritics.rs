use crate::{AppState, RenderTeamimError};
use axum::{
    Router,
    extract::{Multipart, State},
    response::IntoResponse,
    routing::get,
};
use base64::prelude::*;
use bytes::Bytes;
use eyre::{OptionExt, bail, eyre};
use maud::{DOCTYPE, Markup, html};
use teamim::PlaceOptions;

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/", get(get_index).post(post_index))
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

async fn get_index() -> impl IntoResponse {
    page(
        Some("צייר"),
        html! {
            form method="post" enctype="multipart/form-data" {
                input type="file" name="image" accept="image/*" required;
                fieldset {
                    legend { "טישטוש" }
                    input type="number" name="blur" value="0.0" step="0.1";
                }
                fieldset {
                    legend { "חדות" }
                    input type="number" name="contrast" value="0.0" step="0.1";
                }
                fieldset {
                    legend { "ריבועים" }
                    input type="checkbox" name="debug_boxes" value="true";
                }
                input type="submit" value="צייר טעמים";
            }
        },
    )
}

#[derive(Debug, Default)]
struct DiacriticOptions {
    blur: f32,
    contrast: f32,
    debug_boxes: bool,
    image: Bytes,
}

impl DiacriticOptions {
    async fn from_mutipart(data: &mut Multipart) -> eyre::Result<Self> {
        let mut options = DiacriticOptions::default();

        while let Some(field) = data.next_field().await? {
            match field.name().ok_or_eyre("Missing field name")? {
                "image" => options.image = field.bytes().await?,
                "blur" => options.blur = field.text().await?.parse()?,
                "contrast" => options.contrast = field.text().await?.parse()?,
                "debug_boxes" => options.debug_boxes = field.text().await?.parse()?,
                name => bail!("Unexpected field: {name}"),
            }
        }

        Ok(options)
    }
}

async fn post_index(
    State(state): State<AppState>,
    mut data: Multipart,
) -> Result<impl IntoResponse, RenderTeamimError> {
    let options =
        DiacriticOptions::from_mutipart(&mut data).await.map_err(RenderTeamimError::BadRequest)?;

    let image = state.ctx.lock().map_err(|err| eyre!("lock ctx: {err}"))?.place_teamim(
        &options.image,
        PlaceOptions::default()
            .set_blur(options.blur)
            .set_contrast(options.contrast)
            .set_debug_boxes(options.debug_boxes),
    )?;

    Ok(page(
        Some("תוצאות"),
        html! {
            img src={"data:image/png;base64," (BASE64_STANDARD.encode(image))};
        },
    ))
}
