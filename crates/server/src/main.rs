use axum::Router;
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
    let app = Router::new().fallback_service(ServeDir::new("assets/boxedit/assets/"));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("[::1]:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
