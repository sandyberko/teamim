use axum::{
    Json, Router,
    body::Bytes,
    extract::{Multipart, Query, Request, State, multipart::MultipartError},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use eyre::{bail, eyre};
use maud::Markup;
use serde::{Deserialize, Serialize};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use teamim::{
    MismatchError, OriginPos, PlaceError, PlaceOptions, into_geometry, parse_box_line,
    place_teamim, training_diff::Div,
};
use thiserror::Error;
use tokio::{
    fs::File,
    io::{self, AsyncWriteExt, BufWriter},
    net::TcpListener,
};
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing::{error, info, instrument};

#[derive(Clone)]
struct AppState {
    ctx: Arc<Mutex<teamim::TeamimCtx>>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let _guead = tracing_init()?;

    serve().await?;

    Ok(())
}

#[instrument(err)]
async fn serve() -> eyre::Result<()> {
    // ensure assets dir exists
    let boxedit_dir = PathBuf::from("assets/boxedit");
    if !boxedit_dir.exists() {
        bail!("boxedit dir does not exist: {boxedit_dir:?}");
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
        .nest_service(
            "/correctedDiffs",
            ServiceBuilder::new()
                .layer(middleware::from_fn(no_cache))
                .service(ServeDir::new("assets/corrected-diffs")),
        )
        .fallback_service(ServeDir::new(boxedit_dir.join("assets")))
        .with_state(state);

    #[cfg(debug_assertions)]
    let app = app.nest_service("/src", ServeDir::new(boxedit_dir.join("src")));

    let app = app.layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000)).await?;

    // launch browser
    let _browser = tokio::spawn({
        let url = format!("http://{}", listener.local_addr()?);
        async move {
            info!("Opening browser at {url}");
            if let Err(e) = open::that(url) {
                error!("Failed to open browser: {e}");
            }
        }
    });

    axum::serve(listener, app).await?;

    Ok(())
}

fn tracing_init() -> eyre::Result<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::{non_blocking, rolling};
    use tracing_error::ErrorLayer;
    use tracing_subscriber::{
        EnvFilter, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    let (non_blocking_appender, guard) = non_blocking(rolling::daily("logs", "teamim-server"));
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    Registry::default()
        .with(fmt::layer().pretty().with_writer(std::io::stderr))
        .with(file_layer)
        .with(ErrorLayer::default())
        .with(env_filter)
        .init();

    color_eyre::install()?;

    Ok(guard)
}

async fn no_cache(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
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
