use crate::{AppState, RenderTeamimError};
use axum::{
    Router,
    extract::{Multipart, State},
    response::IntoResponse,
    routing::get,
};
use base64::prelude::*;
use eyre::eyre;
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
                script src="build/index.js" type="module" async {}
                link rel="stylesheet" href="index.css";
            }
            body {
                (body)
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
                input type="submit" value="צייר טעמים";
            }
        },
    )
}

async fn post_index(
    State(state): State<AppState>,
    mut data: Multipart,
) -> Result<impl IntoResponse, RenderTeamimError> {
    let image = {
        let image_field = data.next_field().await?.ok_or(RenderTeamimError::MissingImageField)?;
        if image_field.name().is_none_or(|name| name != "image") {
            return Err(RenderTeamimError::MissingImageField);
        }
        image_field.bytes().await?
    };
    // let boxes = {
    //     let boxes_field = data.next_field().await?.ok_or(RenderTeamimError::MissingBoxesField)?;
    //     if boxes_field.name().is_none_or(|name| name != "boxes") {
    //         return Err(RenderTeamimError::MissingBoxesField);
    //     }
    //     boxes_field.text().await?
    // };

    // let text = boxes.lines().flat_map(|line| line.chars().next()).collect::<String>();
    // let boxes =
    //     boxes.lines().map(|line| Ok(into_geometry(&parse_char_box(line)?, OriginPos::TopLeft)));
    let image = state
        .ctx
        .lock()
        .map_err(|err| eyre!("lock ctx: {err}"))?
        .place_teamim(&image, PlaceOptions::default())?;
    Ok(page(
        Some("תוצאות"),
        html! {
            img src={"data:image/png;base64," (BASE64_STANDARD.encode(image))};
        },
    ))
}
