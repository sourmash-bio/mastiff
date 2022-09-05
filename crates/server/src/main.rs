use std::{borrow::Cow, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    body::Bytes,
    error_handling::HandleErrorLayer,
    extract::{ContentLengthLimit, Extension},
    http::StatusCode,
    response::IntoResponse,
    routing::{get_service, post},
    Router,
};
use tower::{BoxError, ServiceBuilder};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use clap::Parser;
use sourmash::index::revindex::RevIndex;
use sourmash::signature::{Signature, SigsTrait};
use sourmash::sketch::minhash::{max_hash_for_scaled, KmerMinHash};
use sourmash::sketch::Sketch;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Path to rocksdb index dir
    #[clap(parse(from_os_str))]
    index: PathBuf,

    /// Path to static assets
    #[clap(
        short = 'a',
        long = "assets",
        parse(from_os_str),
        default_value = "assets/"
    )]
    assets: PathBuf,

    /// ksize
    #[clap(short = 'k', long = "ksize", default_value = "21")]
    ksize: u8,

    /// scaled
    #[clap(short = 's', long = "scaled", default_value = "1000")]
    scaled: usize,

    /// port
    #[clap(short = 'p', long = "port", default_value = "3059")]
    port: u16,

    /// threshold_bp
    #[clap(short = 't', long = "threshold_bp", default_value = "50000")]
    threshold_bp: usize,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "mastiff=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let opts = Cli::parse();

    let max_hash = max_hash_for_scaled(opts.scaled as u64);
    let mh = KmerMinHash::builder()
        .num(0)
        .max_hash(max_hash)
        .ksize(opts.ksize as u32)
        .build();

    let threshold = opts.threshold_bp / mh.scaled() as usize;

    let state = Arc::new(State {
        db: Arc::new(RevIndex::open(opts.index.as_ref(), true)),
        template: Arc::new(Sketch::MinHash(mh)),
        threshold,
    });

    // Build our application by composing routes
    let app = Router::new()
        .route("/search", post(search))
        //.route("/gather", post(gather))
        .fallback(get_service(ServeDir::new(opts.assets)).handle_error(handle_static_serve_error))
        // Add middleware to all routes
        .layer(
            ServiceBuilder::new()
                // Handle errors from middleware
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                .concurrency_limit(200)
                .timeout(Duration::from_secs(3600))
                .layer(TraceLayer::new_for_http())
                .layer(Extension(state))
                .into_inner(),
        );

    // Run our app with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], opts.port));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

type SharedState = Arc<State>;

struct State {
    db: Arc<RevIndex>,
    template: Arc<Sketch>,
    threshold: usize,
}

impl State {
    async fn search(&self, query: Signature) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let db = self.db.clone();
        let threshold = self.threshold;
        let template = self.template.clone();

        let (matches, query_size) = tokio::task::spawn_blocking(move || {
            if let Some(Sketch::MinHash(mh)) = query.select_sketch(&template) {
                let counter = db.counter_for_query(mh);
                let matches = db.matches_from_counter(counter, threshold);
                (matches, mh.size() as f64)
            } else {
                todo!()
            }
        })
        .await?;

        let mut csv = vec!["SRA accession,containment".into()];
        csv.extend(matches.into_iter().map(|(path, size)| {
            let containment = size as f64 / query_size;
            format!(
                "{},{}",
                path.split('/').last().unwrap().split('.').next().unwrap(),
                containment
            )
        }));
        Ok(csv)
    }
}

async fn search(
    ContentLengthLimit(bytes): ContentLengthLimit<Bytes, { 1024 * 5_000 }>, // ~5mb
    Extension(state): Extension<SharedState>,
    //) -> Result<Json<serde_json::Value>, StatusCode> {
) -> Result<String, StatusCode> {
    let sig = parse_sig(&bytes).unwrap();
    let matches = state.search(sig).await.unwrap();

    Ok(matches.join("\n"))
}

fn parse_sig(raw_data: &[u8]) -> Result<Signature, BoxError> {
    let sig = Signature::from_reader(raw_data)?.swap_remove(0);
    Ok(sig)
}

async fn handle_static_serve_error(error: std::io::Error) -> impl IntoResponse {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled static serve error: {}", error)),
    )
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {}", error)),
    )
}
