use crate::entity::{
    oauth_authorization_code, oauth_client, oauth_consent, oauth_scope, oauth_token,
};
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
        if client.public_client {
            return false;
        }
        bcrypt::verify(secret, &client.client_secret_hash).unwrap_or(false)
    }

    pub fn public_client_requires_s256_pkce(
        client: &oauth_client::Model,
        code_challenge: Option<&str>,
        code_challenge_method: &str,
    ) -> bool {
        !client.public_client
            || (code_challenge.is_some_and(|value| !value.is_empty())
                && code_challenge_method == "S256")
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

    /// Authenticate a token-management request. Public clients identify
    /// themselves with client_id; confidential clients must additionally prove
    /// possession of their registered client_secret.
    pub async fn authenticate_token_client(
        db: &DatabaseConnection,
        client_id: Option<&str>,
        client_secret: Option<&str>,
    ) -> Result<oauth_client::Model> {
        if client_secret.is_some() {
            return Self::authenticate_client(db, client_id, client_secret).await;
        }

        let client_id = client_id.ok_or(AppError::BadRequest("missing client_id".to_string()))?;
        let client = oauth_client::Entity::get_by_client_id(db, client_id)
            .await?
            .ok_or(AppError::Unauthorized("invalid client_id".to_string()))?;
        if !client.public_client {
            return Err(AppError::BadRequest("missing client_secret".to_string()));
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
        user_id: IdType,
        client_id: String,
        name: String,
        redirect_uris: Vec<String>,
        allowed_scopes: Vec<String>,
        grant_types: Vec<String>,
        public_client: bool,
    ) -> Result<(oauth_client::Model, Option<String>)> {
        let now = Utc::now().naive_utc();

        let client_secret = if public_client {
            None
        } else {
            let mut buf = [0u8; 32];
            OsRng.fill_bytes(&mut buf);
            Some(hex::encode(buf))
        };
        let secret_hash = match &client_secret {
            Some(secret) => bcrypt::hash(secret, bcrypt::DEFAULT_COST)
                .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("hash error: {}", e)))?,
            None => String::new(),
        };

        let am = oauth_client::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            client_id: Set(client_id),
            user_id: Set(user_id),
            name: Set(name),
            redirect_uris: Set(redirect_uris.join(",")),
            allowed_scopes: Set(allowed_scopes.join(",")),
            client_secret_hash: Set(secret_hash),
            public_client: Set(public_client),
            grant_types: Set(grant_types.join(",")),
            create_time: Set(now),
            update_time: Set(now),
        };
        let inserted = am.insert(db).await.map_err(AppError::from)?;
        Ok((inserted, client_secret))
    }
}

// ---------------------------------------------------------------------------
// Client queries & secret regeneration
// ---------------------------------------------------------------------------
impl OidcService {
    pub async fn list_clients_by_user(
        db: &DatabaseConnection,
        user_id: IdType,
    ) -> Result<Vec<oauth_client::Model>> {
        Ok(oauth_client::Entity::list_by_user(db, user_id).await?)
    }

    pub async fn get_client_by_id_for_owner(
        db: &DatabaseConnection,
        id: i32,
        user_id: IdType,
    ) -> Result<oauth_client::Model> {
        oauth_client::Entity::get_by_id_and_owner(db, id, user_id)
            .await?
            .ok_or(AppError::NotFound("client not found".to_string()))
    }

    /// Regenerate client_secret. Returns (updated Model, plaintext_secret).
    pub async fn regenerate_client_secret(
        db: &DatabaseConnection,
        id: i32,
        user_id: IdType,
    ) -> Result<(oauth_client::Model, String)> {
        let client = Self::get_client_by_id_for_owner(db, id, user_id).await?;

        let mut buf = [0u8; 32];
        OsRng.fill_bytes(&mut buf);
        let new_secret = hex::encode(buf);
        let secret_hash = bcrypt::hash(&new_secret, bcrypt::DEFAULT_COST)
            .map_err(|e| AppError::InternalServerError(anyhow::anyhow!("hash error: {}", e)))?;

        let mut am: oauth_client::ActiveModel = client.into();
        am.client_secret_hash = Set(secret_hash);
        am.public_client = Set(false);
        am.update_time = Set(Utc::now().naive_utc());
        let updated = am.update(db).await?;
        Ok((updated, new_secret))
    }

    /// Update an existing OAuth client's mutable fields.
    pub async fn update_client(
        db: &DatabaseConnection,
        id: i32,
        user_id: IdType,
        name: Option<String>,
        redirect_uris: Option<Vec<String>>,
        allowed_scopes: Option<Vec<String>>,
        grant_types: Option<Vec<String>>,
        public_client: Option<bool>,
    ) -> Result<oauth_client::Model> {
        let client = Self::get_client_by_id_for_owner(db, id, user_id).await?;
        let was_public = client.public_client;
        let mut am: oauth_client::ActiveModel = client.into();
        if let Some(v) = name {
            am.name = Set(v);
        }
        if let Some(v) = redirect_uris {
            am.redirect_uris = Set(v.join(","));
        }
        if let Some(v) = allowed_scopes {
            am.allowed_scopes = Set(v.join(","));
        }
        if let Some(v) = grant_types {
            am.grant_types = Set(v.join(","));
        }
        if let Some(true) = public_client {
            am.public_client = Set(true);
            am.client_secret_hash = Set(String::new());
        } else if public_client == Some(false) && was_public {
            return Err(AppError::BadRequest(
                "a confidential client requires a regenerated client_secret".to_string(),
            ));
        }
        am.update_time = Set(Utc::now().naive_utc());
        let updated = am.update(db).await?;
        Ok(updated)
    }

    /// Delete an OAuth client by id (owner only).
    pub async fn delete_client(db: &DatabaseConnection, id: i32, user_id: IdType) -> Result<()> {
        let client = Self::get_client_by_id_for_owner(db, id, user_id).await?;
        let am: oauth_client::ActiveModel = client.into();
        am.delete(db).await?;
        Ok(())
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
    pub code_challenge: Option<String>,
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
            code_challenge: Set(params.code_challenge.unwrap_or_default()),
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
            .sign_access_token(
                entry.user_id,
                &client.client_id,
                entry.scope.as_deref().unwrap_or(""),
            )
            .await?;

        let id_token = state.id_token_signer().sign_id_token::<IdType>(
            entry.user_id,
            &base,
            Some(&client.client_id),
            entry.nonce.as_deref(),
            Some(&access_token),
        )?;

        let refresh_token = Self::generate_refresh_token_value();
        let now = Utc::now().naive_utc();
        let access_expires = now + Duration::seconds(state.oidc_access_token_exp());
        let refresh_expires = now + Duration::seconds(state.oidc_refresh_token_exp());

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
            .sign_access_token(
                rec.user_id,
                &rec.client_id,
                rec.scope.as_deref().unwrap_or(""),
            )
            .await?;

        let id_token = state.id_token_signer().sign_id_token::<IdType>(
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
        let access_expires = now + Duration::seconds(state.oidc_access_token_exp());
        let refresh_expires = now + Duration::seconds(state.oidc_refresh_token_exp());

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
        state.sign_access_token_for_client(&client.client_id, &client.client_id, "")
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
// Consent operations
// ---------------------------------------------------------------------------
impl OidcService {
    /// Check if a user has already consented to the given client.
    pub async fn find_consent(
        db: &DatabaseConnection,
        user_id: IdType,
        client_id: &str,
    ) -> Result<Option<oauth_consent::Model>> {
        Ok(oauth_consent::Entity::find_by_user_and_client(db, user_id, client_id).await?)
    }

    /// Check whether an existing consent covers all requested scopes.
    pub fn consent_covers_scopes(consent: &oauth_consent::Model, requested: &[String]) -> bool {
        let consented: Vec<&str> = consent.scope.split(' ').map(|s| s.trim()).collect();
        requested.iter().all(|s| consented.contains(&s.as_str()))
    }

    /// Save or update user consent for a client + scopes.
    pub async fn save_consent(
        db: &DatabaseConnection,
        user_id: IdType,
        client_id: &str,
        scope: &str,
    ) -> Result<oauth_consent::Model> {
        let now = Utc::now().naive_utc();
        if let Some(existing) =
            oauth_consent::Entity::find_by_user_and_client(db, user_id, client_id).await?
        {
            // Update: merge scopes (union of old + new)
            let mut all_scopes: Vec<String> = existing
                .scope
                .split(' ')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            for s in scope.split(' ').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                if !all_scopes.contains(&s.to_string()) {
                    all_scopes.push(s.to_string());
                }
            }
            let mut am: oauth_consent::ActiveModel = existing.into();
            am.scope = Set(all_scopes.join(" "));
            am.update_time = Set(now);
            let updated = am.update(db).await?;
            Ok(updated)
        } else {
            let am = oauth_consent::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                user_id: Set(user_id),
                client_id: Set(client_id.to_string()),
                scope: Set(scope.to_string()),
                create_time: Set(now),
                update_time: Set(now),
            };
            let inserted = am.insert(db).await?;
            Ok(inserted)
        }
    }

    /// List all consents for a user, along with the client info.
    pub async fn list_user_consents(
        db: &DatabaseConnection,
        user_id: IdType,
    ) -> Result<Vec<(oauth_consent::Model, Option<oauth_client::Model>)>> {
        let consents = oauth_consent::Entity::find_all_by_user(db, user_id).await?;
        let mut results = Vec::with_capacity(consents.len());
        for consent in consents {
            let client = oauth_client::Entity::get_by_client_id(db, &consent.client_id).await?;
            results.push((consent, client));
        }
        Ok(results)
    }

    /// Revoke a consent and all associated tokens for that user+client.
    pub async fn revoke_consent(
        db: &DatabaseConnection,
        consent_id: i32,
        user_id: IdType,
    ) -> Result<()> {
        // Find the consent to get client_id
        let consent = oauth_consent::Entity::find_by_id(consent_id)
            .one(db)
            .await?
            .ok_or(AppError::NotFound("consent not found".to_string()))?;
        if consent.user_id != user_id {
            return Err(AppError::Unauthorized("not your consent".to_string()));
        }
        let client_id = consent.client_id.clone();

        // Delete the consent record
        oauth_consent::Entity::delete_by_id_and_user(db, consent_id, user_id).await?;

        // Revoke all tokens for this user+client
        let tokens = oauth_token::Entity::find()
            .filter(oauth_token::Column::UserId.eq(user_id))
            .filter(oauth_token::Column::ClientId.eq(&client_id))
            .filter(oauth_token::Column::Revoked.eq(false))
            .all(db)
            .await?;
        for token in tokens {
            let mut am: oauth_token::ActiveModel = token.into();
            am.revoked = Set(true);
            am.update(db).await?;
        }
        Ok(())
    }

    /// Get scopes with their descriptions for display on the consent page.
    pub async fn get_scopes_with_descriptions(
        db: &DatabaseConnection,
        scope_names: &[String],
    ) -> Result<Vec<(String, Option<String>)>> {
        let all_scopes = Self::list_scopes(db).await?;
        let result: Vec<(String, Option<String>)> = scope_names
            .iter()
            .map(|name| {
                let desc = all_scopes
                    .iter()
                    .find(|s| s.name == *name)
                    .and_then(|s| s.description.clone());
                (name.clone(), desc)
            })
            .collect();
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
pub fn get_issuer_base(state: &AppState) -> String {
    // Priority: oidc.issuer > http.public > derived from http.listen
    if !state.config.oidc.issuer.is_empty() {
        return state.config.oidc.issuer.clone();
    }
    if !state.config.http.public.is_empty() {
        return state.config.http.public.clone();
    }
    // Derive a usable URL from the listen address.
    // Replace wildcard bind addresses ([::], 0.0.0.0) with localhost.
    let listen = state.config.http.listen.to_string();
    let usable = listen
        .replace("[::]", "localhost")
        .replace("0.0.0.0", "localhost");
    format!("http://{}", usable)
}
