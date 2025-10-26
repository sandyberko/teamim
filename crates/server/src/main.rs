mod diff_entry;
mod draw_diacritics;

use {
    axum::{
        Json, Router,
        body::Body,
        extract::{Query, Request},
        http::{HeaderValue, StatusCode, header},
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::{get, post},
    },
    color_eyre::Section,
    eyre::{bail, eyre},
    futures::{StreamExt, stream},
    serde::Deserialize,
    std::{
        net::{Ipv4Addr, SocketAddrV4},
        path::PathBuf,
    },
    thiserror::Error,
    tokio::{
        fs::{self, File},
        io::{self, AsyncWriteExt, BufWriter},
        net::TcpListener,
    },
    tower::ServiceBuilder,
    tower_http::{services::ServeDir, trace::TraceLayer},
    tracing::{error, info, instrument},
};

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

    // build our application with a single route
    let app = Router::new()
        .nest(
            "/api",
            Router::new()
                .route("/diff", post(post_diff))
                .route("/diffIndex", get(get_diff_index))
                .route("/saveDiff", post(save_diff)),
        )
        .nest("/drawDiacritics", draw_diacritics::router())
        .nest_service("/fonts", ServeDir::new("assets/fonts"))
        .nest_service("/images", ServeDir::new("assets/images"))
        .nest_service("/diffs", ServeDir::new("assets/diffs"))
        .nest_service(
            "/correctedDiffs",
            ServiceBuilder::new()
                .layer(middleware::from_fn(no_cache))
                .service(ServeDir::new("assets/corrected-diffs")),
        )
        .nest_service(
            "/correctedBoxfiles",
            ServiceBuilder::new()
                .layer(middleware::from_fn(no_cache))
                .service(ServeDir::new("assets/corrected_boxfiles")),
        )
        .fallback_service(ServeDir::new(boxedit_dir.join("assets")));
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
    let file_layer = fmt::layer().with_ansi(false).with_writer(non_blocking_appender);

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
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
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

async fn get_diff_index() -> impl IntoResponse {
    let distances = include_str!("../../../assets/diffs/distances.txt");
    let stream =
        stream::iter(distances.lines().take(1000).enumerate()).then(diff_entry::diff_entry);
    Response::builder()
        .header(header::CONTENT_TYPE, HeaderValue::from_static(mime::TEXT_HTML.as_ref()))
        .body(Body::from_stream(stream))
        .expect("valid headers")
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
    let path = PathBuf::from("assets/corrected-diffs").join(query.file).with_extension("html");
    let parent_dir = path
        .parent()
        .ok_or_else(|| io::Error::other(eyre!("no parent dir").note("path: {path:?}")))?;
    if !parent_dir.exists() {
        fs::create_dir_all(parent_dir).await?;
    }
    let mut w = BufWriter::new(File::create(path).await?);
    w.write_all(diff.as_bytes()).await?;
    w.flush().await?;
    Ok(())
}
// #endregion
