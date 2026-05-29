use clap::Parser;

mod log;

#[derive(Parser)]
#[command(name = "cobotmap-verify", version)]
struct Args {
    /// Maps JavaScript API entry URL to verify, usually http://127.0.0.1:8080/maps-api-js
    url: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    log::set("cobotmap=info".to_string());

    if let Err(err) = cobotmap::verify_maps_js_url(&args.url).await {
        tracing::error!(error = %err, url = args.url, "cobotmap verification failed");
        std::process::exit(1);
    }

    tracing::info!(url = args.url, "cobotmap verification ok");
}
