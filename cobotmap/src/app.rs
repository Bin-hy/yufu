use crate::{
    Config, build_maps_source_url, redact_url,
    routes::{
        RouteEntry, build_digest_map, build_route_entries, build_upstream_url,
        fallback_upstream_url, rewrite_urls_with_base,
    },
    upstream::{
        UpstreamClient, UpstreamResponse, build_client, fetch_direct_url, fetch_request,
        load_maps_source,
    },
};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode, Uri, header::CONTENT_TYPE},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use std::{collections::HashMap, error::Error, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};

const ENTRY_PATH: &str = "/maps/api/js";
const LEGACY_ENTRY_PATH: &str = "/maps-api-js";
const DEMO_PATH: &str = "/demo.html";
const FAVICON_PATH: &str = "/favicon.ico";
const DEMO_HTML: &str = include_str!("../demo.html");

pub async fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind(config.http.listen).await?;
    let client =
        build_client(config.proxy.url.as_ref()).map_err(|err| -> Box<dyn Error> { err.into() })?;
    let source = load_maps_source(
        config.maps.input.as_deref(),
        config.maps.key.as_deref(),
        &client,
    )
    .await
    .map_err(|err| -> Box<dyn Error> { err.into() })?;
    let routes = build_route_entries(&source);
    debug!(routes = routes.len(), "built cobotmap route table");
    let maps_script_src = config
        .maps
        .key
        .as_deref()
        .map(build_local_maps_script_src)
        .unwrap_or_else(|| ENTRY_PATH.to_string());
    let state = Arc::new(AppState {
        client,
        source,
        routes: RwLock::new(RouteState::new(routes)),
        maps_script_src,
        maps_key: config.maps.key,
        maps_input: config.maps.input,
    });
    let app = Router::new()
        .route("/", get(demo))
        .route(DEMO_PATH, get(demo))
        .route(FAVICON_PATH, get(favicon))
        .route(ENTRY_PATH, get(entry))
        .route(LEGACY_ENTRY_PATH, get(entry))
        .fallback(any(proxy))
        .with_state(state)
        .layer(middleware::from_fn(trace_request));

    info!(listen = %config.http.listen, entry_path = ENTRY_PATH, "cobotmap listening");

    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn verify_maps_js_url(url: &str) -> Result<(), String> {
    let response = fetch_direct_url(url).await?;
    let safe_url = redact_url(url);

    if response.status != StatusCode::OK.as_u16() {
        warn!(
            url = safe_url,
            status = response.status,
            "verification returned unexpected status"
        );
        return Err(format!("unexpected status: {}", response.status));
    }

    let content_type = response.content_type.unwrap_or_default();
    let body = String::from_utf8(response.body)
        .map_err(|err| format!("response was not valid UTF-8: {err}"))?;

    if body.is_empty() {
        warn!(url = safe_url, "verification response was empty");
        return Err("empty response body".to_string());
    }

    if !content_type.contains("javascript") && !body.contains("google.maps") {
        warn!(
            url = safe_url,
            content_type, "verification response did not look like Maps JS"
        );
        return Err("response does not look like Google Maps JS".to_string());
    }

    info!(url = safe_url, "verified cobotmap Maps JS endpoint");
    Ok(())
}

async fn demo(State(state): State<Arc<AppState>>) -> Response {
    bytes_response(
        StatusCode::OK,
        DEMO_HTML
            .replace("__COBOTMAP_MAPS_SCRIPT__", &state.maps_script_src)
            .into_bytes(),
        Some("text/html; charset=utf-8"),
    )
}

async fn favicon() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

async fn entry(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let source = match state.maps_key.as_deref() {
        Some(key) if state.maps_input.is_none() => {
            let url = build_maps_source_url(key);
            match fetch_request(&state.client, Method::GET, &url, &[], None).await {
                Ok(upstream) => match String::from_utf8(upstream.body) {
                    Ok(source) => source,
                    Err(err) => {
                        error!(error = %err, "maps source response was not valid UTF-8");
                        return text_response(StatusCode::BAD_GATEWAY, "bad maps source response");
                    }
                },
                Err(err) => {
                    error!(upstream_url = redact_url(&url), error = %err, "maps source request failed");
                    return text_response(StatusCode::BAD_GATEWAY, &err);
                }
            }
        }
        _ => state.source.clone(),
    };

    merge_routes(&state, &source).await;
    let routes = state.routes.read().await.routes.clone();

    let base = request_base(&headers);

    bytes_response(
        StatusCode::OK,
        rewrite_urls_with_base(&source, &routes, &base).into_bytes(),
        Some("application/javascript; charset=utf-8"),
    )
}

async fn proxy(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !(method == Method::GET || method == Method::POST) {
        warn!(%method, uri = redact_request_uri(&uri), "rejected unsupported method");
        return text_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "only GET and POST are supported",
        );
    }

    let url = {
        let routes = state.routes.read().await;
        build_upstream_url(&routes.digest_map, uri.path(), uri.query())
    };

    let Some(url) = url else {
        let fallback = fallback_upstream_url(uri.path(), uri.query());
        if let Some(fallback_url) = fallback {
            warn!(
                uri = redact_request_uri(&uri),
                upstream = redact_url(&fallback_url),
                "unknown proxy route, using fallback"
            );
            match fetch_request(
                &state.client,
                method,
                &fallback_url,
                body.as_ref(),
                Some(&headers),
            )
            .await
            {
                Ok(mut upstream) => {
                    rewrite_upstream_body(&state, &mut upstream, &headers).await;
                    return bytes_response(
                        StatusCode::from_u16(upstream.status).unwrap_or(StatusCode::BAD_GATEWAY),
                        upstream.body,
                        upstream.content_type.as_deref(),
                    );
                }
                Err(err) => {
                    error!(
                        upstream = redact_url(&fallback_url),
                        error = %err,
                        "fallback upstream proxy request failed"
                    );
                    return text_response(StatusCode::BAD_GATEWAY, &err);
                }
            }
        }
        warn!(
            uri = redact_request_uri(&uri),
            "unknown proxy route, no fallback"
        );
        return text_response(StatusCode::NOT_FOUND, "unknown path");
    };

    match fetch_request(&state.client, method, &url, body.as_ref(), Some(&headers)).await {
        Ok(mut upstream) => {
            let upstream_url = redact_url(&url);
            trace!(
                upstream_url,
                status = upstream.status,
                "proxied upstream response"
            );
            rewrite_upstream_body(&state, &mut upstream, &headers).await;
            bytes_response(
                StatusCode::from_u16(upstream.status).unwrap_or(StatusCode::BAD_GATEWAY),
                upstream.body,
                upstream.content_type.as_deref(),
            )
        }
        Err(err) => {
            error!(upstream_url = redact_url(&url), error = %err, "upstream proxy request failed");
            text_response(StatusCode::BAD_GATEWAY, &err)
        }
    }
}

async fn rewrite_upstream_body(
    state: &AppState,
    upstream: &mut UpstreamResponse,
    headers: &HeaderMap,
) {
    if !is_text_response(upstream.content_type.as_deref()) {
        return;
    }

    if let Ok(text) = String::from_utf8(upstream.body.clone()) {
        merge_routes(state, &text).await;
        let routes = state.routes.read().await.routes.clone();
        let base = request_base(headers);
        upstream.body = rewrite_urls_with_base(&text, &routes, &base).into_bytes();
    }
}

struct AppState {
    client: UpstreamClient,
    source: String,
    routes: RwLock<RouteState>,
    maps_script_src: String,
    maps_key: Option<String>,
    maps_input: Option<PathBuf>,
}

struct RouteState {
    routes: Vec<RouteEntry>,
    digest_map: HashMap<String, String>,
}

impl RouteState {
    fn new(routes: Vec<RouteEntry>) -> Self {
        let digest_map = build_digest_map(&routes);
        Self { routes, digest_map }
    }
}

async fn merge_routes(state: &AppState, text: &str) {
    let discovered = build_route_entries(text);
    if discovered.is_empty() {
        return;
    }

    let mut route_state = state.routes.write().await;
    let mut added = 0usize;

    for route in discovered {
        if route_state
            .digest_map
            .values()
            .any(|domain| domain == &route.domain)
        {
            continue;
        }

        route_state
            .digest_map
            .insert(route.digest.clone(), route.domain.clone());
        route_state.routes.push(route);
        added += 1;
    }

    if added > 0 {
        debug!(
            routes_added = added,
            routes = route_state.routes.len(),
            "extended cobotmap route table"
        );
    }
}

fn is_text_response(content_type: Option<&str>) -> bool {
    let Some(content_type) = content_type else {
        return false;
    };

    content_type.starts_with("text/")
        || content_type.contains("javascript")
        || content_type.contains("json")
        || content_type.contains("xml")
        || content_type.contains("svg")
}

fn text_response(status: StatusCode, body: &str) -> Response {
    bytes_response(
        status,
        body.as_bytes().to_vec(),
        Some("text/plain; charset=utf-8"),
    )
}

fn bytes_response(status: StatusCode, body: Vec<u8>, content_type: Option<&str>) -> Response {
    let mut builder = Response::builder().status(status);
    if let Some(content_type) = content_type {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    builder.body(Body::from(body)).unwrap()
}

async fn trace_request(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = redact_request_uri(request.uri());
    trace!(%method, uri, "request started");

    let response = next.run(request).await;
    trace!(%method, uri, status = response.status().as_u16(), "request finished");

    response
}

fn redact_request_uri(uri: &Uri) -> String {
    let url = format!("http://cobotmap.local{uri}");
    redact_url(&url)
        .strip_prefix("http://cobotmap.local")
        .unwrap_or(uri.path())
        .to_string()
}

fn request_base(headers: &HeaderMap) -> String {
    headers
        .get("host")
        .and_then(|host| host.to_str().ok())
        .map(|host| format!("http://{host}"))
        .unwrap_or_default()
}

fn build_local_maps_script_src(api_key: &str) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query
        .append_pair("key", api_key)
        .append_pair("callback", "initMap");
    format!("{ENTRY_PATH}?{}", query.finish())
}
