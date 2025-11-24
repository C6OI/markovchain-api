use crate::client_ip::add_client_ip;
use crate::generator::generate_text;
use crate::input::process_input;
use crate::request_logging::log_requests;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::from_fn;
use axum::response::{IntoResponse, NoContent, Response};
use axum::routing::post;
use axum::{Json, Router};
use axum_valid::Valid;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use validator::Validate;

pub async fn start(state: Arc<AppState>) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/input", post(add_input))
        .route("/generate", post(generate))
        .layer(CorsLayer::permissive())
        .layer(log_requests())
        .layer(from_fn(add_client_ip))
        .with_state(state.clone());

    let settings = &state.settings.server;
    let addr = SocketAddr::new(settings.host, settings.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Starting web server on http://{}", addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

#[derive(Debug, Validate, Deserialize)]
struct Input {
    #[validate(length(min = 1, max = 2000))]
    input: String,
}

async fn add_input(
    state: State<Arc<AppState>>,
    Valid(Json(payload)): Valid<Json<Input>>,
) -> Result<NoContent, AppError> {
    process_input(&state.pool, payload.input).await?;
    Ok(NoContent)
}

#[derive(Debug, Validate, Deserialize)]
struct Generate {
    #[validate(length(min = 1, max = 2000))]
    start: Option<String>,
    #[validate(range(min = 1, max = 2000))]
    max_length: Option<usize>,
}

async fn generate(
    state: State<Arc<AppState>>,
    Valid(Json(payload)): Valid<Json<Generate>>,
) -> Result<String, AppError> {
    let text = generate_text(&state.pool, payload.max_length, payload.start).await?;
    Ok(text)
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error = self.0;
        tracing::error!(?error, "api error");

        (StatusCode::INTERNAL_SERVER_ERROR, format!("{error}")).into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
