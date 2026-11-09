use super::repository::{
    pg::{get, one_row, signed, timestamp},
    *,
};
use crate::{
    domain::identity::{
        account::{ManagementMode, owner_is_qualified},
        application::{AppEnvironmentKind, EnvironmentConfig},
        error::IdentityError,
        migration::LegacyClientSummary,
    },
    service::security::repository::{PgSecurityTransaction, now},
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// Include siblings in a grant and recursive derived credentials. Revoking a
// grant also invalidates its refresh pair, even if only access held client_id.
const CREDENTIAL_GRAPH: &str = "WITH RECURSIVE affected AS (
    SELECT c.* FROM security_credential c WHERE c.oidc_client_id=$1
    UNION SELECT c.* FROM security_credential c JOIN affected p
    ON c.grant_id=p.grant_id OR (c.issuer=p.issuer AND c.revocation_parent_jti=p.jti)
) ";

impl PgSecurityTransaction {
    pub(crate) async fn identity_legacy_ids(
        &self,
        after: i32,
        limit: u32,
    ) -> Result<Vec<i32>, IdentityError> {
        self.identity_rows("SELECT id FROM oauth_client WHERE id>$1 AND management_state='legacy_unmanaged' ORDER BY id LIMIT $2",
            vec![after.into(), i64::from(limit).into()]).await?.iter().map(|row| get(row, "id")).collect()
    }
    pub(crate) async fn identity_legacy_workspace(
        &self,
        id: i32,
    ) -> Result<LegacyWorkspace, IdentityError> {
        let client = self.identity_one("SELECT c.*,p.issuer AS provider_issuer FROM oauth_client c CROSS JOIN identity_provider p WHERE c.id=$1 AND p.id=1", vec![id.into()]).await?;
        let client_id: String = get(&client, "client_id")?;
        let issuer: String = get(&client, "provider_issuer")?;
        let apps = self.identity_rows("SELECT id,app_key,management_state,identity_revision FROM application WHERE oidc_client_id=$1 ORDER BY id", vec![id.into()]).await?;
        let graph = self.identity_rows(&format!("{CREDENTIAL_GRAPH} SELECT c.issuer,c.jti,c.grant_id,c.session_id,c.family_id,c.subject,c.principal_kind,c.revoked_at,s.sso_session_id,s.oidc_rp_session_id,s.environment_id FROM affected c LEFT JOIN security_session s ON s.id=c.session_id ORDER BY c.issuer,c.jti LIMIT 10001"), vec![client_id.clone().into()]).await?;
        if graph.len() > 10_000 || apps.len() > 100 {
            return Err(IdentityError::CapacityExceeded);
        }
        let mut keys = vec![IdentityLockKey::Policy, IdentityLockKey::OAuthClient(id)];
        let mut related = Vec::new();
        let mut app_evidence = Vec::new();
        let mut conflicts = Vec::new();
        for app in &apps {
            let app_id: i32 = get(app, "id")?;
            related.push(app_id);
            keys.push(IdentityLockKey::Application(app_id));
            let state: String = get(app, "management_state")?;
            if state != "legacy_unmanaged" {
                conflicts.push("managed_binding".to_owned());
            }
            app_evidence.push(serde_json::json!([
                app_id,
                get::<String>(app, "app_key")?,
                state,
                get::<i64>(app, "identity_revision")?
            ]));
        }
        if related.is_empty() {
            conflicts.push("orphan_client".to_owned());
        }
        if related.len() > 1 {
            conflicts.push("multiple_applications".to_owned());
        }
        let registry = self.identity_rows("SELECT issuer,audience,application,enabled FROM security_session_client WHERE client_id=$1 ORDER BY issuer",vec![client_id.clone().into()]).await?;
        let mut registry_evidence = Vec::new();
        for row in registry {
            let registered_issuer: String = get(&row, "issuer")?;
            if registered_issuer != issuer {
                conflicts.push("issuer_conflict".to_owned());
            }
            registry_evidence.push(serde_json::json!([
                registered_issuer,
                get::<String>(&row, "audience")?,
                get::<String>(&row, "application")?,
                get::<bool>(&row, "enabled")?
            ]));
        }
        let mut credential_evidence = Vec::new();
        for row in graph {
            let credential_issuer: String = get(&row, "issuer")?;
            if credential_issuer != issuer {
                conflicts.push("issuer_conflict".to_owned());
            }
            let jti: String = get(&row, "jti")?;
            let grant: Uuid = get(&row, "grant_id")?;
            keys.push(IdentityLockKey::Grant(grant));
            keys.push(IdentityLockKey::Credential {
                issuer: credential_issuer.clone(),
                jti: jti.clone(),
            });
            if get::<String>(&row, "principal_kind")? == "user" {
                keys.push(IdentityLockKey::User(
                    get::<String>(&row, "subject")?
                        .parse()
                        .map_err(|_| IdentityError::Conflict)?,
                ));
            }
            for (column, ctor) in [
                (
                    "session_id",
                    IdentityLockKey::SecuritySession as fn(Uuid) -> IdentityLockKey,
                ),
                ("family_id", IdentityLockKey::Family),
                ("sso_session_id", IdentityLockKey::Center),
                ("oidc_rp_session_id", IdentityLockKey::RelyingParty),
            ] {
                if let Some(value) = get::<Option<Uuid>>(&row, column)? {
                    keys.push(ctor(value));
                }
            }
            if get::<Option<Uuid>>(&row, "environment_id")?.is_some() {
                conflicts.push("managed_binding".to_owned());
            }
            credential_evidence.push(serde_json::json!([
                credential_issuer,
                jti,
                grant,
                get::<Option<i64>>(&row, "revoked_at")?
            ]));
        }
        let tokens = self.identity_rows("SELECT id,access_token,id_token,revoked FROM oauth_token WHERE client_id=$1 ORDER BY id LIMIT 10001", vec![client_id.clone().into()]).await?;
        if tokens.len() > 10_000 {
            return Err(IdentityError::CapacityExceeded);
        }
        let mut token_evidence = Vec::new();
        for row in tokens {
            let mut fingerprints = Vec::new();
            for column in ["access_token", "id_token"] {
                let token: String = get(&row, column)?;
                if crate::domain::identity::migration::historical_token_issuer(&token)
                    .is_some_and(|historical| historical != issuer)
                {
                    conflicts.push("issuer_conflict".to_owned());
                }
                fingerprints.push(hex::encode(Sha256::digest(token.as_bytes())));
            }
            token_evidence.push(serde_json::json!([
                get::<i32>(&row, "id")?,
                fingerprints,
                get::<bool>(&row, "revoked")?
            ]));
        }
        let codes=self.identity_rows("SELECT id,used FROM oauth_authorization_code WHERE client_id=$1 ORDER BY id LIMIT 10001",vec![client_id.clone().into()]).await?;
        if codes.len() > 10_000 {
            return Err(IdentityError::CapacityExceeded);
        }
        let mut code_evidence = Vec::new();
        for row in codes {
            let code: i32 = get(&row, "id")?;
            keys.push(IdentityLockKey::Code(code));
            code_evidence.push(serde_json::json!([code, get::<bool>(&row, "used")?]));
        }
        let owner: i32 = get(&client, "user_id")?;
        if self
            .identity_account(owner)
            .await?
            .is_none_or(|a| !a.enabled)
        {
            conflicts.push("owner_unavailable".to_owned());
        }
        let management_state: String = get(&client, "management_state")?;
        if management_state != "legacy_unmanaged" {
            conflicts.push("managed_binding".to_owned());
        }
        conflicts.sort();
        conflicts.dedup();
        // Only a digest leaves the repository; secret hashes and credential
        // identifiers in this evidence are never part of scan responses.
        let evidence = serde_json::json!([
            get::<String>(&client, "client_id")?,
            get::<String>(&client, "name")?,
            owner,
            get::<bool>(&client, "public_client")?,
            get::<String>(&client, "client_secret_hash")?,
            get::<String>(&client, "redirect_uris")?,
            get::<String>(&client, "allowed_scopes")?,
            get::<String>(&client, "grant_types")?,
            &management_state,
            get::<Option<Uuid>>(&client, "environment_id")?,
            issuer,
            app_evidence,
            registry_evidence,
            credential_evidence,
            code_evidence,
            token_evidence
        ]);
        let snapshot = hex::encode(Sha256::digest(
            serde_json::to_vec(&evidence).map_err(|_| IdentityError::InvalidInput)?,
        ));
        Ok(LegacyWorkspace {
            summary: LegacyClientSummary {
                oauth_client_id: id,
                client_id,
                name: get(&client, "name")?,
                candidate_owner_id: owner,
                public_client: get(&client, "public_client")?,
                management_state,
                candidate_application_ids: related,
                conflict_kinds: conflicts,
                snapshot,
            },
            locks: IdentityLockScope::new(keys)?,
        })
    }
    pub(crate) async fn write_identity_migration(
        &mut self,
        command: IdentityWrite,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let at = signed(now(self.clock.as_ref())?)?;
        match command {
            IdentityWrite::RecordLegacyScan { oauth_client_id } => {
                let workspace = self.identity_legacy_workspace(oauth_client_id).await?;
                let s = workspace.summary;
                let kinds = if s.conflict_kinds.is_empty() {
                    vec!["related_client".to_owned()]
                } else {
                    s.conflict_kinds
                };
                for kind in kinds {
                    self.identity_execute("INSERT INTO identity_migration_conflict (id,oauth_client_id,candidate_application_id,candidate_owner_id,conflict_kind,evidence_summary,created_at) SELECT $1,$2,$3,$4,$5,$6,$7 WHERE NOT EXISTS (SELECT 1 FROM identity_migration_conflict WHERE oauth_client_id=$2 AND conflict_kind=$5 AND resolved_at IS NULL)",vec![Uuid::new_v4().into(),oauth_client_id.into(),s.candidate_application_ids.first().copied().into(),s.candidate_owner_id.into(),kind.into(),serde_json::json!({"client_id":s.client_id,"affected_count":s.candidate_application_ids.len()}).into(),at.into()]).await?;
                }
                Ok(IdentityWriteResult::Changed)
            }
            IdentityWrite::AdoptLegacyApplication(input) => {
                self.adopt_identity_legacy(*input, at).await
            }
            _ => Err(IdentityError::InvalidInput),
        }
    }
    async fn adopt_identity_legacy(
        &mut self,
        input: LegacyAdoptionWrite,
        at: i64,
    ) -> Result<IdentityWriteResult, IdentityError> {
        let workspace = self
            .identity_legacy_workspace(input.oauth_client_id)
            .await?;
        let summary = &workspace.summary;
        if summary.snapshot != input.expected_snapshot
            || summary.management_state != "legacy_unmanaged"
            || summary.conflict_kinds.iter().any(|s| {
                [
                    "issuer_conflict",
                    "multiple_applications",
                    "managed_binding",
                ]
                .contains(&s.as_str())
            })
        {
            return Err(IdentityError::Conflict);
        }
        if !summary.candidate_application_ids.is_empty()
            && summary.candidate_application_ids.first().copied() != input.existing_application_id
        {
            return Err(IdentityError::Conflict);
        }
        let r = &input.registration;
        let owner = self
            .identity_account(r.owner_user_id)
            .await?
            .ok_or(IdentityError::OwnerRequired)?;
        if !owner_is_qualified(&owner, ManagementMode::AdminConfigured) {
            return Err(IdentityError::OwnerRequired);
        }
        let retained = match input.retained_environment {
            AppEnvironmentKind::Development => &r.development,
            AppEnvironmentKind::Production => &r.production,
        };
        if retained.client_id != summary.client_id
            || summary.public_client == retained.config.permits_client_secret()
        {
            return Err(IdentityError::InvalidInput);
        }
        let app_id = if let Some(app) = input.existing_application_id {
            one_row(self.identity_execute("UPDATE application SET name=$3,description=$4,management_state='managed',application_kind=$5,management_mode='admin_configured',lifecycle='active',identity_revision=identity_revision+1,update_time=$6 WHERE id=$1 AND app_key=$2 AND management_state='legacy_unmanaged' AND (oidc_client_id IS NULL OR oidc_client_id=$7)",vec![app.into(),r.app_key.clone().into(),r.name.clone().into(),r.description.clone().into(),if r.third_party{"third_party"}else{"internal"}.into(),timestamp(at)?.into(),input.oauth_client_id.into()]).await?)?;
            app
        } else {
            let row=self.identity_one("INSERT INTO application (app_key,name,description,oidc_client_id,management_state,application_kind,management_mode,lifecycle,create_time,update_time) VALUES ($1,$2,$3,$4,'managed',$5,'admin_configured','active',$6,$6) RETURNING id",vec![r.app_key.clone().into(),r.name.clone().into(),r.description.clone().into(),input.oauth_client_id.into(),if r.third_party{"third_party"}else{"internal"}.into(),timestamp(at)?.into()]).await?;
            get(&row, "id")?
        };
        self.identity_execute("INSERT INTO identity_app_member (application_id,user_id,member_role,created_by,created_at) VALUES ($1,$2,'owner',$3,$4)",vec![app_id.into(),owner.user_id.into(),input.actor_id.into(),at.into()]).await?;
        for (kind, environment) in [
            (AppEnvironmentKind::Development, &r.development),
            (AppEnvironmentKind::Production, &r.production),
        ] {
            let retained = kind == input.retained_environment;
            let config =
                EnvironmentConfig::from_payload(environment.config.to_payload()?, kind, true)?;
            let client = if retained {
                one_row(self.identity_execute("UPDATE oauth_client SET management_state='managed',environment_id=$2,user_id=$3,name=$4,redirect_uris=$5,allowed_scopes=$6,grant_types='authorization_code,refresh_token',update_time=$7 WHERE id=$1 AND management_state='legacy_unmanaged'",vec![input.oauth_client_id.into(),environment.id.into(),owner.user_id.into(),r.name.clone().into(),config.redirect_uris.iter().map(|u|u.as_str()).collect::<Vec<_>>().join(",").into(),config.identity_scopes.as_protocol_string().replace(' ',",").into(),timestamp(at)?.into()]).await?)?;
                self.identity_execute("INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) SELECT issuer,$1,$2,$3,$4 FROM identity_provider WHERE id=1 ON CONFLICT (issuer,client_id) DO UPDATE SET audience=EXCLUDED.audience,application=EXCLUDED.application,enabled=EXCLUDED.enabled",vec![environment.client_id.clone().into(),config.access_audience.as_str().into(),r.app_key.clone().into(),(kind==AppEnvironmentKind::Development).into()]).await?;
                input.oauth_client_id
            } else {
                self.insert_identity_client(
                    environment.id,
                    &environment.client_id,
                    environment.client_secret_hash.as_ref(),
                    &config,
                    owner.user_id,
                    &r.name,
                    &r.app_key,
                    kind == AppEnvironmentKind::Development,
                    at,
                )
                .await?
            };
            self.identity_execute("INSERT INTO identity_app_environment (id,application_id,kind,oauth_client_id,active_version_id,created_at) VALUES ($1,$2,$3,$4,$5,$6)",vec![environment.id.into(),app_id.into(),match kind {AppEnvironmentKind::Development=>"development",AppEnvironmentKind::Production=>"production"}.into(),client.into(),if kind==AppEnvironmentKind::Development{Some(environment.initial_version_id)}else{None}.into(),at.into()]).await?;
            self.insert_identity_version(
                environment.initial_version_id,
                environment.id,
                1,
                &config,
                None,
                input.actor_id,
                at,
            )
            .await?;
        }
        self.revoke_legacy_client(&summary.client_id, at).await?;
        self.identity_execute("UPDATE identity_migration_conflict SET resolved_by=$2,resolution='explicit_adoption',resolved_at=$3 WHERE oauth_client_id=$1 AND resolved_at IS NULL",vec![input.oauth_client_id.into(),input.actor_id.into(),at.into()]).await?;
        Ok(IdentityWriteResult::Application {
            application_id: app_id,
            development_client_id: r.development.client_id.clone(),
            production_client_id: r.production.client_id.clone(),
        })
    }
    async fn revoke_legacy_client(&self, client: &str, at: i64) -> Result<(), IdentityError> {
        for (table, column) in [
            ("security_grant", "grant_id"),
            ("security_session", "session_id"),
            ("security_refresh_family", "family_id"),
        ] {
            self.identity_execute(&format!("{CREDENTIAL_GRAPH} UPDATE {table} SET revoked_at=COALESCE(revoked_at,$2) WHERE id IN (SELECT {column} FROM affected WHERE {column} IS NOT NULL)"),vec![client.into(),at.into()]).await?;
        }
        self.identity_execute(&format!("{CREDENTIAL_GRAPH} UPDATE security_credential c SET revoked_at=COALESCE(c.revoked_at,$2) FROM affected a WHERE c.issuer=a.issuer AND c.jti=a.jti"),vec![client.into(),at.into()]).await?;
        self.identity_execute(
            "UPDATE oauth_authorization_code SET used=TRUE WHERE client_id=$1",
            vec![client.into()],
        )
        .await?;
        self.identity_execute(
            "UPDATE oauth_token SET revoked=TRUE WHERE client_id=$1",
            vec![client.into()],
        )
        .await?;
        self.identity_execute(
            "DELETE FROM oauth_consent WHERE client_id=$1",
            vec![client.into()],
        )
        .await?;
        Ok(())
    }
}
