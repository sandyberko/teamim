use axum::{
    body::Bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
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

    // build our application with a single route
    let app = Router::new()
        .route("/recognize", post(post_recognize))
        .fallback_service(ServeDir::new("assets/boxedit/assets/"));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("[::1]:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

struct AppError(eyre::Report);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

impl From<eyre::Report> for AppError {
    fn from(err: eyre::Report) -> Self {
        Self(err)
    }
}

#[axum::debug_handler]
async fn post_recognize(image: Bytes) -> Result<String, AppError> {
    Ok(teamim::recognize(&image)?)
}
