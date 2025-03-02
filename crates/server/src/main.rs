use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Multipart, Query, Request, State, multipart::MultipartError},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use eyre::eyre;
use maud::Markup;
use serde::{Deserialize, Serialize};
use teamim::{
    MismatchError, OriginPos, PlaceError, PlaceOptions, into_geometry, parse_box_line,
    place_teamim, training_diff::Div,
};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{self, AsyncWriteExt, BufWriter},
};
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    ctx: Arc<Mutex<teamim::TeamimCtx>>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

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

    let state = AppState {
        ctx: Arc::new(Mutex::new(teamim::TeamimCtx::new()?)),
    };

    // build our application with a single route
    let app = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/recognize", post(post_recognize))
                .route("/recognizeTraining", post(post_recognize_training))
                .route("/renderTeamim", post(post_render_teamim))
                .route("/diff", post(post_diff))
                .route("/saveDiff", post(save_diff)),
        )
        .nest_service("/fonts", ServeDir::new("assets/fonts"))
        .nest_service("/images", ServeDir::new("assets/images"))
        .nest_service("/diffs", ServeDir::new("assets/diffs"))
        .nest_service("/correctedDiffs", ServeDir::new("assets/corrected-diffs"))
        // TODO disable this in production
        .nest_service("/src", ServeDir::new(boxedit_dir.join("src")))
        .fallback_service(
            ServiceBuilder::new()
                .layer(middleware::from_fn(set_cache_control))
                .service(ServeDir::new(boxedit_dir.join("assets"))),
        )
        .with_state(state);

    // .layer(TraceLayer::new_for_http());

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    // launch browser
    let _browser = tokio::spawn(async move {
        let url = "http://localhost:3000";
        println!("Opening browser at {url}");
        _ = open::that(url);
    });

    axum::serve(listener, app).await?;
    Ok(())
}

async fn set_cache_control(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    response
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
            RecognizeError::Other(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
        }
    }
}
#[axum::debug_handler]
async fn post_recognize(
    State(state): State<AppState>,
    image: Bytes,
) -> Result<impl IntoResponse, RecognizeError> {
    if image.is_empty() {
        return Err(RecognizeError::EmptyImage);
    }
    let box_file = state
        .ctx
        .lock()
        .map_err(|_| eyre!("lock ctx"))?
        .recognize(&image)?;

    let headers = [(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/tesseract-lstm-box"),
    )]
    .into_iter()
    .collect::<HeaderMap>();
    Ok((headers, box_file))
}

#[axum::debug_handler]
async fn post_recognize_training(
    State(state): State<AppState>,
    image: Bytes,
) -> Result<Markup, RecognizeError> {
    if image.is_empty() {
        return Err(RecognizeError::EmptyImage);
    }
    let (tess_box, width, height) = state
        .ctx
        .lock()
        .map_err(|err| eyre!("lock error: {err}"))?
        .recognize_training(&image)?;
    Ok(Div {
        width,
        height,
        tess_box,
    }
    .into())
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

// #region Save diff
#[derive(Debug, Error)]
#[error("Something went wrong: {0}")]
struct SaveDiffError(#[from] io::Error);

impl IntoResponse for SaveDiffError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}
#[derive(Deserialize)]
struct SaveDiffQuery {
    file: String,
}
#[axum::debug_handler]
async fn save_diff(Query(query): Query<SaveDiffQuery>, diff: String) -> Result<(), SaveDiffError> {
    let path = PathBuf::from("assets/corrected-diffs")
        .join(query.file)
        .with_extension("html");
    let mut w = BufWriter::new(File::create(path).await?);
    w.write_all(diff.as_bytes()).await?;
    Ok(())
}
// #endregion

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use maud::Markup;
    use teamim::training_diff::{BoundingBoxDiff, DiffOp};

    #[test]
    fn diff_serialization() {
        let root = super::Div {
            width: 100,
            height: 100,
            tess_box: vec![
                BoundingBoxDiff {
                    left: 1,
                    bottom: 2,
                    right: 3,
                    top: 4,
                    value: vec![
                        DiffOp::Equal("foo".to_owned()),
                        DiffOp::Delete("bar".to_owned()),
                        DiffOp::insert("baz".to_owned()),
                    ],
                },
                BoundingBoxDiff {
                    left: 5,
                    bottom: 6,
                    right: 7,
                    top: 8,
                    value: vec![DiffOp::Equal("qux".to_owned())],
                },
            ],
        };
        let markup: Markup = root.into();
        assert_snapshot!(markup.into_string());
    }
}
