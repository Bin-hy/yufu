use super::super::common::IdentityFixture;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use yufu::migration::{Migrator, MigratorTrait};

#[tokio::test]
async fn identity_migration_production_key_missing_corrupt_and_restart_policy() {
    use yufu::config::Config;
    struct KeyFile(std::path::PathBuf);
    impl Drop for KeyFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let path =
        KeyFile(std::env::temp_dir().join(format!("identity-policy-{}.pem", uuid::Uuid::new_v4())));
    let mut config = Config::default();
    config.oidc.issuer = "https://identity.example".into();
    config.oidc.rsa_private_key_path = path.0.to_string_lossy().into();
    config.auth.exp_seconds = 300;
    let error = config
        .load_identity_signer()
        .await
        .err()
        .unwrap()
        .to_string();
    assert_eq!(error, "persistent RSA key unavailable");
    assert!(!error.contains(path.0.to_str().unwrap()));
    assert!(
        !path.0.exists(),
        "production must never generate a replacement"
    );
    config.oidc.issuer = "http://localhost:4000".into();
    assert!(config.load_identity_signer().await.is_err());
    config.http.listen = "127.0.0.1:4000".parse().unwrap();
    config.auth.allow_insecure_loopback_cookies = true;
    let first = config.load_identity_signer().await.unwrap();
    let pem = std::fs::read(&path.0).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path.0).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    config.auth.allow_insecure_loopback_cookies = false;
    config.oidc.issuer = "https://identity.example".into();
    let second = config.load_identity_signer().await.unwrap();
    assert_eq!(first.jwks_json(), second.jwks_json());
    assert_eq!(std::fs::read(&path.0).unwrap(), pem);
    std::fs::write(&path.0, "corrupt-key-secret").unwrap();
    for development in [false, true] {
        config.auth.allow_insecure_loopback_cookies = development;
        config.oidc.issuer = if development {
            "http://localhost:4000"
        } else {
            "https://identity.example"
        }
        .into();
        assert_eq!(
            config
                .load_identity_signer()
                .await
                .err()
                .unwrap()
                .to_string(),
            "persistent RSA key invalid"
        );
        assert_eq!(
            std::fs::read_to_string(&path.0).unwrap(),
            "corrupt-key-secret"
        );
    }
}

async fn isolated_schema() -> (
    testcontainers::ContainerAsync<testcontainers::GenericImage>,
    sea_orm::DatabaseConnection,
) {
    let (container, url) = super::super::common::get_postgres_container()
        .await
        .unwrap();
    let db = sea_orm::Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    // Schema tests seed explicit people; migrations do not create user accounts.
    db.execute_unprepared(r#"INSERT INTO "user" (id,username,password,role_id,create_time,update_time) VALUES (1,'schema-admin','schema-hash',1,NOW(),NOW()),(2,'schema-developer','schema-hash',3,NOW(),NOW())"#).await.unwrap();
    (container, db)
}

async fn scalar_i64(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "value")
        .unwrap()
}

#[tokio::test]
async fn identity_migration_operations_preserve_legacy_denials_and_do_not_restore_edited_grants() {
    let (_container, url) = super::super::common::get_postgres_container()
        .await
        .unwrap();
    let db = sea_orm::Database::connect(url).await.unwrap();
    let preceding = Migrator::migrations()
        .iter()
        .position(|m| m.name() == "m20261109_000004_identity_operations")
        .unwrap();
    Migrator::up(&db, Some(preceding as u32)).await.unwrap();
    db.execute_unprepared("INSERT INTO \"user\" (id,username,password,role_id,create_time,update_time) VALUES (1,'upgrade-admin','fixture-hash',1,NOW(),NOW()); INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) SELECT 1,id,false,NOW(),NOW() FROM authority WHERE method='POST' AND authority_uri='/user'").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(scalar_i64(&db,"SELECT COUNT(*)::bigint AS value FROM user_authority ua JOIN authority a ON a.id=ua.authority_id WHERE ua.user_id=1 AND NOT ua.allow AND a.authority_uri IN ('/admin/invitations','/admin/invitations/{id}/revoke')").await,2);
    assert_eq!(scalar_i64(&db,"SELECT COUNT(*)::bigint AS value FROM authority_role ar JOIN authority a ON a.id=ar.authority_id WHERE ar.role_id=3 AND a.authority_uri LIKE '/admin/%'").await,0);
    db.execute_unprepared("DELETE FROM authority_role WHERE authority_id IN (SELECT id FROM authority WHERE authority_uri='/admin/invitations'); DELETE FROM seaql_migrations WHERE version='m20261109_000004_identity_operations'").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(scalar_i64(&db,"SELECT COUNT(*)::bigint AS value FROM authority_role ar JOIN authority a ON a.id=ar.authority_id WHERE a.authority_uri='/admin/invitations'").await,0);
    assert_eq!(scalar_i64(&db,"SELECT COUNT(*)::bigint AS value FROM user_authority ua JOIN authority a ON a.id=ua.authority_id WHERE ua.user_id=1 AND NOT ua.allow AND a.authority_uri='/admin/invitations'").await,1);
}

#[tokio::test]
async fn identity_migration_fixture_database_keys_ports_and_cleanup_are_isolated() {
    let first = IdentityFixture::start().await;
    let second = IdentityFixture::start().await;
    assert_ne!(first.base_url, second.base_url);
    assert_ne!(first.key_path, second.key_path);
    assert_ne!(first.signer.jwks_json(), second.signer.jwks_json());
    assert!(first.signer.is_rsa());
    assert!(second.signer.is_rsa());
    use auth::security::budget::Clock;
    let second_reading = second.clock.now().unwrap();
    first.clock.advance(100);
    assert_eq!(
        second.clock.now().unwrap().monotonic,
        second_reading.monotonic
    );
    assert_eq!(
        first.clock.now().unwrap().monotonic,
        std::time::Duration::from_millis(100)
    );
    for f in [&first, &second] {
        let discovery: serde_json::Value = f
            .client
            .get(format!("{}/.well-known/openid-configuration", f.base_url))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(discovery["issuer"], f.base_url);
        assert!(
            discovery["id_token_signing_alg_values_supported"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("RS256"))
        );
    }
    first.db.execute_unprepared("CREATE TABLE fixture_isolation (id INTEGER PRIMARY KEY); INSERT INTO fixture_isolation VALUES (42)").await.unwrap();
    let row = second
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('fixture_isolation')::text AS name".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<Option<String>>("", "name").unwrap(), None);
    let first_key = first.key_path.clone();
    let second_key = second.key_path.clone();
    first.close().await;
    second.close().await;
    assert!(!first_key.exists());
    assert!(!second_key.exists());
}

#[tokio::test]
async fn identity_migration_fresh_schema_has_no_guessed_provider_or_access() {
    let (_container, db) = isolated_schema().await;
    for table in [
        "identity_provider",
        "identity_subject",
        "identity_invitation",
        "identity_password_reset",
        "identity_collaboration_candidate",
        "identity_user_group",
        "identity_group_member",
        "identity_app_environment",
        "identity_app_config_version",
        "identity_app_member",
        "identity_app_assignment",
        "identity_app_test_user",
        "identity_access_generation",
        "identity_app_approval",
        "identity_admin_activation",
        "identity_introspection_reader",
        "identity_request_operation",
        "identity_migration_conflict",
        "identity_sso_session",
        "identity_authorization_flow",
        "identity_action_flow",
        "identity_browser_binding",
        "identity_oidc_rp_session",
        "identity_audit_event",
        "identity_logout_delivery",
    ] {
        assert_eq!(
            scalar_i64(
                &db,
                &format!("SELECT COUNT(*)::bigint AS value FROM {table}")
            )
            .await,
            0,
            "unexpected data in {table}"
        );
    }
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE NOT enabled OR developer_enabled OR auth_epoch <> 0 OR person_kind <> 'employee' OR email IS NOT NULL").await, 0);
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM application WHERE management_state <> 'legacy_unmanaged'").await, 0);
    Migrator::up(&db, None).await.unwrap();
}

#[tokio::test]
async fn identity_migration_legacy_users_orphan_clients_codes_and_sessions_are_preserved() {
    let (_container, url) = super::super::common::get_postgres_container()
        .await
        .unwrap();
    let db = sea_orm::Database::connect(url).await.unwrap();
    let previous = Migrator::migrations()
        .iter()
        .position(|m| m.name() == "m20261109_000001_identity_accounts")
        .unwrap();
    Migrator::up(&db, Some(previous as u32)).await.unwrap();
    db.execute_unprepared(r#"INSERT INTO "user" (id,username,password,role_id,create_time,update_time) VALUES (1,'legacy-user','old-hash',1,NOW(),NOW())"#).await.unwrap();
    db.execute_unprepared(r#"
        UPDATE "user" SET nickname='legacy-person',password='unchanged-hash' WHERE id=1;
        INSERT INTO oauth_client (client_id,user_id,name,redirect_uris,allowed_scopes,client_secret_hash,public_client,grant_types,create_time,update_time)
            VALUES ('orphan-client',1,'Orphan','https://legacy.example/cb','openid','old-secret-hash',FALSE,'authorization_code',NOW(),NOW());
        INSERT INTO oauth_authorization_code (code,user_id,client_id,redirect_uri,code_challenge,code_challenge_method,scope,state,nonce,used,expires_at,create_time)
            VALUES ('legacy-plaintext-code',1,'orphan-client','https://legacy.example/cb','challenge','S256','openid','s','n',FALSE,NOW()+INTERVAL '1 minute',NOW());
        INSERT INTO security_session (id,issuer,principal_kind,subject,created_at)
            VALUES ('00000000-0000-0000-0000-000000000001','https://legacy-id.example','user','1',1000);
    "#).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE id=1 AND nickname='legacy-person' AND password='unchanged-hash' AND enabled AND NOT developer_enabled").await, 1);
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM oauth_client WHERE client_id='orphan-client' AND client_secret_hash='old-secret-hash' AND management_state='legacy_unmanaged' AND environment_id IS NULL").await, 1);
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE code='legacy-plaintext-code' AND code_digest IS NULL AND sso_session_id IS NULL").await, 1);
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM security_session WHERE sso_session_id IS NULL AND environment_id IS NULL").await, 1);
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM security_session_client WHERE client_id='orphan-client'").await, 0);
}

#[tokio::test]
async fn identity_migration_account_secret_subject_and_group_constraints_are_enforced() {
    let (_container, db) = isolated_schema().await;
    db.execute_unprepared(r#"
        INSERT INTO identity_provider (id,issuer,initialized_at) VALUES (1,'https://id.example',1000);
        INSERT INTO identity_subject (issuer,sub,user_id,created_at) VALUES ('https://id.example','1',1,1000);
        INSERT INTO identity_invitation (id,secret_digest,purpose,person_kind,developer_grant,created_by,created_at,expires_at)
            VALUES ('00000000-0000-0000-0000-000000000001',decode(repeat('01',32),'hex'),'onboarding','collaborator',FALSE,1,1000,2000);
        INSERT INTO identity_user_group (id,name,created_by,created_at,updated_at)
            VALUES ('00000000-0000-0000-0000-000000000002','test-group',1,1000,1000);
        INSERT INTO identity_group_member (group_id,user_id,created_by,created_at)
            VALUES ('00000000-0000-0000-0000-000000000002',1,1,1000);
    "#).await.unwrap();
    for sql in [
        "INSERT INTO identity_provider (id,issuer,initialized_at) VALUES (2,'https://second.example',1000)",
        "INSERT INTO identity_subject (issuer,sub,user_id,created_at) VALUES ('https://id.example','another-sub',1,1000)",
        "INSERT INTO identity_subject (issuer,sub,user_id,created_at) VALUES ('https://id.example','deleted',NULL,1000)",
        "UPDATE identity_invitation SET secret_digest=decode('ff','hex')",
        "UPDATE identity_invitation SET consumed_at=1500 WHERE accepted_user_id IS NULL",
        "UPDATE identity_invitation SET purpose='qualification' WHERE target_user_id IS NULL",
        "INSERT INTO identity_group_member (group_id,user_id,created_by,created_at) VALUES ('00000000-0000-0000-0000-000000000002',1,1,1000)",
        "UPDATE \"user\" SET auth_epoch=-1 WHERE id=1",
        "UPDATE \"user\" SET email_verified_at=1000,email=NULL WHERE id=1",
    ] {
        assert!(
            db.execute_unprepared(sql).await.is_err(),
            "constraint missing: {sql}"
        );
    }
    db.execute_unprepared("INSERT INTO identity_subject (issuer,sub,user_id,created_at,tombstoned_at) VALUES ('https://id.example','deleted',NULL,1000,2000)").await.unwrap();
}

async fn seed_application(db: &sea_orm::DatabaseConnection) {
    db.execute_unprepared(r#"
        INSERT INTO application (app_key,name,create_time,update_time) VALUES ('fixture-app','Fixture',NOW(),NOW());
        INSERT INTO oauth_client (client_id,user_id,name,redirect_uris,allowed_scopes,client_secret_hash,public_client,grant_types,create_time,update_time)
            VALUES ('fixture-client',1,'Fixture','https://app.example/cb','openid','',TRUE,'authorization_code',NOW(),NOW());
        INSERT INTO identity_app_environment (id,application_id,kind,oauth_client_id,created_at)
            SELECT '00000000-0000-0000-0000-000000000010',a.id,'production',c.id,1000 FROM application a,oauth_client c WHERE a.app_key='fixture-app' AND c.client_id='fixture-client';
        INSERT INTO identity_app_config_version (id,environment_id,version,payload,payload_hash,created_by,created_at)
            VALUES ('00000000-0000-0000-0000-000000000011','00000000-0000-0000-0000-000000000010',1,'{}',repeat('0',64),1,1000);
        INSERT INTO identity_app_member (application_id,user_id,member_role,created_by,created_at)
            SELECT id,1,'owner',1,1000 FROM application WHERE app_key='fixture-app';
        INSERT INTO identity_app_assignment (id,application_id,target_kind,user_id,created_by,created_at)
            SELECT '00000000-0000-0000-0000-000000000012',id,'user',1,1,1000 FROM application WHERE app_key='fixture-app';
    "#).await.unwrap();
}

#[tokio::test]
async fn identity_migration_governance_unique_targets_versions_and_idempotency_are_enforced() {
    let (_container, db) = isolated_schema().await;
    seed_application(&db).await;
    db.execute_unprepared(r#"
        INSERT INTO identity_user_group (id,name,created_by,created_at,updated_at) VALUES ('00000000-0000-0000-0000-000000000020','group',1,1000,1000);
        INSERT INTO identity_request_operation (id,actor_id,operation_kind,idempotency_key,request_hash,resource_id,result_summary,committed_at)
            VALUES ('00000000-0000-0000-0000-000000000021',1,'register','once',repeat('0',64),'fixture-app','{}',1000);
    "#).await.unwrap();
    for sql in [
        "INSERT INTO identity_app_member (application_id,user_id,member_role,created_by,created_at) SELECT id,2,'owner',1,1000 FROM application WHERE app_key='fixture-app'",
        "UPDATE identity_app_assignment SET group_id='00000000-0000-0000-0000-000000000020' WHERE target_kind='user'",
        "UPDATE identity_app_assignment SET user_id=NULL WHERE target_kind='user'",
        "UPDATE identity_app_config_version SET payload='{\"modified\":true}'",
        "DELETE FROM identity_app_config_version",
        "INSERT INTO identity_app_config_version (id,environment_id,version,payload,payload_hash,created_by,created_at) VALUES ('00000000-0000-0000-0000-000000000022','00000000-0000-0000-0000-000000000010',1,'{}',repeat('0',64),1,1000)",
        "INSERT INTO identity_request_operation (id,actor_id,operation_kind,idempotency_key,request_hash,resource_id,result_summary,committed_at) VALUES ('00000000-0000-0000-0000-000000000023',1,'register','once',repeat('1',64),'fixture-app','{}',1001)",
        "UPDATE oauth_client SET management_state='managed' WHERE client_id='fixture-client'",
    ] {
        assert!(
            db.execute_unprepared(sql).await.is_err(),
            "constraint missing: {sql}"
        );
    }
}

#[tokio::test]
async fn identity_migration_managed_session_code_and_logout_links_are_complete() {
    let (_container, db) = isolated_schema().await;
    seed_application(&db).await;
    db.execute_unprepared(r#"
        INSERT INTO identity_provider (id,issuer,initialized_at) VALUES (1,'https://id.example',1000);
        INSERT INTO identity_subject (issuer,sub,user_id,created_at) VALUES ('https://id.example','1',1,1000);
        INSERT INTO identity_sso_session (id,issuer,user_id,subject,secret_digest,auth_epoch,authenticated_at,expires_at,created_at)
            VALUES ('00000000-0000-0000-0000-000000000030','https://id.example',1,'1',decode(repeat('03',32),'hex'),0,1000,100000,1000);
        INSERT INTO identity_authorization_flow (id,request_payload,environment_id,config_version_id,browser_proof_digest,browser_binding_digest,bound_sso_session_id,created_at,expires_at)
            VALUES ('00000000-0000-0000-0000-000000000031','{}','00000000-0000-0000-0000-000000000010','00000000-0000-0000-0000-000000000011',decode(repeat('04',32),'hex'),decode(repeat('05',32),'hex'),'00000000-0000-0000-0000-000000000030',1000,100000);
        INSERT INTO identity_oidc_rp_session (id,sso_session_id,issuer,environment_id,config_version_id,protocol_sid,subject,auth_time,identity_scopes,created_at,oauth_client_id)
            VALUES ('00000000-0000-0000-0000-000000000032','00000000-0000-0000-0000-000000000030','https://id.example','00000000-0000-0000-0000-000000000010','00000000-0000-0000-0000-000000000011','00000000-0000-0000-0000-000000000033','1',1,'openid',1000,(SELECT id FROM oauth_client WHERE client_id='fixture-client'));
        INSERT INTO security_session (id,issuer,principal_kind,subject,created_at,sso_session_id,oidc_rp_session_id,environment_id,config_version_id,auth_epoch,lifecycle_epoch,access_generation,identity_scopes)
            VALUES ('00000000-0000-0000-0000-000000000034','https://id.example','user','1',1000,'00000000-0000-0000-0000-000000000030','00000000-0000-0000-0000-000000000032','00000000-0000-0000-0000-000000000010','00000000-0000-0000-0000-000000000011',0,0,0,'openid');
        INSERT INTO oauth_authorization_code (code,user_id,client_id,code_challenge,code_challenge_method,used,expires_at,create_time,code_digest,flow_id,sso_session_id,environment_id,config_version_id,auth_time,auth_epoch,lifecycle_epoch,access_generation,oidc_rp_session_id)
            VALUES ('sha256:'||repeat('06',32),1,'fixture-client','challenge','S256',FALSE,NOW()+INTERVAL '1 minute',NOW(),decode(repeat('06',32),'hex'),'00000000-0000-0000-0000-000000000031','00000000-0000-0000-0000-000000000030','00000000-0000-0000-0000-000000000010','00000000-0000-0000-0000-000000000011',1,0,0,0,'00000000-0000-0000-0000-000000000032');
        INSERT INTO identity_audit_event (id,request_id,actor_id,action,object_kind,object_id,happened_at,result,change_summary,commit_marker)
            VALUES ('00000000-0000-0000-0000-000000000035','00000000-0000-0000-0000-000000000036',1,'logout','root','root-one',2000,'success','{}','00000000-0000-0000-0000-000000000037');
        INSERT INTO identity_logout_delivery (id,event_id,rp_session_id,client_id,endpoint_snapshot,next_attempt_at,token_jti,created_at)
            VALUES ('00000000-0000-0000-0000-000000000038','00000000-0000-0000-0000-000000000035','00000000-0000-0000-0000-000000000032','fixture-client','https://app.example/backchannel',2000,'00000000-0000-0000-0000-000000000039',2000);
    "#).await.unwrap();
    for sql in [
        "UPDATE security_session SET access_generation=NULL",
        "UPDATE security_session SET sso_session_id=NULL",
        "UPDATE security_session SET auth_epoch=1",
        "UPDATE oauth_authorization_code SET code='plaintext-must-not-be-stored'",
        "UPDATE oauth_authorization_code SET flow_id=NULL",
        "UPDATE oauth_authorization_code SET oidc_rp_session_id=NULL",
        "UPDATE oauth_authorization_code SET oidc_rp_session_id='00000000-0000-0000-0000-000000000034'",
        "UPDATE identity_logout_delivery SET status='leased'",
        "INSERT INTO identity_logout_delivery (id,event_id,rp_session_id,client_id,endpoint_snapshot,next_attempt_at,token_jti,created_at) SELECT '00000000-0000-0000-0000-000000000040',event_id,rp_session_id,client_id,endpoint_snapshot,next_attempt_at,token_jti,created_at FROM identity_logout_delivery",
    ] {
        assert!(
            db.execute_unprepared(sql).await.is_err(),
            "constraint missing: {sql}"
        );
    }
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM security_session WHERE environment_id IS NOT NULL").await, 1);
    // Upgrade from the schema immediately before identity_code_rp retires
    // managed codes without guessing an RP. Roll back that migration and every
    // later one; newer seed migrations have no-op down steps too.
    let migrations = Migrator::migrations();
    let code_rp = migrations
        .iter()
        .position(|migration| migration.name().contains("identity_code_rp"))
        .unwrap();
    Migrator::down(&db, Some((migrations.len() - code_rp) as u32))
        .await
        .unwrap();
    db.execute_unprepared("INSERT INTO oauth_authorization_code (code,user_id,client_id,code_challenge,code_challenge_method,used,expires_at,create_time) VALUES ('legacy-code-preserved',1,'fixture-client','old-challenge','S256',FALSE,NOW()+INTERVAL '1 minute',NOW())").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE code_digest IS NOT NULL AND used AND oidc_rp_session_id IS NULL").await, 1);
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM oauth_authorization_code WHERE code='legacy-code-preserved' AND NOT used AND oidc_rp_session_id IS NULL").await, 1);
    assert!(
        db.execute_unprepared(
            "UPDATE oauth_authorization_code SET used=FALSE WHERE code_digest IS NOT NULL"
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn identity_migration_entity_roundtrip_and_explicit_links_match_schema() {
    use sea_orm::{ActiveModelTrait, EntityTrait, ModelTrait, Set};
    use yufu::entity::{identity_invitation, identity_user_group, user};
    let (_container, db) = isolated_schema().await;
    let id = uuid::Uuid::new_v4();
    let group = identity_user_group::ActiveModel {
        id: Set(id),
        name: Set("mapped-group".into()),
        description: Set(None),
        revision: Set(3),
        created_by: Set(1),
        created_at: Set(1000),
        updated_at: Set(1100),
    }
    .insert(&db)
    .await
    .unwrap();
    assert_eq!(
        identity_user_group::Entity::find_by_id(id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        group
    );
    assert_eq!(
        group
            .find_related(user::Entity)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .id,
        1
    );
    let invitation = identity_invitation::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        secret_digest: Set(vec![17; 32]),
        purpose: Set("qualification".into()),
        target_user_id: Set(Some(2)),
        person_kind: Set("employee".into()),
        developer_grant: Set(true),
        created_by: Set(1),
        created_at: Set(1000),
        expires_at: Set(2000),
        consumed_at: Set(None),
        accepted_user_id: Set(None),
        revoked_at: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();
    assert_eq!(
        invitation
            .find_linked(identity_invitation::TargetUserLink)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .id,
        2
    );
    assert_eq!(
        invitation
            .find_linked(identity_invitation::CreatorLink)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .id,
        1
    );
    assert_eq!(
        format!("{invitation:?}"),
        "identity_invitation::Model(REDACTED)"
    );
    assert_eq!(invitation.secret_digest, vec![17; 32]);
}

#[tokio::test]
async fn identity_migration_provider_subject_backfill_is_stable_and_conflicts_are_explicit() {
    use yufu::domain::identity::error::IdentityError;
    use yufu::service::identity::account::initialize_provider;
    let (_container, db) = isolated_schema().await;
    initialize_provider(&db, "https://id.example", 1000)
        .await
        .unwrap();
    initialize_provider(&db, "https://id.example", 2000)
        .await
        .unwrap();
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*)::bigint AS value FROM identity_subject"
        )
        .await,
        2
    );
    db.execute_unprepared(
        "UPDATE \"user\" SET username='renamed',nickname='new profile',role_id=3 WHERE id=1",
    )
    .await
    .unwrap();
    initialize_provider(&db, "https://id.example", 3000)
        .await
        .unwrap();
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM identity_subject WHERE issuer='https://id.example' AND sub='1' AND user_id=1").await, 1);
    assert_eq!(
        initialize_provider(&db, "https://different.example", 4000).await,
        Err(IdentityError::Conflict)
    );
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM identity_provider WHERE issuer='https://id.example'").await, 1);
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM identity_migration_conflict WHERE conflict_kind='issuer_mismatch'").await, 1);
    db.execute_unprepared(
        "UPDATE identity_subject SET user_id=NULL,tombstoned_at=4000 WHERE sub='1'",
    )
    .await
    .unwrap();
    assert_eq!(
        initialize_provider(&db, "https://id.example", 5000).await,
        Err(IdentityError::Conflict)
    );
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM identity_subject WHERE sub='1' AND user_id IS NULL").await, 1);
}

#[tokio::test]
async fn identity_migration_unadopted_schema_rollback_keeps_historical_accounts() {
    let (_container, db) = isolated_schema().await;
    let identity_count = Migrator::migrations()
        .iter()
        .filter(|m| m.name().starts_with("m20261109_"))
        .count();
    Migrator::down(&db, Some(identity_count as u32))
        .await
        .unwrap();
    assert_eq!(scalar_i64(&db, "SELECT COUNT(*)::bigint AS value FROM \"user\" WHERE username='schema-admin' AND password='schema-hash'").await, 1);
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*)::bigint AS value FROM identity_subject"
        )
        .await,
        0
    );
}
