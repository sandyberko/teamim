use std::path::PathBuf;

use axum::{
    body::Bytes,
    extract::{multipart::MultipartError, Multipart},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use axum_serde::Xml;
use serde::Serialize;
use teamim::{
    into_geometry, parse_box_line, place_teamim, tesseract_ext::BoundingBox, MismatchError,
    OriginPos, PlaceError, PlaceOptions,
};
use thiserror::Error;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ensure assets dir exists
    let boxedit_dir = PathBuf::from("assets/boxedit");
    if !boxedit_dir.exists() {
        panic!("boxedit dir does not exist: {boxedit_dir:?}");
    }

    // build our application with a single route
    let app = Router::new()
        .route("/recognize", post(post_recognize))
        .route("/recognizeTraining", post(post_recognize_training))
        .route("/renderTeamim", post(post_render_teamim))
        .route("/diff", post(post_diff))
        .nest_service("/fonts", ServeDir::new("assets/fonts"))
        // TODO disable this in production
        .nest_service("/src", ServeDir::new(boxedit_dir.join("src")))
        .fallback_service(ServeDir::new(boxedit_dir.join("assets")));
    // .layer(TraceLayer::new_for_http());

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    // launch browser
    let _browser = tokio::spawn(async move {
        let url = "http://localhost:3000";
        println!("Opening browser at {url}");
        open::that(url).unwrap();
    });

    axum::serve(listener, app).await.unwrap();
}

#[derive(Error, Debug)]
enum RecognizeError {
    #[error("empty image")]
    EmptyImage,
    #[error(transparent)]
    Other(#[from] eyre::Report),
}

impl IntoResponse for RecognizeError {
    fn into_response(self) -> Response {
        match self {
            RecognizeError::EmptyImage => {
                (StatusCode::BAD_REQUEST, self.to_string()).into_response()
            }
            RecognizeError::Other(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    }
}
#[axum::debug_handler]
async fn post_recognize(image: Bytes) -> Result<impl IntoResponse, RecognizeError> {
    if image.is_empty() {
        return Err(RecognizeError::EmptyImage);
    }
    let box_file = teamim::recognize(&image)?;
    let headers = [(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/tesseract-lstm-box"),
    )]
    .into_iter()
    .collect::<HeaderMap>();
    Ok((headers, box_file))
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Div {
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "@style")]
    style: String,
    tess_box: Vec<BoundingBox<String>>,
}

#[axum::debug_handler]
async fn post_recognize_training(image: Bytes) -> Result<Xml<Div>, RecognizeError> {
    if image.is_empty() {
        return Err(RecognizeError::EmptyImage);
    }
    let (tess_box, w, h) = teamim::recognize_training(&image)?;
    let boxes = Div {
        id: "box-container".to_owned(),
        style: format!("width: {w}px; height: {h}px;"),
        tess_box,
    };
    Ok(boxes.into())
}

#[derive(Serialize)]
#[serde(remote = "MismatchError", rename_all = "camelCase")]
struct MismatchErrorDef {
    box_number: usize,
    expected: char,
}

#[derive(Serialize)]
struct Helper(#[serde(with = "MismatchErrorDef")] MismatchError);

#[derive(Error, Debug)]
enum RenderTeamimError {
    #[error(transparent)]
    Place(#[from] PlaceError),
    #[error(transparent)]
    Multipart(#[from] MultipartError),
    #[error("missing image field")]
    MissingImageField,
    #[error("missing box field")]
    MissingBoxesField,
}

impl IntoResponse for RenderTeamimError {
    fn into_response(self) -> Response {
        match self {
            RenderTeamimError::Place(err) => match err {
                PlaceError::NotFound => {
                    (StatusCode::BAD_REQUEST, Json("Not found")).into_response()
                }
                PlaceError::Mismatch(e) => {
                    (StatusCode::BAD_REQUEST, Json(Helper(e))).into_response()
                }
                PlaceError::Other(e) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response()
                }
                PlaceError::Pix(pix_error) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, pix_error.to_string()).into_response()
                }
            },
            RenderTeamimError::MissingImageField => {
                (StatusCode::BAD_REQUEST, "missing image field").into_response()
            }
            RenderTeamimError::MissingBoxesField => {
                (StatusCode::BAD_REQUEST, "missing box field").into_response()
            }
            RenderTeamimError::Multipart(e) => e.into_response(),
        }
    }
}

async fn post_render_teamim(mut data: Multipart) -> Result<impl IntoResponse, RenderTeamimError> {
    let image = {
        let image_field = data
            .next_field()
            .await?
            .ok_or(RenderTeamimError::MissingImageField)?;
        if image_field.name().is_none_or(|name| name != "image") {
            return Err(RenderTeamimError::MissingImageField);
        }
        image_field.bytes().await?
    };
    let boxes = {
        let boxes_field = data
            .next_field()
            .await?
            .ok_or(RenderTeamimError::MissingBoxesField)?;
        if boxes_field.name().is_none_or(|name| name != "boxes") {
            return Err(RenderTeamimError::MissingBoxesField);
        }
        boxes_field.text().await?
    };

    let text = boxes
        .lines()
        .flat_map(|line| line.chars().next())
        .collect::<String>();
    let boxes = boxes
        .lines()
        .map(|line| Ok(into_geometry(parse_box_line(line)?, OriginPos::TopLeft)));
    let image = place_teamim(&image, PlaceOptions::default(), &text, boxes)?;
    let headers = [(
        header::CONTENT_TYPE,
        HeaderValue::from_static(mime::IMAGE_PNG.as_ref()),
    )]
    .into_iter()
    .collect::<HeaderMap>();
    Ok((headers, Bytes::from_owner(image)))
}

#[derive(Debug, Error)]
enum DiffError {
    #[error(transparent)]
    Other(#[from] eyre::Report),
}

impl IntoResponse for DiffError {
    fn into_response(self) -> Response {
        match self {
            DiffError::Other(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    }
}

#[axum::debug_handler]
async fn post_diff(text: String) -> Result<Json<Vec<teamim::DiffOp<'static>>>, DiffError> {
    Ok(teamim::diff(&text).map(Json)?)
}
