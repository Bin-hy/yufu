use crate::IdType;
use crate::entity::{application, permission, role, role_permission, user, user_permission};
use crate::error::AppError;
use crate::result::Result;

use sea_orm::{DatabaseConnection, EntityTrait};

/// Resolve user roles (names) and permission names for a given target application.
///
/// Looks up `aud` as the application's `app_key`. If no application with that
/// key exists (e.g. for resource-app's own internal tokens), returns empty sets.
///
/// Legacy metadata only: a string-name list cannot express resource scopes or
/// overlapping denies. Strict authorization uses the structured primary snapshot,
/// never this list as evidence. Legacy issuance is migrated by its lifecycle task.
/// Resolution order (permission):
///   1. `user_permission` explicit allow/deny overrides everything (including superadmin)
///   2. superadmin role implicitly holds every permission of the application
///   3. `role_permission` via the user's role(s)
pub async fn resolve_user_permissions(
    db: &DatabaseConnection,
    user_id: IdType,
    aud: &str,
) -> Result<(Vec<String>, Vec<String>)> {
    let user = user::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user {user_id} not found")))?;

    let user_role = role::Entity::find_by_id(user.role_id)
        .one(db)
        .await?
        .map(|r| r.name)
        .unwrap_or_default();

    let is_superadmin = user_role == role::RoleEnum::Superadmin.get_name();
    let roles = vec![user_role];

    let Some(app) = application::Entity::get_by_app_key(db, aud).await? else {
        return Ok((roles, vec![]));
    };

    let all_perms = permission::Entity::list_by_application(db, app.id).await?;

    // user_permission overrides (allow/deny) — evaluated BEFORE the superadmin
    // short-circuit: an explicit deny wins even over superadmin role grants,
    // matching the legacy authority system (rbac.rs).
    let user_overrides: Vec<user_permission::Model> =
        user_permission::Entity::list_by_user(db, user_id).await?;

    let mut denied_ids: std::collections::HashSet<IdType> = std::collections::HashSet::new();
    let mut allowed_ids: std::collections::HashSet<IdType> = std::collections::HashSet::new();

    for ov in &user_overrides {
        if ov.allow {
            allowed_ids.insert(ov.permission_id);
        } else {
            denied_ids.insert(ov.permission_id);
        }
    }

    // Superadmin implicitly holds every permission of the application except
    // explicit `user_permission` denies (mirrors
    // `AuthorityService::ensure_sadmin_authorities` for the legacy authority system).
    if is_superadmin {
        let names = all_perms
            .iter()
            .filter(|p| !denied_ids.contains(&p.id))
            .map(|p| p.name.clone())
            .collect();
        return Ok((roles, names));
    }

    // role_permission via user's role
    let role_perm_ids: Vec<IdType> =
        role_permission::Entity::list_permission_ids_by_role(db, user.role_id).await?;
    let role_perm_set: std::collections::HashSet<IdType> = role_perm_ids.into_iter().collect();

    let mut permission_names: Vec<String> = Vec::new();
    for perm in &all_perms {
        // explicit user deny overrides everything
        if denied_ids.contains(&perm.id) {
            continue;
        }
        // explicit user allow OR role-based grant
        if allowed_ids.contains(&perm.id) || role_perm_set.contains(&perm.id) {
            permission_names.push(perm.name.clone());
        }
    }

    Ok((roles, permission_names))
}

/// Legacy subject-only PDP shape; retained to return an explicit unauthorized
/// response during migration. None of these fields is authentication evidence.
#[derive(serde::Deserialize)]
pub struct EnforceRequest {
    /// Application key (e.g. "cobotrix").
    pub application: String,
    /// Untrusted legacy subject id; never a substitute for verified evidence.
    pub subject: IdType,
    /// Action verb (e.g. "read", "submit").
    pub action: String,
    /// Resource type (e.g. "device", "task"). Optional if action-only permission.
    #[serde(default)]
    pub resource_type: Option<String>,
    /// Resource identifier (e.g. "example-resource:lift"). Optional if no object match needed.
    #[serde(default)]
    pub resource_id: Option<String>,
}

/// PDP enforcement response.
#[derive(serde::Serialize)]
pub struct EnforceResponse {
    pub allow: bool,
    pub reason: String,
}

/// Legacy subject-only requests carry no verified subject evidence or signed
/// bounds. Keep the input contract long enough to return an explicit 401, but
/// never turn a caller-provided user id or a missing resource into authorization.
/// New decisions use security::authorization with an AuthenticatedContext.
pub async fn enforce(_db: &DatabaseConnection, _req: &EnforceRequest) -> Result<EnforceResponse> {
    Err(AppError::Unauthorized(
        "verified subject evidence required".into(),
    ))
}

/// Canonical operation, independent of a rule's display name. A full action can
/// bind task:execute to a tool resource; verb-only legacy actions remain literal.
pub(super) fn registered_operation(
    row: &permission::Model,
) -> Option<auth::security::policy::OperationId> {
    let name = if row.action.contains(':') {
        row.action.clone()
    } else {
        format!("{}:{}", row.resource_type.as_ref()?, row.action)
    };
    auth::security::policy::OperationId::new(name).ok()
}

/// Finite typed selectors replace delimiter-globs. Missing/ambiguous legacy
/// boundaries need an administrator correction; they never become wildcards.
pub(super) fn resource_selector(
    row: &permission::Model,
    application: &str,
) -> std::result::Result<
    auth::security::policy::ResourceSelector,
    auth::security::error::SecurityError,
> {
    use auth::security::{
        error::SecurityError,
        policy::{ResourceKind, ResourceScope, ResourceSelector},
    };
    let invalid = || SecurityError::DependencyUnavailable;
    let raw = row
        .resource_pattern
        .as_deref()
        .filter(|s| s.len() <= 16_384)
        .ok_or_else(invalid)?;
    let selector: ResourceSelector = serde_json::from_str(raw).map_err(|_| invalid())?;
    let kind: ResourceKind =
        serde_json::from_value(row.resource_type.clone().ok_or_else(invalid)?.into())
            .map_err(|_| invalid())?;
    let selector_kind = match selector.scope() {
        ResourceScope::Exact { target } => target.kind(),
        ResourceScope::Application { kind }
        | ResourceScope::Gateway { kind, .. }
        | ResourceScope::Device { kind, .. } => *kind,
    };
    if selector.application() != application || kind != selector_kind {
        return Err(invalid());
    }
    Ok(selector)
}
