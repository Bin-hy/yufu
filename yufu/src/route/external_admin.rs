use super::identity_browser::IdentityRequest;
use crate::{
    AppState,
    domain::identity::{
        error::IdentityError,
        external::{
            ExternalProviderAdminView, ExternalProviderKey, ExternalProviderStats,
            RotateExternalProviderSecret, UpdateExternalProvider,
        },
    },
    error::AppError,
    service::identity::{
        browser,
        external::{ExternalProviderAdminService, StatsWindow},
        repository::PgIdentityRepository,
    },
};
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, PathRejection},
    },
    routing::{get, post, put},
};
use http::HeaderMap;
use serde::Deserialize;
use std::sync::Arc;

fn service(state: &AppState) -> Result<ExternalProviderAdminService, IdentityError> {
    let clock = state.security_clock()?;
    Ok(ExternalProviderAdminService::new(
        Arc::new(PgIdentityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        )),
        clock,
        state.external_cipher_arc(),
    ))
}

pub fn get_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/external-providers", get(list))
        .route("/admin/external-providers/{provider}", put(update))
        .route(
            "/admin/external-providers/{provider}/secret",
            post(rotate_secret),
        )
        .route("/admin/external-providers/stats", get(stats))
}

async fn list(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<Vec<ExternalProviderAdminView>>), AppError> {
    let result = async {
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let rows = service(&state)?.list(&secret, &context.budget).await?;
        Ok::<_, IdentityError>((browser::protected_headers(), Json(rows)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

async fn update(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<String>, PathRejection>,
    payload: Result<Json<UpdateExternalProvider>, JsonRejection>,
) -> Result<(HeaderMap, Json<ExternalProviderAdminView>), AppError> {
    let result = async {
        let Path(provider) = path.map_err(|_| IdentityError::InvalidInput)?;
        let provider = provider
            .parse::<ExternalProviderKey>()
            .map_err(|_| IdentityError::InvalidInput)?;
        let Json(payload) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let view = service(&state)?
            .update(&secret, provider, payload, context.id, &context.budget)
            .await?;
        state
            .reload_external_registry()
            .await
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok::<_, IdentityError>((browser::protected_headers(), Json(view)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

async fn rotate_secret(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    path: Result<Path<String>, PathRejection>,
    payload: Result<Json<RotateExternalProviderSecret>, JsonRejection>,
) -> Result<(HeaderMap, Json<ExternalProviderAdminView>), AppError> {
    let result = async {
        let Path(provider) = path.map_err(|_| IdentityError::InvalidInput)?;
        let provider = provider
            .parse::<ExternalProviderKey>()
            .map_err(|_| IdentityError::InvalidInput)?;
        let Json(payload) = payload.map_err(|_| IdentityError::InvalidInput)?;
        let clock = state.security_clock()?;
        browser::verify_write(
            &state.config,
            clock.as_ref(),
            &headers,
            None,
            &context.budget,
        )?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let view = service(&state)?
            .rotate_secret(&secret, provider, payload, context.id, &context.budget)
            .await?;
        state
            .reload_external_registry()
            .await
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        Ok::<_, IdentityError>((browser::protected_headers(), Json(view)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatsQuery {
    window: Option<String>,
}

async fn stats(
    State(state): State<AppState>,
    Extension(context): Extension<IdentityRequest>,
    headers: HeaderMap,
    Query(query): Query<StatsQuery>,
) -> Result<(HeaderMap, Json<ExternalProviderStats>), AppError> {
    let result = async {
        let window = StatsWindow::parse(query.window.as_deref())?;
        let secret = browser::read_sso_cookie(&state.config, &headers)?;
        let stats = service(&state)?
            .stats(&secret, window, &context.budget)
            .await?;
        Ok::<_, IdentityError>((browser::protected_headers(), Json(stats)))
    }
    .await;
    result.map_err(|reason| AppError::identity(reason, context.id))
}
