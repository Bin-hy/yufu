use axum::{Json, response::IntoResponse, routing::MethodRouter};
use http::{Method, StatusCode, Uri};
use std::borrow::Cow;

#[cfg(feature = "webui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../assets/cobotusr/"]
struct Assets;

#[cfg(feature = "webui")]
fn load_asset(path: &str) -> Option<Cow<'static, [u8]>> {
    Assets::get(path).map(|asset| asset.data)
}

#[cfg(not(feature = "webui"))]
fn load_asset(_: &str) -> Option<Cow<'static, [u8]>> {
    None
}

pub(crate) fn fallback(api_prefix: String) -> MethodRouter {
    fallback_with_assets(api_prefix, load_asset)
}

fn fallback_with_assets(
    api_prefix: String,
    load: impl Fn(&str) -> Option<Cow<'static, [u8]>> + Clone + Send + Sync + 'static,
) -> MethodRouter {
    axum::routing::any(move |method: Method, uri: Uri| {
        let response = static_response(method, uri, &api_prefix, &load);
        async move { response }
    })
}

fn not_found() -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "message": "Not found" })),
    )
        .into_response()
}

fn static_response(
    method: Method,
    uri: Uri,
    api_prefix: &str,
    load: impl Fn(&str) -> Option<Cow<'static, [u8]>>,
) -> axum::response::Response {
    let prefix = api_prefix.trim_end_matches('/');
    let is_api_path = uri.path() == prefix
        || uri
            .path()
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'));
    // API misses and mutations must never look like successful SPA responses.
    if is_api_path || !matches!(method, Method::GET | Method::HEAD) {
        return not_found();
    }

    let path = uri.path().trim_start_matches('/');
    let file_path = if path.is_empty() { "index.html" } else { path };
    if let Some(content) = load(file_path) {
        let mime = mime_guess::from_path(file_path).first_or_octet_stream();
        return ([(http::header::CONTENT_TYPE, mime.as_ref())], content).into_response();
    }

    // Missing JavaScript/CSS files are resource errors, not navigation requests.
    if path.starts_with("assets/") {
        return not_found();
    }
    match load("index.html") {
        Some(index) => ([(http::header::CONTENT_TYPE, "text/html")], index).into_response(),
        None => not_found(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, routing::post};
    use http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn app(prefix: &str) -> Router {
        Router::new()
            .nest(
                prefix,
                Router::new().route(
                    "/login",
                    post(|| async { Json(serde_json::json!({ "access_token": "test-only" })) }),
                ),
            )
            .fallback_service(fallback_with_assets(prefix.into(), |path| match path {
                "index.html" => Some(Cow::Borrowed(b"<!doctype html><html>test SPA</html>")),
                "assets/app.js" => Some(Cow::Borrowed(b"console.log('test')")),
                _ => None,
            }))
    }

    async fn response(prefix: &str, method: Method, path: &str) -> axum::response::Response {
        app(prefix)
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_fallback_unknown_api_returns_json_404_with_configured_prefix() {
        for prefix in ["/api/v1", "/gateway/api"] {
            for path in [prefix.to_string(), format!("{prefix}/missing")] {
                for method in [Method::GET, Method::POST] {
                    let res = response(prefix, method, &path).await;
                    assert_eq!(res.status(), StatusCode::NOT_FOUND);
                    assert_eq!(
                        res.headers()[http::header::CONTENT_TYPE],
                        "application/json"
                    );
                    let body = res.into_body().collect().await.unwrap().to_bytes();
                    assert_eq!(
                        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["message"],
                        "Not found"
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn test_fallback_mutation_to_spa_path_never_returns_html() {
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            let res = response("/api/v1", method, "/login").await;
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                res.headers()[http::header::CONTENT_TYPE],
                "application/json"
            );
        }
    }

    #[tokio::test]
    async fn test_fallback_spa_deep_links_still_return_index() {
        for path in [
            "/",
            "/login",
            "/users?page_number=3",
            "/oidc/login",
            "/oidc/consent",
            "/api/v10/page",
        ] {
            let res = response("/api/v1", Method::GET, path).await;
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(res.headers()[http::header::CONTENT_TYPE], "text/html");
            let body = res.into_body().collect().await.unwrap().to_bytes();
            assert!(body.starts_with(b"<!doctype html>"));
        }
    }

    #[tokio::test]
    async fn test_fallback_head_preserves_headers_without_body() {
        let res = response("/api/v1", Method::HEAD, "/users").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[http::header::CONTENT_TYPE], "text/html");
        assert!(
            res.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_fallback_assets_keep_mime_and_missing_assets_return_404() {
        let res = response("/api/v1", Method::GET, "/assets/app.js").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()[http::header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .contains("javascript")
        );
        assert_eq!(
            response("/api/v1", Method::GET, "/assets/missing.js")
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn test_fallback_does_not_override_login_or_method_mismatch() {
        let res = response("/api/v1", Method::POST, "/api/v1/login").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()[http::header::CONTENT_TYPE],
            "application/json"
        );
        assert_eq!(
            response("/api/v1", Method::GET, "/api/v1/login")
                .await
                .status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[tokio::test]
    async fn test_fallback_without_assets_returns_404() {
        let app = Router::new().fallback_service(fallback_with_assets("/api/v1".into(), |_| None));
        let res = app
            .oneshot(Request::get("/login").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
