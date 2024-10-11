use anyhow::Context;
use axum::{http::HeaderName, Router};
use axum_extra::TypedHeader;
use hyper::header::CONTENT_TYPE;
use tokio::net::TcpListener;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod axum_range;
pub mod jrec;
pub mod transport;
pub mod utils;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=trace,tower_http=trace,axum=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let router = jrec::make_router();
    let app = Router::new()
        .nest("/", router)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers([CONTENT_TYPE]),
        )
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .context("binding to port 3000")?;

    info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app.into_make_service())
        .await
        .context("running server")
}
