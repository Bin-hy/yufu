use crate::IdType;
use crate::entity::{application, permission, role, role_permission, user, user_permission};
use crate::error::AppError;
use crate::result::Result;

use sea_orm::{DatabaseConnection, EntityTrait};

/// Resolve user roles (names) and permission names for a given target application.
///
/// Looks up `aud` as the application's `app_key`. If no application with that
/// key exists (e.g. for cobotsrv's own internal tokens), returns empty sets.
///
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

/// PDP enforcement request.
#[derive(serde::Deserialize)]
pub struct EnforceRequest {
    /// Application key (e.g. "cobotrix").
    pub application: String,
    /// Subject user id.
    pub subject: IdType,
    /// Action verb (e.g. "read", "submit").
    pub action: String,
    /// Resource type (e.g. "device", "task"). Optional if action-only permission.
    #[serde(default)]
    pub resource_type: Option<String>,
    /// Resource identifier (e.g. "cobotlet-0:lift"). Optional if no object match needed.
    #[serde(default)]
    pub resource_id: Option<String>,
}

/// PDP enforcement response.
#[derive(serde::Serialize)]
pub struct EnforceResponse {
    pub allow: bool,
    pub reason: String,
}

/// PDP enforcement: check whether `subject` may perform `action` on
/// `resource_type:resource_id` within the given application.
///
/// Pattern matching on `resource_pattern` uses simple glob-style matching
/// (`*` matches any sequence, `?` matches any single character).
pub async fn enforce(db: &DatabaseConnection, req: &EnforceRequest) -> Result<EnforceResponse> {
    let Some(app) = application::Entity::get_by_app_key(db, &req.application).await? else {
        return Ok(EnforceResponse {
            allow: false,
            reason: format!("unknown application: {}", req.application),
        });
    };

    let user = match user::Entity::find_by_id(req.subject).one(db).await? {
        Some(u) => u,
        None => {
            return Ok(EnforceResponse {
                allow: false,
                reason: format!("unknown subject: {}", req.subject),
            });
        }
    };

    let all_perms = permission::Entity::list_by_application(db, app.id).await?;

    // user_permission overrides — evaluated BEFORE the superadmin short-circuit:
    // an explicit deny wins even over superadmin, matching the legacy authority
    // system (rbac.rs).
    let user_overrides: Vec<user_permission::Model> =
        user_permission::Entity::list_by_user(db, req.subject).await?;

    let denied_ids: std::collections::HashSet<IdType> = user_overrides
        .iter()
        .filter(|o| !o.allow)
        .map(|o| o.permission_id)
        .collect();
    let allowed_ids: std::collections::HashSet<IdType> = user_overrides
        .iter()
        .filter(|o| o.allow)
        .map(|o| o.permission_id)
        .collect();

    // Superadmin is allowed everything EXCEPT what an explicit `user_permission`
    // deny covers (mirrors the legacy authority system's `ensure_sadmin_authorities`
    // plus its ABAC override precedence).
    let is_superadmin = role::Entity::find_by_id(user.role_id)
        .one(db)
        .await?
        .map(|r| r.name == role::RoleEnum::Superadmin.get_name())
        .unwrap_or(false);
    if is_superadmin {
        if let Some(perm) = all_perms
            .iter()
            .find(|p| denied_ids.contains(&p.id) && perm_matches(p, req))
        {
            return Ok(EnforceResponse {
                allow: false,
                reason: format!(
                    "explicit user deny on permission #{} ({})",
                    perm.id, perm.name
                ),
            });
        }
        return Ok(EnforceResponse {
            allow: true,
            reason: "superadmin".to_string(),
        });
    }

    let role_perm_ids: Vec<IdType> =
        role_permission::Entity::list_permission_ids_by_role(db, user.role_id).await?;
    let role_perm_set: std::collections::HashSet<IdType> = role_perm_ids.into_iter().collect();

    for perm in &all_perms {
        // explicit deny
        if denied_ids.contains(&perm.id) {
            continue;
        }
        // must be granted
        if !allowed_ids.contains(&perm.id) && !role_perm_set.contains(&perm.id) {
            continue;
        }
        if !perm_matches(perm, req) {
            continue;
        }
        return Ok(EnforceResponse {
            allow: true,
            reason: format!("matched permission #{} ({})", perm.id, perm.name),
        });
    }

    Ok(EnforceResponse {
        allow: false,
        reason: "no matching permission".to_string(),
    })
}

/// Whether a permission row matches the request's action / resource_type /
/// resource_pattern. Action matches either the verb ("read") or the full
/// permission name ("device:read").
fn perm_matches(perm: &permission::Model, req: &EnforceRequest) -> bool {
    if perm.action != req.action && perm.name != req.action {
        return false;
    }
    // resource_type must match if specified
    if let (Some(req_rt), Some(perm_rt)) = (&req.resource_type, &perm.resource_type)
        && req_rt != perm_rt
    {
        return false;
    }
    // resource_pattern match
    if let (Some(rid), Some(pattern)) = (&req.resource_id, &perm.resource_pattern)
        && !glob_match(pattern, rid)
    {
        return false;
    }
    true
}

/// Simple glob match: `*` matches any sequence (including empty), `?` matches exactly one char.
fn glob_match(pattern: &str, input: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let inp: Vec<char> = input.chars().collect();
    let (pi, ii) = (pat.len(), inp.len());
    let mut dp = vec![vec![false; ii + 1]; pi + 1];
    dp[0][0] = true;
    for (i, &pc) in pat.iter().enumerate() {
        if pc == '*' {
            dp[i + 1][0] = dp[i][0];
        } else {
            break;
        }
    }
    for i in 0..pi {
        for j in 0..ii {
            if pat[i] == '*' {
                dp[i + 1][j + 1] = dp[i][j + 1] || dp[i + 1][j];
            } else if pat[i] == '?' || pat[i] == inp[j] {
                dp[i + 1][j + 1] = dp[i][j];
            }
        }
    }
    dp[pi][ii]
}
