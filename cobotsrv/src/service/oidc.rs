use crate::entity::{oauth_authorization_code, oauth_client, oauth_scope, oauth_token};
use crate::error::AppError;
use crate::result::Result;
use crate::{AppState, IdType};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use rand::RngCore;
use rand::rngs::OsRng;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sha2::{Digest, Sha256};

pub struct OidcService;

// ---------------------------------------------------------------------------
// Client helpers (moved from entity/oauth_client.rs Model impl)
// ---------------------------------------------------------------------------
impl OidcService {
    pub fn client_has_redirect(client: &oauth_client::Model, uri: &str) -> bool {
        client
            .redirect_uris
            .split(',')
            .map(|s| s.trim())
            .any(|v| v == uri)
    }

    pub fn client_allows_scopes(client: &oauth_client::Model, scopes: &[String]) -> bool {
        let allowed: Vec<&str> = client.allowed_scopes.split(',').map(|s| s.trim()).collect();
        scopes.iter().all(|s| allowed.contains(&s.as_str()))
    }

    pub fn client_allows_grant_type(client: &oauth_client::Model, grant_type: &str) -> bool {
        client
            .grant_types
            .split(',')
            .map(|s| s.trim())
            .any(|v| v == grant_type)
    }

    pub fn client_verify_secret(client: &oauth_client::Model, secret: &str) -> bool {
        bcrypt::verify(secret, &client.client_secret_hash).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Client authentication
// ---------------------------------------------------------------------------
impl OidcService {
    /// Authenticate an OAuth client by client_id + client_secret.
    pub async fn authenticate_client(
        db: &DatabaseConnection,
        client_id: Option<&str>,
        client_secret: Option<&str>,
    ) -> Result<oauth_client::Model> {
        let cid = client_id.ok_or(AppError::BadRequest("missing client_id".to_string()))?;
        let secret =
            client_secret.ok_or(AppError::BadRequest("missing client_secret".to_string()))?;
        let client = oauth_client::Entity::get_by_client_id(db, cid)
            .await?
            .ok_or(AppError::Unauthorized("invalid client_id".to_string()))?;
        if !Self::client_verify_secret(&client, secret) {
            return Err(AppError::Unauthorized("invalid client_secret".to_string()));
        }
        Ok(client)
    }
}

// ---------------------------------------------------------------------------
// Client registration
// ---------------------------------------------------------------------------
impl OidcService {
    /// Register a new OAuth client. Returns (Model, plaintext_secret).
    pub async fn register_client(
        db: &DatabaseConnection,
        client_id: String,
        name: String,
        redirect_uris: Vec<String>,
        allowed_scopes: Vec<String>,
        grant_types: Vec<String>,
    ) -> Result<(oauth_client::Model, String)> {
        let now = Utc::now().naive_utc();

        // Generate a random client_secret
        let mut buf = [0u8; 32];
        OsRng.fill_bytes(&mut buf);
        let client_secret = hex::encode(buf);
        let secret_hash = bcrypt::hash(&client_secret, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("hash error: {}", e)))?;

        let am = oauth_client::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            client_id: Set(client_id),
            name: Set(name),
            redirect_uris: Set(redirect_uris.join(",")),
            allowed_scopes: Set(allowed_scopes.join(",")),
            client_secret_hash: Set(secret_hash),
            grant_types: Set(grant_types.join(",")),
            create_time: Set(now),
            update_time: Set(now),
        };
        let inserted = am.insert(db).await.map_err(AppError::from)?;
        Ok((inserted, client_secret))
    }
}

// ---------------------------------------------------------------------------
// Scope management
// ---------------------------------------------------------------------------
impl OidcService {
    pub async fn add_scope(
        db: &DatabaseConnection,
        name: String,
        description: Option<String>,
    ) -> Result<oauth_scope::Model> {
        let now = Utc::now().naive_utc();
        let am = oauth_scope::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            name: Set(name),
            description: Set(description),
            create_time: Set(now),
            update_time: Set(now),
        };
        let inserted = am.insert(db).await.map_err(AppError::from)?;
        Ok(inserted)
    }

    pub async fn list_scopes(db: &DatabaseConnection) -> Result<Vec<oauth_scope::Model>> {
        let list = oauth_scope::Entity::find()
            .all(db)
            .await
            .map_err(AppError::from)?;
        Ok(list)
    }

    pub async fn scopes_exist(db: &DatabaseConnection, names: &[String]) -> Result<bool> {
        Ok(oauth_scope::Entity::exists_by_names(db, names).await?)
    }
}

// ---------------------------------------------------------------------------
// PKCE verification
// ---------------------------------------------------------------------------
impl OidcService {
    /// Verify PKCE code_verifier against code_challenge.
    pub fn verify_pkce(
        code_challenge: &str,
        code_challenge_method: &str,
        code_verifier: &str,
    ) -> bool {
        match code_challenge_method {
            "plain" => code_challenge == code_verifier,
            "S256" => {
                let digest = Sha256::digest(code_verifier.as_bytes());
                let challenge = URL_SAFE_NO_PAD.encode(digest);
                code_challenge == challenge
            }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Authorization code operations
// ---------------------------------------------------------------------------

/// Parameters for creating an authorization code.
pub struct CreateAuthCodeParams {
    pub user_id: IdType,
    pub client_id: String,
    pub redirect_uri: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
}

impl OidcService {
    /// Generate and persist an authorization code (5 min TTL).
    pub async fn create_authorization_code(
        db: &DatabaseConnection,
        params: CreateAuthCodeParams,
    ) -> Result<String> {
        let mut buf = [0u8; 32];
        OsRng.fill_bytes(&mut buf);
        let code = hex::encode(buf);

        let now = Utc::now().naive_utc();
        let am = oauth_authorization_code::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            code: Set(code.clone()),
            user_id: Set(params.user_id),
            client_id: Set(params.client_id),
            redirect_uri: Set(params.redirect_uri),
            code_challenge: Set(params.code_challenge),
            code_challenge_method: Set(params.code_challenge_method),
            scope: Set(params.scope),
            state: Set(params.state),
            nonce: Set(params.nonce),
            used: Set(false),
            expires_at: Set(now + Duration::minutes(5)),
            create_time: Set(now),
        };
        am.insert(db).await?;
        Ok(code)
    }

    /// Take (consume) an authorization code: find it, mark as used, return it.
    /// Returns None if not found or already used.
    pub async fn take_authorization_code(
        db: &DatabaseConnection,
        code: &str,
    ) -> Result<Option<oauth_authorization_code::Model>> {
        let rec = oauth_authorization_code::Entity::find_unused_by_code(db, code).await?;
        if let Some(rec) = rec {
            let mut am: oauth_authorization_code::ActiveModel = rec.clone().into();
            am.used = Set(true);
            am.update(db).await?;
            Ok(Some(rec))
        } else {
            Ok(None)
        }
    }

    /// Delete expired authorization codes.
    pub async fn cleanup_expired_codes(db: &DatabaseConnection) -> Result<u64> {
        let now = Utc::now().naive_utc();
        let res = oauth_authorization_code::Entity::delete_many()
            .filter(oauth_authorization_code::Column::ExpiresAt.lt(now))
            .exec(db)
            .await?;
        Ok(res.rows_affected)
    }
}

// ---------------------------------------------------------------------------
// Token operations (revoke, issue, refresh)
// ---------------------------------------------------------------------------
impl OidcService {
    /// Revoke a token record by its refresh_token value.
    pub async fn revoke_by_refresh_token(db: &DatabaseConnection, rt: &str) -> Result<()> {
        if let Some(rec) = oauth_token::Entity::find_by_refresh_token(db, rt).await? {
            let mut am: oauth_token::ActiveModel = rec.into();
            am.revoked = Set(true);
            am.update(db).await?;
        }
        Ok(())
    }

    /// Revoke a token record by its access_token value.
    pub async fn revoke_by_access_token(db: &DatabaseConnection, at: &str) -> Result<()> {
        if let Some(rec) = oauth_token::Entity::find_by_access_token(db, at).await? {
            let mut am: oauth_token::ActiveModel = rec.into();
            am.revoked = Set(true);
            am.update(db).await?;
        }
        Ok(())
    }

    fn generate_refresh_token_value() -> String {
        let mut buf = [0u8; 32];
        OsRng.fill_bytes(&mut buf);
        hex::encode(buf)
    }

    /// Issue tokens for authorization_code grant.
    pub async fn issue_authorization_code_tokens(
        state: &AppState,
        entry: &oauth_authorization_code::Model,
        client: &oauth_client::Model,
    ) -> Result<(String, String, String)> {
        let base = get_issuer_base(state);
        let access_token = state
            .auth_state
            .token_manager
            .generate_token::<IdType>(entry.user_id)?;

        let id_token = state.id_token_signer.sign_id_token::<IdType>(
            entry.user_id,
            &base,
            Some(&client.client_id),
            entry.nonce.as_deref(),
            Some(&access_token),
        )?;

        let refresh_token = Self::generate_refresh_token_value();
        let now = Utc::now().naive_utc();
        let access_expires = now + Duration::seconds(state.config.auth.exp_seconds);
        let refresh_expires = now + Duration::seconds(state.config.auth.refresh_exp_seconds);

        let new_rec = oauth_token::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            user_id: Set(entry.user_id),
            client_id: Set(client.client_id.clone()),
            access_token: Set(access_token.clone()),
            id_token: Set(id_token.clone()),
            refresh_token: Set(refresh_token.clone()),
            scope: Set(entry.scope.clone()),
            token_type: Set("Bearer".to_string()),
            create_time: Set(now),
            access_expires_at: Set(access_expires),
            refresh_expires_at: Set(refresh_expires),
            update_time: Set(now),
            revoked: Set(false),
        };
        new_rec.insert(&state.database.connection).await?;

        Ok((access_token, id_token, refresh_token))
    }

    /// Issue tokens for refresh_token grant (with rotation).
    pub async fn issue_refresh_tokens(
        state: &AppState,
        old_rt: &str,
        rec: &oauth_token::Model,
    ) -> Result<(String, String, String)> {
        let db = &state.database.connection;
        let base = get_issuer_base(state);

        let access_token = state
            .auth_state
            .token_manager
            .generate_token::<IdType>(rec.user_id)?;

        let id_token = state.id_token_signer.sign_id_token::<IdType>(
            rec.user_id,
            &base,
            Some(&rec.client_id),
            None,
            Some(&access_token),
        )?;

        // Rotate: revoke old refresh_token, issue new one
        Self::revoke_by_refresh_token(db, old_rt).await?;
        let new_refresh_token = Self::generate_refresh_token_value();
        let now = Utc::now().naive_utc();
        let access_expires = now + Duration::seconds(state.config.auth.exp_seconds);
        let refresh_expires = now + Duration::seconds(state.config.auth.refresh_exp_seconds);

        let new_rec = oauth_token::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            user_id: Set(rec.user_id),
            client_id: Set(rec.client_id.clone()),
            access_token: Set(access_token.clone()),
            id_token: Set(id_token.clone()),
            refresh_token: Set(new_refresh_token.clone()),
            scope: Set(rec.scope.clone()),
            token_type: Set("Bearer".to_string()),
            create_time: Set(now),
            access_expires_at: Set(access_expires),
            refresh_expires_at: Set(refresh_expires),
            update_time: Set(now),
            revoked: Set(false),
        };
        new_rec.insert(db).await?;

        Ok((access_token, id_token, new_refresh_token))
    }

    /// Issue access_token for client_credentials grant (M2M, no user context).
    pub fn issue_client_credentials_token(
        state: &AppState,
        client: &oauth_client::Model,
    ) -> Result<String> {
        let access_token = state
            .auth_state
            .token_manager
            .generate_token::<String>(client.client_id.clone())?;
        Ok(access_token)
    }
}

// ---------------------------------------------------------------------------
// Introspection
// ---------------------------------------------------------------------------
impl OidcService {
    /// Look up a token record by access_token or refresh_token.
    pub async fn find_token_record(
        db: &DatabaseConnection,
        token: &str,
    ) -> Result<Option<oauth_token::Model>> {
        if let Some(rec) = oauth_token::Entity::find_by_access_token(db, token).await? {
            return Ok(Some(rec));
        }
        Ok(oauth_token::Entity::find_by_refresh_token(db, token).await?)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
pub fn get_issuer_base(state: &AppState) -> String {
    if state.config.http.public.is_empty() {
        format!("http://{}", state.config.http.listen)
    } else {
        state.config.http.public.clone()
    }
}
