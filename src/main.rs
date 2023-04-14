use axum::{
    extract::MatchedPath,
    http::Request,
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
    BoxError, Router,
};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

use axum_tracing_opentelemetry::opentelemetry_tracing_layer;
use std::{future::ready, net::SocketAddr, time::Instant};
use tokio::signal;

fn metrics_app() -> Router {
    let recorder_handle = setup_metrics_recorder();
    Router::new().route("/metrics", get(move || ready(recorder_handle.render())))
}

fn main_app() -> Router {
    Router::new()
        .route("/", get(root))
        // include trace context as header into the response
        .layer(axum_tracing_opentelemetry::response_with_trace_layer())
        // opentelemetry_tracing_layer setup `TraceLayer`, that is provided by tower-http so you have to add that as a dependency.
        .layer(opentelemetry_tracing_layer())
        .route_layer(middleware::from_fn(track_metrics))
}

async fn start_metrics_server() -> Result<(), axum::BoxError> {
    let app = metrics_app();

    // NOTE: expose metrics enpoint on a different port
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("metrics listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_metrics_signal())
        .await?;
    Ok(())
}

async fn start_main_server() -> Result<(), axum::BoxError> {
    let app = main_app();

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    tracing::info!("Main server listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_main_signal())
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), axum::BoxError> {
    // from the docs, this is a "very opinionated init of tracing, look at source to make your own"
    axum_tracing_opentelemetry::tracing_subscriber_ext::init_subscribers()?;

    // not needed since we're using opentelemetry now?
    // tracing_subscriber::registry()
    //     .with(
    //         tracing_subscriber::EnvFilter::try_from_default_env()
    //             .unwrap_or_else(|_| "example_todos=debug,tower_http=debug".into()),
    //     )
    //     .with(tracing_subscriber::fmt::layer())
    //     .init();

    // The `/metrics` endpoint should not be publicly available. If behind a reverse proxy, this
    // can be achieved by rejecting requests to `/metrics`. In this example, a second server is
    // started on another port to expose `/metrics`.
    let (_main_server, _metrics_server) = tokio::join!(start_main_server(), start_metrics_server());
    Ok(())
}

async fn root() -> &'static str {
    let trace_id = axum_tracing_opentelemetry::find_current_trace_id();
    println!("{:?}", trace_id);
    "Hello world"
}

async fn shutdown_metrics_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown of metrics");
}

async fn shutdown_main_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown of main");
    opentelemetry::global::shutdown_tracer_provider();
}

fn setup_metrics_recorder() -> PrometheusHandle {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("http_requests_duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap()
        .install_recorder()
        .unwrap()
}

async fn track_metrics<B>(req: Request<B>, next: Next<B>) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::increment_counter!("http_requests_total", &labels);
    metrics::histogram!("http_requests_duration_seconds", latency, &labels);

    response
}
