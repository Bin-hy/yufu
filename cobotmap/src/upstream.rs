use crate::{ProxyConfig, build_maps_source_url, load_maps_from_file, redact_url};
use axum::{
    body::Bytes,
    http::{
        HeaderMap, HeaderValue, Method, Request, Uri,
        header::{ACCEPT_ENCODING, CONTENT_TYPE, HOST, USER_AGENT},
    },
};
use http_body_util::{BodyExt, Full};
use hyper::{Response, body::Incoming};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{
        Client,
        connect::{HttpConnector, proxy::SocksV5},
    },
    rt::TokioExecutor,
};
use std::{path::Path, time::Duration};
use tracing::{debug, trace};
use url::Url;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const USER_AGENT_VALUE: &str = "cobotmap/0.0.0";

type UpstreamResult<T> = Result<T, String>;
type RequestBody = Full<Bytes>;
type DirectConnector = HttpsConnector<HttpConnector>;
type DirectClient = Client<DirectConnector, RequestBody>;
type SocksProxyConnector = HttpsConnector<SocksV5<HttpConnector>>;
type SocksProxyClient = Client<SocksProxyConnector, RequestBody>;

#[derive(Clone, Debug)]
pub struct UpstreamResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub struct UpstreamClient {
    direct: DirectClient,
    proxy: Option<SocksProxyClient>,
}

pub fn build_client(proxy: Option<&ProxyConfig>) -> UpstreamResult<UpstreamClient> {
    if let Some(proxy) = proxy {
        debug!(
            proxy = proxy.url().as_str(),
            "building upstream client with SOCKS5 proxy"
        );
    } else {
        debug!("building upstream client without proxy");
    }

    Ok(UpstreamClient {
        direct: build_direct_client()?,
        proxy: build_proxy_client(proxy)?,
    })
}

pub async fn load_maps_source(
    input_file: Option<&Path>,
    api_key: Option<&str>,
    client: &UpstreamClient,
) -> UpstreamResult<String> {
    match input_file {
        Some(path) => load_maps_from_file(path),
        None => {
            let key = api_key
                .ok_or("missing maps.key, or use maps.input for offline mode".to_string())?;
            let response =
                fetch_request(client, Method::GET, &build_maps_source_url(key), &[], None).await?;
            String::from_utf8(response.body)
                .map_err(|err| format!("source response was not valid UTF-8: {err}"))
        }
    }
}

pub async fn fetch_direct_url(url: &str) -> UpstreamResult<UpstreamResponse> {
    trace!(url = redact_url(url), "fetching direct verification URL");
    let client = UpstreamClient {
        direct: build_direct_client()?,
        proxy: None,
    };

    fetch_request(&client, Method::GET, url, &[], None).await
}

pub async fn fetch_request(
    client: &UpstreamClient,
    method: Method,
    url: &str,
    body: &[u8],
    headers: Option<&HeaderMap>,
) -> UpstreamResult<UpstreamResponse> {
    let uri: Uri = url
        .parse()
        .map_err(|err| format!("bad upstream URL: {err}"))?;
    let uses_proxy = client.proxy.is_some();
    let upstream_url = redact_url(url);
    trace!(%method, upstream_url, uses_proxy, "upstream request started");
    let request = build_request(method.clone(), uri.clone(), body, headers)?;

    let response = match client.proxy.as_ref() {
        Some(proxy) => timeout_request(proxy.request(request)).await?,
        None => timeout_request(client.direct.request(request)).await?,
    };

    let response = into_upstream_response(response).await?;
    trace!(%method, upstream_url, uses_proxy, status = response.status, "upstream request finished");
    Ok(response)
}

fn build_proxy_client(proxy: Option<&ProxyConfig>) -> UpstreamResult<Option<SocksProxyClient>> {
    proxy.map(build_socks_proxy_client).transpose()
}

fn build_direct_client() -> UpstreamResult<DirectClient> {
    let connector = build_https_connector()?;
    Ok(Client::builder(TokioExecutor::new()).build(connector))
}

fn build_socks_proxy_client(proxy: &ProxyConfig) -> UpstreamResult<SocksProxyClient> {
    let mut connector = HttpConnector::new();
    connector.set_connect_timeout(Some(CONNECT_TIMEOUT));

    let proxy_url = proxy.url();
    let mut socks = SocksV5::new(rebuild_socks_proxy_uri(proxy_url)?, connector)
        .local_dns(proxy_url.scheme() == "socks5");

    if !proxy_url.username().is_empty() {
        let password = proxy_url.password().unwrap_or_default();
        socks = socks.with_auth(proxy_url.username().to_string(), password.to_string());
    }

    let connector = wrap_https_connector(socks)?;
    Ok(Client::builder(TokioExecutor::new()).build(connector))
}

fn build_https_connector() -> UpstreamResult<DirectConnector> {
    let mut connector = HttpConnector::new();
    connector.enforce_http(false);
    connector.set_connect_timeout(Some(CONNECT_TIMEOUT));
    wrap_https_connector(connector)
}

fn wrap_https_connector<C>(connector: C) -> UpstreamResult<HttpsConnector<C>> {
    let connector = HttpsConnectorBuilder::new()
        .with_native_roots()
        .map_err(|err| format!("failed to load native root certificates: {err}"))?
        .https_or_http()
        .enable_http1()
        .wrap_connector(connector);

    Ok(connector)
}

fn rebuild_socks_proxy_uri(url: &Url) -> UpstreamResult<Uri> {
    let host = url.host_str().ok_or("proxy URL missing host".to_string())?;
    let port = url.port().unwrap_or(1080);
    let authority = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };

    format!("http://{authority}")
        .parse()
        .map_err(|err| format!("bad proxy URL: {err}"))
}

fn build_request(
    method: Method,
    uri: Uri,
    body: &[u8],
    headers: Option<&HeaderMap>,
) -> UpstreamResult<Request<RequestBody>> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

    if let Some(headers) = headers {
        for (name, value) in headers.iter() {
            if *name == HOST || *name == ACCEPT_ENCODING {
                continue;
            }
            if value.as_bytes() == b"cobotmap/0.0.0" {
                continue;
            }
            builder = builder.header(name, value);
        }
    }

    builder
        .body(if body.is_empty() {
            Full::new(Bytes::new())
        } else {
            Full::new(Bytes::copy_from_slice(body))
        })
        .map_err(|err| format!("failed to build upstream request: {err}"))
}

async fn timeout_request<F>(future: F) -> UpstreamResult<Response<Incoming>>
where
    F: std::future::Future<Output = Result<Response<Incoming>, hyper_util::client::legacy::Error>>,
{
    tokio::time::timeout(REQUEST_TIMEOUT, future)
        .await
        .map_err(|_| {
            format!(
                "upstream request timed out after {}s",
                REQUEST_TIMEOUT.as_secs()
            )
        })?
        .map_err(|err| format!("upstream request failed: {err}; debug={err:?}"))
}

async fn into_upstream_response(response: Response<Incoming>) -> UpstreamResult<UpstreamResponse> {
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|err| format!("failed to read upstream body: {err}"))?
        .to_bytes()
        .to_vec();

    Ok(UpstreamResponse {
        status,
        content_type,
        body,
    })
}
