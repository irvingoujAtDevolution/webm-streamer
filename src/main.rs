use std::time::Duration;

use anyhow::Context;
use axum::{http::HeaderName, Router};
use hyper::Request;
use tokio::net::TcpListener;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::state::AppState;

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
    let state = AppState::new();
    let app = Router::new()
        .nest("/", router)
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any) // Allow any origin
                .allow_headers(Any) // Allow any headers
                .allow_methods(Any) // Allow any HTTP methods
                .expose_headers(Any)
                .max_age(Duration::from_secs(86400)), // Expose any headers
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    // Create span for tracing each request
                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        base_url = %request.uri().path(),
                        status_code = tracing::field::Empty,
                    )
                })
                .on_response(
                    |response: &axum::http::Response<_>,
                     _latency: std::time::Duration,
                     span: &Span| {
                        let status = response.status();
                        // Log errors for non-2xx responses
                        if !(100..300).contains(&status.as_u16()) {
                            tracing::error!(status = %status, "Non-2xx response");
                        } else {
                            tracing::info!(status = %status, "2xx response");
                        }
                        let cors = response
                            .headers()
                            .get(HeaderName::from_static("access-control-allow-origin"));
                        tracing::info!(cors = ?cors, "CORS header");
                        span.record("status_code", status.as_u16());
                    },
                ),
        );

    let listener = TcpListener::bind("127.0.0.1:3000")
        .await
        .context("binding to port 3000")?;

    info!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await.context("running server")
}
