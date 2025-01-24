use std::path::PathBuf;

use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use eyre::OptionExt;
use serde::Serialize;
use teamim::{
    into_geometry, parse_box_line, place_teamim, MismatchError, OriginPos, PlaceError, PlaceOptions,
};
use thiserror::Error;
use tower_http::{services::ServeDir, trace::TraceLayer};
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
        .route("/renderTeamim", post(post_render_teamim))
        // TODO disable this in production
        .nest_service("/src", ServeDir::new(boxedit_dir.join("src")))
        .fallback_service(ServeDir::new(boxedit_dir.join("assets")))
        .layer(TraceLayer::new_for_http());

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("[::1]:3000").await.unwrap();

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
            RecognizeError::EmptyImage => (StatusCode::BAD_REQUEST, "empty image").into_response(),
            RecognizeError::Other(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    }
}

async fn post_recognize(image: Bytes) -> Result<String, RecognizeError> {
    if image.is_empty() {
        return Err(RecognizeError::EmptyImage);
    }
    Ok(teamim::recognize(&image)?)
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
#[error(transparent)]
struct RenderTeamimError(#[from] PlaceError);

impl From<eyre::Report> for RenderTeamimError {
    fn from(err: eyre::Report) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for RenderTeamimError {
    fn into_response(self) -> Response {
        match self.0 {
            PlaceError::NotFound => (StatusCode::BAD_REQUEST, Json("Not found")).into_response(),
            PlaceError::Mismatch(e) => (StatusCode::BAD_REQUEST, Json(Helper(e))).into_response(),
            PlaceError::Other(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())).into_response()
            }
        }
    }
}

async fn post_render_teamim(boxes: String) -> Result<(), RenderTeamimError> {
    let text = boxes
        .lines()
        .map(|line| line.chars().next().ok_or_eyre("empty line"))
        .collect::<eyre::Result<String>>()?;
    let boxes = boxes
        .lines()
        .map(|line| Ok(into_geometry(parse_box_line(line)?, OriginPos::TopLeft)));
    place_teamim(&(), PlaceOptions::default(), &text, boxes).map_err(RenderTeamimError)?;
    Ok(())
}
