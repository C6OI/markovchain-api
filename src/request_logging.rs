use axum::extract::Request;
use tower_http::trace::{
    DefaultOnBodyChunk, DefaultOnEos, DefaultOnRequest, DefaultOnResponse, HttpMakeClassifier,
    TraceLayer,
};
use tracing::{info_span, Level, Span};

use crate::client_ip::ClientIp;

/// Creates a span for the request and includes the matched path.
///
/// The matched path is useful for figuring out which handler the request was routed to.
pub fn log_requests() -> TraceLayer<
    HttpMakeClassifier,
    impl Fn(&Request) -> Span + Clone,
    DefaultOnRequest,
    DefaultOnResponse,
    DefaultOnBodyChunk,
    DefaultOnEos,
    (),
> {
    TraceLayer::new_for_http()
        .make_span_with(|request: &Request| {
            let method = request.method();
            let uri = request.uri();
            let client_ip = request
                .extensions()
                .get::<ClientIp>()
                .map(|client_ip| client_ip.0)
                .unwrap();

            info_span!("req", %client_ip, %method, %uri)
        })
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
        // By default, `TraceLayer` will log 5xx responses, but we're doing our specific
        // logging inside [AppError::into_response] so disable that
        .on_failure(())
}
