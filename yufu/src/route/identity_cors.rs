use crate::{
    AppState,
    domain::identity::error::IdentityError,
    service::{
        identity::repository::{IdentityRepository, PgIdentityRepository},
        security::browser::single_header,
    },
};
use auth::security::budget::SecurityBudget;
use http::{HeaderMap, HeaderValue, Method};

pub(crate) fn is_identity_path(path: &str) -> bool {
    path.starts_with("/.well-known/")
        || path
            .split('/')
            .any(|s| matches!(s, "identity" | "account" | "developer" | "admin" | "oidc"))
}
fn methods(path: &str, api_prefix: &str) -> Option<&'static str> {
    if path == "/.well-known/openid-configuration" || path == "/.well-known/jwks.json" {
        Some("GET")
    } else if path == "/oidc/token" {
        Some("POST")
    } else if path == "/oidc/userinfo" || path == format!("{api_prefix}/oidc/userinfo") {
        Some("GET, POST")
    } else {
        None
    }
}
pub(crate) async fn protocol_headers(
    state: &AppState,
    path: &str,
    method: &Method,
    headers: &HeaderMap,
    budget: &SecurityBudget,
) -> Result<Option<HeaderMap>, IdentityError> {
    let Some(allowed_methods) = methods(path, &state.config.http.api_prefix) else {
        return Ok(None);
    };
    let Some(origin) = single_header(headers, "origin")? else {
        return Ok(None);
    };
    let parsed = url::Url::parse(origin).map_err(|_| IdentityError::InvalidInput)?;
    if parsed.origin().ascii_serialization() != origin
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(IdentityError::InvalidInput);
    }
    if origin
        == url::Url::parse(&state.issuer())
            .map_err(|_| IdentityError::DependencyUnavailable)?
            .origin()
            .ascii_serialization()
    {
        return Ok(None);
    }
    let preflight = method == Method::OPTIONS;
    let requested = if preflight {
        single_header(headers, "access-control-request-method")?
            .ok_or(IdentityError::InvalidInput)?
    } else {
        method.as_str()
    };
    if !allowed_methods.split(", ").any(|m| m == requested) {
        return Err(IdentityError::Forbidden);
    }
    if preflight
        && let Some(requested) = single_header(headers, "access-control-request-headers")?
        && requested.split(',').any(|h| {
            !matches!(
                h.trim().to_ascii_lowercase().as_str(),
                "authorization" | "content-type" | "accept"
            )
        })
    {
        return Err(IdentityError::Forbidden);
    }
    let repo =
        PgIdentityRepository::new(state.database.connection.clone(), state.security_clock()?);
    let mut tx = repo.begin(budget).await?;
    let permitted = tx.approved_spa_origin(origin).await;
    match permitted {
        Ok(true) => tx.commit().await?,
        Ok(false) => {
            let _ = tx.rollback().await;
            return Err(IdentityError::Forbidden);
        }
        Err(error) => {
            let _ = tx.rollback().await;
            return Err(error);
        }
    }
    budget.remaining()?;
    let mut cors = HeaderMap::new();
    cors.insert(
        "access-control-allow-origin",
        origin.parse().map_err(|_| IdentityError::InvalidInput)?,
    );
    cors.insert(
        "vary",
        HeaderValue::from_static(
            "Origin, Access-Control-Request-Method, Access-Control-Request-Headers",
        ),
    );
    if preflight {
        cors.insert(
            "access-control-allow-methods",
            HeaderValue::from_static(allowed_methods),
        );
        cors.insert(
            "access-control-allow-headers",
            HeaderValue::from_static("authorization, content-type, accept"),
        );
    }
    Ok(Some(cors))
}
