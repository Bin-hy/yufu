use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr, Statement};

#[path = "security_revocation.rs"]
mod revocation;

#[path = "security_mcp.rs"]
mod mcp;

#[tokio::test]
async fn security_t02_postgres_fixtures_are_isolated() {
    let (_first_container, first_url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let (_second_container, second_url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let first = Database::connect(first_url).await.unwrap();
    let second = Database::connect(second_url).await.unwrap();
    for db in [&first, &second] {
        db.execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "CREATE TABLE security_fixture_probe (id INTEGER PRIMARY KEY)".to_owned(),
        ))
        .await
        .unwrap();
    }
    first
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            "INSERT INTO security_fixture_probe VALUES (1)".to_owned(),
        ))
        .await
        .unwrap();
    let row = second
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS count FROM security_fixture_probe".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
}

async fn execute(db: &DatabaseConnection, sql: &str) -> Result<sea_orm::ExecResult, DbErr> {
    db.execute_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await
}

#[tokio::test]
async fn security_t13_identity_schema_up_down_up_and_namespaced_jti_uniqueness() {
    use cobotusr::migration::{Migrator, MigratorTrait};
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let tables = [
        "security_grant",
        "security_session",
        "security_refresh_family",
        "security_credential",
    ];
    for table in tables {
        execute(&db, &format!("SELECT * FROM {table} LIMIT 0"))
            .await
            .unwrap();
    }
    // Roll back this migration and every dependent newer migration, leaving old
    // schema and seeds intact. Future security migrations remain part of this test.
    let count = Migrator::migrations()
        .into_iter()
        .filter(|m| m.name() >= "m20261107_000001_security_identity")
        .count() as u32;
    assert!(count > 0);
    Migrator::down(&db, Some(count)).await.unwrap();
    assert!(
        execute(&db, "SELECT * FROM security_credential")
            .await
            .is_err()
    );
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    execute(&db, "INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at) VALUES
        ('00000000-0000-4000-8000-000000000001','https://idp.test','machine','service','app','service',1000),
        ('00000000-0000-4000-8000-000000000002','https://other.test','machine','service','app','service',1000)")
        .await.unwrap();
    let insert = "INSERT INTO security_credential (issuer,jti,principal_kind,subject,credential_kind,token_use,audience,application,bounds,binding,grant_id,signing_key_id,issued_at,expires_at)
        VALUES ('https://idp.test','same-jti','machine','service','access','security_service','aud','app','{\"grants\":[]}','{\"kind\":\"none\"}','00000000-0000-4000-8000-000000000001','key',1000,9000)";
    execute(&db, insert).await.unwrap();
    assert!(execute(&db, insert).await.is_err());
    execute(
        &db,
        &insert
            .replace("https://idp.test", "https://other.test")
            .replace(
                "00000000-0000-4000-8000-000000000001",
                "00000000-0000-4000-8000-000000000002",
            ),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn security_t13_identity_roots_and_refresh_digest_constraints() {
    use cobotusr::migration::{Migrator, MigratorTrait};
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    execute(&db, "INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at)
        VALUES ('00000000-0000-4000-8000-000000000001','https://idp.test','user','alice','app','session',1000)")
        .await.unwrap();
    execute(
        &db,
        "INSERT INTO security_session (id,issuer,principal_kind,subject,created_at)
        VALUES ('00000000-0000-4000-8000-000000000002','https://idp.test','user','alice',1000)",
    )
    .await
    .unwrap();
    let family = "INSERT INTO security_refresh_family (id,issuer,principal_kind,subject,application,session_id,grant_id,created_at)
        VALUES ('00000000-0000-4000-8000-000000000003','https://idp.test','user','alice','app','00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000001',1000)";
    assert!(
        execute(&db, &family.replace("'alice'", "'bob'"))
            .await
            .is_err()
    );
    execute(&db, family).await.unwrap();
    let refresh = "INSERT INTO security_credential (issuer,jti,principal_kind,subject,credential_kind,token_use,audience,application,bounds,binding,grant_id,session_id,family_id,secret_digest,issued_at,expires_at)
        VALUES ('https://idp.test','refresh-jti','user','alice','refresh','user_session','aud','app','{\"grants\":[]}','{\"kind\":\"none\"}',
        '00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003',decode(repeat('ab',32),'hex'),1000,9000)";
    assert!(
        execute(&db, &refresh.replace("'alice'", "'bob'"))
            .await
            .is_err()
    );
    assert!(
        execute(&db, &refresh.replace("repeat('ab',32)", "repeat('ab',31)"))
            .await
            .is_err()
    );
    assert!(
        execute(&db, &refresh.replace("1000,9000", "9000,1000"))
            .await
            .is_err()
    );
    execute(&db, refresh).await.unwrap();
    assert!(
        execute(&db, &refresh.replace("'refresh-jti'", "'second-jti'"))
            .await
            .is_err()
    );
    assert!(
        execute(
            &db,
            "UPDATE security_credential SET replacement_jti='missing' WHERE jti='refresh-jti'"
        )
        .await
        .is_err()
    );
    // Stable roots and credentials are security facts; deleting a root with live
    // or historical dependent credentials must not erase revocation information.
    assert!(execute(&db, "DELETE FROM security_grant").await.is_err());
}

#[tokio::test]
async fn security_t14_machine_bindings_are_constrained() {
    use cobotusr::migration::{Migrator, MigratorTrait};
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let service = "INSERT INTO security_binding (id,issuer,client_id,token_use,application,audience,allowed_operations,identity_binding)
        VALUES ('00000000-0000-4000-8000-000000000001','https://idp.test','client','permission_sync','app','idp','[]','{\"kind\":\"none\"}')";
    execute(&db, service).await.unwrap();
    assert!(
        execute(
            &db,
            &service.replace(
                "00000000-0000-4000-8000-000000000001",
                "00000000-0000-4000-8000-000000000002"
            )
        )
        .await
        .is_err()
    );
    assert!(
        execute(
            &db,
            &service.replace("permission_sync", "user_session").replace(
                "00000000-0000-4000-8000-000000000001",
                "00000000-0000-4000-8000-000000000002"
            )
        )
        .await
        .is_err()
    );
}

async fn insert_machine_access(db: &DatabaseConnection) {
    execute(db, "INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at)
        VALUES ('00000000-0000-4000-8000-000000000001','https://idp.test','machine','service','app','service',1000)").await.unwrap();
    execute(db, "INSERT INTO security_credential (issuer,jti,principal_kind,subject,credential_kind,token_use,audience,application,bounds,binding,grant_id,signing_key_id,issued_at,expires_at)
        VALUES ('https://idp.test','parent','machine','service','access','security_service','aud','app','{\"grants\":[]}','{\"kind\":\"none\"}','00000000-0000-4000-8000-000000000001','key',1000,9000)").await.unwrap();
}

#[tokio::test]
async fn security_t16_entities_keep_stable_roots_audit_origin_and_revocation_parent_distinct() {
    use cobotusr::{
        entity::{
            security_credential as credential, security_grant as grant,
            security_refresh_family as family, security_session as session,
        },
        migration::{Migrator, MigratorTrait},
    };
    use sea_orm::EntityTrait;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    execute(&db, "INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at)
        VALUES ('00000000-0000-4000-8000-000000000001','https://idp.test','user','alice','app','session',1000)").await.unwrap();
    execute(
        &db,
        "INSERT INTO security_session (id,issuer,principal_kind,subject,created_at)
        VALUES ('00000000-0000-4000-8000-000000000002','https://idp.test','user','alice',1000)",
    )
    .await
    .unwrap();
    execute(&db, "INSERT INTO security_refresh_family (id,issuer,principal_kind,subject,application,session_id,grant_id,created_at)
        VALUES ('00000000-0000-4000-8000-000000000003','https://idp.test','user','alice','app',
        '00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000001',1000)").await.unwrap();
    let access = "INSERT INTO security_credential (issuer,jti,principal_kind,subject,credential_kind,token_use,audience,application,bounds,binding,grant_id,session_id,family_id,signing_key_id,issued_at,expires_at)
        VALUES ('https://idp.test','old-access','user','alice','access','user_session','aud','app','{\"grants\":[]}','{\"kind\":\"none\"}',
        '00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003','key',1000,9000)";
    execute(&db, access).await.unwrap();
    let refresh = access
        .replace("'old-access'", "'old-refresh'")
        .replace("'access'", "'refresh'")
        .replace("signing_key_id", "secret_digest")
        .replace("'key'", "decode(repeat('ab',32),'hex')");
    execute(&db, &refresh).await.unwrap();
    execute(
        &db,
        &refresh
            .replace("old-refresh", "new-refresh")
            .replace("repeat('ab'", "repeat('cd'"),
    )
    .await
    .unwrap();
    execute(&db, "UPDATE security_credential SET consumed_at=2000,replacement_jti='new-refresh' WHERE jti='old-refresh'").await.unwrap();
    execute(&db, &access.replace("old-access", "new-access"))
        .await
        .unwrap();
    execute(&db, "UPDATE security_credential SET minted_by='{\"issuer\":\"https://idp.test\",\"jti\":\"old-refresh\"}' WHERE jti='new-access'").await.unwrap();
    execute(
        &db,
        &access
            .replace("old-access", "delegation")
            .replace("user_session", "gateway_operation")
            .replace("signing_key_id,", "revocation_parent_jti,signing_key_id,")
            .replace("'key',", "'old-access','key',"),
    )
    .await
    .unwrap();
    let fresh =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "new-access".to_owned()))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
    let old =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "old-refresh".to_owned()))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
    let delegation =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "delegation".to_owned()))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(fresh.minted_by.as_ref().unwrap()["jti"], "old-refresh");
    assert!(fresh.revocation_parent_jti.is_none());
    assert_eq!(
        delegation.revocation_parent_jti.as_deref(),
        Some("old-access")
    );
    assert_eq!(old.replacement_jti.as_deref(), Some("new-refresh"));
    assert_eq!(old.consumed_at, Some(2000));
    assert_eq!(old.secret_digest, Some(vec![0xab; 32]));
    assert!(fresh.secret_digest.is_none());
    let root = grant::Entity::find_by_id(fresh.grant_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let session = session::Entity::find_by_id(fresh.session_id.unwrap())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let family = family::Entity::find_by_id(fresh.family_id.unwrap())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(family.grant_id, root.id);
    assert_eq!(family.session_id, session.id);
    assert_eq!(family.subject, "alice");
    assert!(
        root.revoked_at.is_none() && session.revoked_at.is_none() && family.revoked_at.is_none()
    );
    let columns = db.query_all_raw(Statement::from_string(DbBackend::Postgres,
        "SELECT column_name FROM information_schema.columns WHERE table_name='security_credential'".to_owned())).await.unwrap();
    let names: Vec<String> = columns
        .iter()
        .map(|row| row.try_get("", "column_name").unwrap())
        .collect();
    assert!(
        !names
            .iter()
            .any(|name| ["refresh_token", "access_token", "secret", "token"]
                .contains(&name.as_str()))
    );
}

#[tokio::test]
async fn security_t17_primary_transactions_refresh_hashes_and_disabled_bindings() {
    use auth::security::{
        budget::SecurityBudget,
        credential::CredentialRef,
        identity::{PrincipalKind, PrincipalRef, SecretToken, TokenUse},
    };
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::repository::{
            PgSecurityRepository, SecurityRepository, ServiceBindingKey,
        },
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    insert_machine_access(&db).await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let budget = SecurityBudget::new(clock.clone()).unwrap();
    let repository = PgSecurityRepository::new(db.clone(), clock);
    let mut txn = repository.begin(&budget).await.unwrap();
    let found = txn
        .credential(&CredentialRef::new("https://idp.test", "parent").unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.reference().jti(), "parent");
    assert_eq!(found.checked_at(), 2000);
    assert!(
        txn.credential(&CredentialRef::new("https://other.test", "parent").unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        txn.refresh(
            "https://idp.test",
            &SecretToken::new("unknown-refresh-marker".into()).unwrap()
        )
        .await
        .unwrap()
        .is_none()
    );
    let key = ServiceBindingKey {
        principal: PrincipalRef::new("https://idp.test", PrincipalKind::Machine, "service")
            .unwrap(),
        token_use: TokenUse::SecurityService,
        application: "app".into(),
        audience: "aud".into(),
    };
    assert!(txn.service_binding(&key).await.unwrap().is_none());
    txn.commit().await.unwrap();
    execute(&db, "INSERT INTO security_binding (id,issuer,client_id,token_use,application,audience,allowed_operations,identity_binding)
        VALUES ('00000000-0000-4000-8000-000000000007','https://idp.test','service','security_service','app','aud','[]','{\"kind\":\"none\"}')").await.unwrap();
    let mut txn = repository.begin(&budget).await.unwrap();
    assert!(txn.service_binding(&key).await.unwrap().is_none());
    txn.rollback().await.unwrap();
    execute(&db, "UPDATE security_binding SET enabled=true")
        .await
        .unwrap();
    let mut txn = repository.begin(&budget).await.unwrap();
    let binding = txn.service_binding(&key).await.unwrap().unwrap();
    assert!(binding.bounds().grants().is_empty());
    txn.commit().await.unwrap();
}

#[tokio::test]
async fn security_t17_refresh_lookup_preserves_consumed_records_without_plaintext() {
    use auth::security::{budget::SecurityBudget, identity::SecretToken};
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::repository::{PgSecurityRepository, SecurityRepository, refresh_digest},
    };
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    insert_machine_access(&db).await;
    // User roots are required for a refresh family, independently of machine roots.
    execute(&db, "INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at)
        VALUES ('00000000-0000-4000-8000-000000000010','https://idp.test','user','alice','app','session',1000)").await.unwrap();
    execute(
        &db,
        "INSERT INTO security_session (id,issuer,principal_kind,subject,created_at)
        VALUES ('00000000-0000-4000-8000-000000000011','https://idp.test','user','alice',1000)",
    )
    .await
    .unwrap();
    execute(&db, "INSERT INTO security_refresh_family (id,issuer,principal_kind,subject,application,session_id,grant_id,created_at)
        VALUES ('00000000-0000-4000-8000-000000000012','https://idp.test','user','alice','app','00000000-0000-4000-8000-000000000011','00000000-0000-4000-8000-000000000010',1000)").await.unwrap();
    use cobotusr::entity::security_credential as credential;
    let mut row: credential::ActiveModel =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "parent".to_owned()))
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into();
    let secret = SecretToken::new("refresh-secret-marker-never-store-this-value".into()).unwrap();
    row.jti = Set("refresh".into());
    row.principal_kind = Set("user".into());
    row.subject = Set("alice".into());
    row.credential_kind = Set("refresh".into());
    row.token_use = Set("user_session".into());
    row.signing_key_id = Set(None);
    row.grant_id = Set("00000000-0000-4000-8000-000000000010".parse().unwrap());
    row.session_id = Set(Some(
        "00000000-0000-4000-8000-000000000011".parse().unwrap(),
    ));
    row.family_id = Set(Some(
        "00000000-0000-4000-8000-000000000012".parse().unwrap(),
    ));
    row.secret_digest = Set(Some(refresh_digest(&secret).to_vec()));
    row.consumed_at = Set(Some(2000));
    row.insert(&db).await.unwrap();
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let budget = SecurityBudget::new(clock.clone()).unwrap();
    let repository = PgSecurityRepository::new(db, clock);
    let mut txn = repository.begin(&budget).await.unwrap();
    let found = txn
        .refresh("https://idp.test", &secret)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.reference().jti(), "refresh");
    assert!(!format!("{found:?}").contains(secret.expose_secret()));
    assert!(
        txn.refresh("https://other.test", &secret)
            .await
            .unwrap()
            .is_none()
    );
    txn.rollback().await.unwrap();
}

async fn enable_service_binding(db: &DatabaseConnection) {
    execute(db,"INSERT INTO security_binding (id,issuer,client_id,token_use,application,audience,allowed_operations,identity_binding,enabled)
        VALUES ('00000000-0000-4000-8000-000000000007','https://idp.test','service','security_service','app','aud','[]','{\"kind\":\"none\"}',true)").await.unwrap();
}
fn security_budget(
    clock: &std::sync::Arc<crate::cobotusr::common::SecurityTestClock>,
) -> auth::security::budget::SecurityBudget {
    auth::security::budget::SecurityBudget::new(clock.clone()).unwrap()
}
async fn insert_user_access(db: &DatabaseConnection) {
    use cobotusr::entity::security_credential as credential;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    execute(db,"INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at)
        VALUES ('00000000-0000-4000-8000-000000000010','https://idp.test','user','alice','app','session',1000),
        ('00000000-0000-4000-8000-000000000013','https://idp.test','user','alice','app','mcp',1000)").await.unwrap();
    execute(
        db,
        "INSERT INTO security_session (id,issuer,principal_kind,subject,created_at)
        VALUES ('00000000-0000-4000-8000-000000000011','https://idp.test','user','alice',1000)",
    )
    .await
    .unwrap();
    execute(db,"INSERT INTO security_refresh_family (id,issuer,principal_kind,subject,application,session_id,grant_id,created_at)
        VALUES ('00000000-0000-4000-8000-000000000012','https://idp.test','user','alice','app',
        '00000000-0000-4000-8000-000000000011','00000000-0000-4000-8000-000000000010',1000)").await.unwrap();
    let mut row: credential::ActiveModel =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "parent".to_owned()))
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .into();
    row.jti = Set("user-access".into());
    row.principal_kind = Set("user".into());
    row.subject = Set("alice".into());
    row.token_use = Set("user_session".into());
    row.grant_id = Set("00000000-0000-4000-8000-000000000010".parse().unwrap());
    row.session_id = Set(Some(
        "00000000-0000-4000-8000-000000000011".parse().unwrap(),
    ));
    row.family_id = Set(Some(
        "00000000-0000-4000-8000-000000000012".parse().unwrap(),
    ));
    row.clone().insert(db).await.unwrap();
    row.jti = Set("mcp-access".into());
    row.token_use = Set("mcp_access".into());
    row.grant_id = Set("00000000-0000-4000-8000-000000000013".parse().unwrap());
    row.session_id = Set(None);
    row.family_id = Set(None);
    row.insert(db).await.unwrap();
}

#[tokio::test]
async fn security_t18_unknown_revoked_expired_or_disabled_credentials_are_rejected() {
    use auth::security::{credential::CredentialRef, error::SecurityError};
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    insert_machine_access(&db).await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    );
    let reference = CredentialRef::new("https://idp.test", "parent").unwrap();
    assert_eq!(
        lifecycle
            .check_registered(
                &CredentialRef::new("https://idp.test", "missing").unwrap(),
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    assert_eq!(
        lifecycle
            .check_registered(&reference, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    enable_service_binding(&db).await;
    let status = lifecycle
        .check_registered(&reference, &security_budget(&clock))
        .await
        .unwrap();
    assert!(status.active);
    assert_eq!(status.claims.sub, "service");
    assert_eq!(status.validity.checked_at(), 2000);
    assert_eq!(status.validity.valid_until(), 7000);
    for (sql, expected, reset) in [
        (
            "UPDATE security_credential SET revoked_at=2000",
            SecurityError::CredentialRevoked,
            "UPDATE security_credential SET revoked_at=NULL",
        ),
        (
            "UPDATE security_grant SET revoked_at=2000",
            SecurityError::CredentialRevoked,
            "UPDATE security_grant SET revoked_at=NULL",
        ),
        (
            "UPDATE security_credential SET expires_at=3000",
            SecurityError::InvalidCredential,
            "UPDATE security_credential SET expires_at=9000",
        ),
        (
            "UPDATE security_credential SET not_before=3000",
            SecurityError::InvalidCredential,
            "UPDATE security_credential SET not_before=NULL",
        ),
        (
            "UPDATE security_binding SET enabled=false",
            SecurityError::CredentialRevoked,
            "UPDATE security_binding SET enabled=true",
        ),
        (
            "UPDATE security_binding SET identity_binding='{\"kind\":\"gateway\",\"gateway_id\":\"other\"}'",
            SecurityError::InvalidCredential,
            "UPDATE security_binding SET identity_binding='{\"kind\":\"none\"}'",
        ),
        (
            "UPDATE security_binding SET allowed_operations='[\"device:read\"]'",
            SecurityError::DependencyUnavailable,
            "UPDATE security_binding SET allowed_operations='[]'",
        ),
        (
            "UPDATE security_credential SET bounds='{}'",
            SecurityError::DependencyUnavailable,
            "UPDATE security_credential SET bounds='{\"grants\":[]}'",
        ),
        (
            "UPDATE security_credential SET issued_at=1001",
            SecurityError::DependencyUnavailable,
            "UPDATE security_credential SET issued_at=1000",
        ),
    ] {
        execute(&db, sql).await.unwrap();
        assert_eq!(
            lifecycle
                .check_registered(&reference, &security_budget(&clock))
                .await
                .err(),
            Some(expected),
            "{sql}"
        );
        execute(&db, reset).await.unwrap();
    }
}

#[tokio::test]
async fn security_t18_session_family_and_grant_revocation_keep_mcp_roots_independent() {
    use auth::security::{credential::CredentialRef, error::SecurityError};
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    insert_machine_access(&db).await;
    insert_user_access(&db).await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    );
    let user = CredentialRef::new("https://idp.test", "user-access").unwrap();
    let mcp = CredentialRef::new("https://idp.test", "mcp-access").unwrap();
    assert!(
        lifecycle
            .check_registered(&user, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
    assert!(
        lifecycle
            .check_registered(&mcp, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
    for table in ["security_session", "security_refresh_family"] {
        execute(&db, &format!("UPDATE {table} SET revoked_at=2000"))
            .await
            .unwrap();
        assert_eq!(
            lifecycle
                .check_registered(&user, &security_budget(&clock))
                .await
                .err(),
            Some(SecurityError::CredentialRevoked)
        );
        assert!(
            lifecycle
                .check_registered(&mcp, &security_budget(&clock))
                .await
                .unwrap()
                .active
        );
        execute(&db, &format!("UPDATE {table} SET revoked_at=NULL"))
            .await
            .unwrap();
    }
    execute(
        &db,
        "UPDATE security_grant SET revoked_at=2000 WHERE grant_kind='mcp'",
    )
    .await
    .unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&mcp, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert!(
        lifecycle
            .check_registered(&user, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
    // Rotation's consumed source is audit metadata, never a new revocation parent.
    execute(&db,"UPDATE security_credential SET minted_by='{\"issuer\":\"https://idp.test\",\"jti\":\"consumed-refresh\"}' WHERE jti='user-access'").await.unwrap();
    assert!(
        lifecycle
            .check_registered(&user, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
}

#[tokio::test]
async fn security_t18_legacy_execution_credentials_are_rejected() {
    use auth::security::{credential::CredentialRef, error::SecurityError};
    use cobotusr::{
        entity::security_credential as credential,
        migration::{Migrator, MigratorTrait},
        service::security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    insert_machine_access(&db).await;
    enable_service_binding(&db).await;
    let mut row: credential::ActiveModel =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "parent".to_owned()))
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into();
    row.jti = Set("child".into());
    row.token_use = Set("gateway_operation".into());
    row.revocation_parent_jti = Set(Some("parent".into()));
    row.binding = Set(serde_json::json!({"kind":"gateway","gateway_id":"g"}));
    row.audience = Set("legacy-gateway:g".into());
    row.clone().insert(&db).await.unwrap();
    row.jti = Set("grandchild".into());
    row.revocation_parent_jti = Set(Some("child".into()));
    row.insert(&db).await.unwrap();
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    );
    for jti in ["child", "grandchild"] {
        assert_eq!(
            lifecycle
                .check_registered(
                    &CredentialRef::new("https://idp.test", jti).unwrap(),
                    &security_budget(&clock)
                )
                .await
                .err(),
            Some(SecurityError::InvalidCredential)
        );
    }
}

struct SecuritySigningFixture {
    encoding: jsonwebtoken::EncodingKey,
    jwks: serde_json::Value,
    public_pem: String,
}
fn security_signing_fixture() -> &'static SecuritySigningFixture {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use rsa::{
        pkcs8::{EncodePrivateKey, EncodePublicKey},
        traits::PublicKeyParts,
    };
    static FIXTURE: std::sync::OnceLock<SecuritySigningFixture> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(|| {
        let private=rsa::PrivateKey::new(&mut getrandom::SysRng,2048).unwrap();
        let pem=private.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let public=rsa::PublicKey::from(&private);
        let n=URL_SAFE_NO_PAD.encode(private.n_bytes());let e=URL_SAFE_NO_PAD.encode(private.e_bytes());
        SecuritySigningFixture {encoding:jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            jwks:serde_json::json!({"keys":[{"kid":"key","n":n,"e":e},{"kid":"other-key","n":n,"e":e}]}),
            public_pem:public.to_public_key_pem(rsa::pkcs8::LineEnding::LF).unwrap()}
    })
}
async fn security_jwks() -> (
    std::sync::Arc<auth::jwks::JwksClient>,
    tokio::sync::oneshot::Sender<()>,
) {
    let value = security_signing_fixture().jwks.clone();
    let app = axum::Router::new().route(
        "/jwks",
        axum::routing::get(move || {
            let value = value.clone();
            async move { axum::Json(value) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/jwks", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel();
    tokio::spawn(async {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    (
        std::sync::Arc::new(
            auth::jwks::JwksClient::new(url, Some("aud".into()))
                .await
                .unwrap(),
        ),
        stop,
    )
}
fn sign_security_claims(
    claims: &auth::security::credential::SecurityAccessClaims,
    kid: &str,
) -> auth::security::identity::SecretToken {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(kid.into());
    header.typ = Some("at+jwt".into());
    auth::security::identity::SecretToken::new(
        jsonwebtoken::encode(&header, claims, &security_signing_fixture().encoding).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn security_t18_verified_jwt_must_match_registry_claims_and_signing_key() {
    use auth::security::{
        credential::{
            CredentialExpectation, CredentialProfile, CredentialRef, CurrentCredentialChecker,
            StrictAccessState,
        },
        error::SecurityError,
        identity::{PrincipalKind, TokenUse},
    };
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    insert_machine_access(&db).await;
    enable_service_binding(&db).await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let lifecycle = Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    ));
    let (jwks, _stop) = security_jwks().await;
    let expected = CredentialExpectation::new(
        "https://idp.test",
        "aud",
        "app",
        vec![CredentialProfile::new(PrincipalKind::Machine, TokenUse::SecurityService).unwrap()],
    )
    .unwrap();
    let claims = lifecycle
        .check_registered(
            &CredentialRef::new("https://idp.test", "parent").unwrap(),
            &security_budget(&clock),
        )
        .await
        .unwrap()
        .claims;
    let token = sign_security_claims(&claims, "key");
    let verified = jwks
        .verify_credential(&token, &expected, clock.as_ref())
        .unwrap();
    assert!(
        lifecycle
            .check_current(&verified, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
    let other_key = sign_security_claims(&claims, "other-key");
    let verified = jwks
        .verify_credential(&other_key, &expected, clock.as_ref())
        .unwrap();
    assert_eq!(
        lifecycle
            .check_current(&verified, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    let mut changed = claims.clone();
    changed.jti = "unknown-jti".into();
    let changed = sign_security_claims(&changed, "key");
    let verified = jwks
        .verify_credential(&changed, &expected, clock.as_ref())
        .unwrap();
    assert_eq!(
        lifecycle
            .check_current(&verified, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    let mut changed = claims.clone();
    changed.exp += 1;
    let changed = sign_security_claims(&changed, "key");
    let verified = jwks
        .verify_credential(&changed, &expected, clock.as_ref())
        .unwrap();
    assert_eq!(
        lifecycle
            .check_current(&verified, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    let state = StrictAccessState::new(jwks, expected, clock.clone(), lifecycle);
    let headers = axum::http::HeaderMap::from_iter([(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token.expose_secret()).parse().unwrap(),
    )]);
    assert_eq!(
        state
            .authenticate(&headers)
            .await
            .unwrap()
            .principal()
            .subject(),
        "service"
    );
    execute(
        &db,
        "UPDATE security_credential SET revoked_at=2000 WHERE jti='parent'",
    )
    .await
    .unwrap();
    assert_eq!(
        state.authenticate(&headers).await.err(),
        Some(SecurityError::CredentialRevoked)
    );
}

#[tokio::test]
async fn security_t18_database_locks_outages_and_unhealthy_clocks_fail_closed() {
    use auth::security::{credential::CredentialRef, error::SecurityError};
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    use sea_orm::TransactionTrait;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    insert_machine_access(&db).await;
    enable_service_binding(&db).await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    );
    let reference = CredentialRef::new("https://idp.test", "parent").unwrap();
    let budget = security_budget(&clock);
    clock.set_synchronized(false);
    assert_eq!(
        lifecycle.check_registered(&reference, &budget).await.err(),
        Some(SecurityError::DependencyUnavailable)
    );
    clock.set_synchronized(true);
    let lock = db.begin().await.unwrap();
    lock.execute_raw(Statement::from_string(
        DbBackend::Postgres,
        "LOCK TABLE security_credential IN ACCESS EXCLUSIVE MODE".to_owned(),
    ))
    .await
    .unwrap();
    let short = security_budget(&clock)
        .child(Duration::from_millis(100))
        .unwrap();
    let start = Instant::now();
    let result = lifecycle.check_registered(&reference, &short).await.err();
    assert!(matches!(
        result,
        Some(SecurityError::DeadlineExceeded | SecurityError::DependencyUnavailable)
    ));
    assert!(start.elapsed() < Duration::from_secs(1));
    lock.rollback().await.unwrap();
    assert!(
        lifecycle
            .check_registered(&reference, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
    db.close().await.unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&reference, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::DependencyUnavailable)
    );
}

async fn authenticated_policy_user(
    db: &DatabaseConnection,
    application: &str,
    bounds: auth::security::policy::CredentialBounds,
    clock: std::sync::Arc<crate::cobotusr::common::SecurityTestClock>,
    role_id: i32,
) -> auth::security::credential::AuthenticatedContext {
    use auth::security::{
        credential::{
            CredentialExpectation, CredentialProfile, SecurityAccessClaims, StrictAccessState,
        },
        identity::{IdentityBinding, PrincipalKind, TokenUse},
    };
    use cobotusr::{
        entity::security_credential as credential,
        service::security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
    };
    use sea_orm::{ActiveModelTrait, Set};
    use std::sync::Arc;
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO \"user\" (id,username,role_id,create_time,update_time) VALUES (100,'policy-user',$1,NOW(),NOW())",[role_id.into()])).await.unwrap();
    let grant_id = uuid::Uuid::new_v4();
    let session_id = uuid::Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at) VALUES ($1,'https://idp.test','user','100',$2,'session',1000)",
        [grant_id.into(),application.into()])).await.unwrap();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO security_session (id,issuer,principal_kind,subject,created_at) VALUES ($1,'https://idp.test','user','100',1000)",[session_id.into()])).await.unwrap();
    let claims = SecurityAccessClaims {
        iss: "https://idp.test".into(),
        sub: "100".into(),
        aud: application.into(),
        iat: 1,
        exp: 30,
        nbf: None,
        jti: "policy-access".into(),
        principal_kind: PrincipalKind::User,
        token_use: TokenUse::UserSession,
        application: application.into(),
        grant_id: grant_id.to_string(),
        bounds: bounds.clone(),
        binding: IdentityBinding::None,
        session_id: Some(session_id.to_string()),
        family_id: None,
    };
    credential::ActiveModel {
        issuer: Set(claims.iss.clone()),
        jti: Set(claims.jti.clone()),
        principal_kind: Set("user".into()),
        subject: Set("100".into()),
        credential_kind: Set("access".into()),
        token_use: Set("user_session".into()),
        audience: Set(application.into()),
        application: Set(application.into()),
        bounds: Set(serde_json::to_value(bounds).unwrap()),
        binding: Set(serde_json::json!({"kind":"none"})),
        grant_id: Set(grant_id),
        session_id: Set(Some(session_id)),
        family_id: Set(None),
        paired_access_jti: Set(None),
        oidc_client_id: Set(None),
        revocation_parent_jti: Set(None),
        minted_by: Set(None),
        signing_key_id: Set(Some("key".into())),
        secret_digest: Set(None),
        issued_at: Set(1000),
        not_before: Set(None),
        expires_at: Set(30000),
        revoked_at: Set(None),
        consumed_at: Set(None),
        replacement_jti: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
    let (jwks, _stop) = security_jwks().await;
    let checker = Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    ));
    let expected = CredentialExpectation::new(
        "https://idp.test",
        application,
        application,
        vec![CredentialProfile::new(PrincipalKind::User, TokenUse::UserSession).unwrap()],
    )
    .unwrap();
    let state = StrictAccessState::new(jwks, expected, clock, checker);
    let token = sign_security_claims(&claims, "key");
    let headers = axum::http::HeaderMap::from_iter([(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token.expose_secret()).parse().unwrap(),
    )]);
    state.authenticate(&headers).await.unwrap()
}
async fn policy_authority(db: &DatabaseConnection, method: &str, path: &str) -> i32 {
    use cobotusr::entity::authority;
    use sea_orm::{ActiveModelTrait, Set};
    let now = chrono::Utc::now().naive_utc();
    authority::ActiveModel {
        id: sea_orm::NotSet,
        name: Set(format!("{method} {path}")),
        authority_uri: Set(path.into()),
        method: Set(method.into()),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(db)
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn security_t19_uri_adapter_rejects_unknown_and_user_deny_beats_superadmin() {
    use auth::security::policy::{
        CredentialBounds, PolicyAllowReason, PolicyDenyReason, PolicyEffect, ResourceScope,
        ResourceSelector, ScopedOperation,
    };
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::{
            authority::{uri_operation, uri_resource},
            security::{authorization::AuthorizationService, repository::PgSecurityRepository},
        },
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let authority = policy_authority(&db, "GET", "/security-test").await;
    let resource = uri_resource("cobotusr", "GET", "/security-test").unwrap();
    let bounds = CredentialBounds::new(vec![
        ScopedOperation::new(
            uri_operation(authority).unwrap(),
            vec![
                ResourceSelector::new(
                    "cobotusr",
                    ResourceScope::Exact {
                        target: resource.target().clone(),
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    ]);
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let context = authenticated_policy_user(&db, "cobotusr", bounds, clock.clone(), 3).await;
    let service = AuthorizationService::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
        "cobotusr",
        "cobotusr",
    )
    .unwrap();
    assert_eq!(
        service
            .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::NoGrant)
    );
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) VALUES (100,$1,true,NOW(),NOW())",[authority.into()])).await.unwrap();
    assert_eq!(
        service
            .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Allow(PolicyAllowReason::Principal)
    );
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO authority_role (role_id,authority_id,create_time,update_time) VALUES (3,$1,NOW(),NOW())",[authority.into()])).await.unwrap();
    execute(&db, "UPDATE user_authority SET allow=false")
        .await
        .unwrap();
    assert_eq!(
        service
            .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
    );
    execute(&db, "UPDATE \"user\" SET role_id=1 WHERE id=100")
        .await
        .unwrap();
    assert_eq!(
        service
            .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
    );
    execute(&db, "DELETE FROM user_authority").await.unwrap();
    assert_eq!(
        service
            .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Allow(PolicyAllowReason::Role)
    );
    for (method, path) in [("POST", "/security-test"), ("GET", "/not-registered")] {
        assert_eq!(
            service
                .authorize_uri(&context, method, path, &security_budget(&clock))
                .await
                .unwrap()
                .effect,
            PolicyEffect::Deny(PolicyDenyReason::UnknownOperation)
        );
    }
    for (application, audience) in [("other", "cobotusr"), ("cobotusr", "business-audience")] {
        let other = AuthorizationService::new(
            Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
            clock.clone(),
            application,
            audience,
        )
        .unwrap();
        assert_eq!(
            other
                .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
                .await
                .err(),
            Some(auth::security::error::SecurityError::Forbidden)
        );
    }
}

#[tokio::test]
async fn security_t19_current_policy_cannot_extend_original_bounds_freshness_or_budget() {
    use auth::security::{
        error::SecurityError,
        policy::{
            CredentialBounds, PolicyDenyReason, PolicyEffect, ResourceScope, ResourceSelector,
            ScopedOperation,
        },
    };
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::{
            authority::{uri_operation, uri_resource},
            security::{authorization::AuthorizationService, repository::PgSecurityRepository},
        },
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let authority = policy_authority(&db, "GET", "/security-test").await;
    policy_authority(&db, "POST", "/new-security-test").await;
    let resource = uri_resource("cobotusr", "GET", "/security-test").unwrap();
    let bounds = CredentialBounds::new(vec![
        ScopedOperation::new(
            uri_operation(authority).unwrap(),
            vec![
                ResourceSelector::new(
                    "cobotusr",
                    ResourceScope::Exact {
                        target: resource.target().clone(),
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    ]);
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let context = authenticated_policy_user(&db, "cobotusr", bounds, clock.clone(), 1).await;
    let service = AuthorizationService::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
        "cobotusr",
        "cobotusr",
    )
    .unwrap();
    assert_eq!(
        service
            .authorize_uri(
                &context,
                "POST",
                "/new-security-test",
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::CredentialBounds)
    );
    let first = service
        .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
        .await
        .unwrap();
    clock.advance(1100);
    let second = service
        .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
        .await
        .unwrap();
    assert_eq!(first.validity.valid_until(), second.validity.valid_until());
    clock.advance(1000);
    assert_eq!(
        service
            .authorize_uri(&context, "GET", "/security-test", &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::DeadlineExceeded)
    );
}

#[test]
fn security_t19_api_prefix_only_strips_complete_path_segment() {
    assert_eq!(
        cobotusr::rbac::get_pure_uri("/api".into(), "/apiary/users?x=1".into()),
        "/apiary/users"
    );
    assert_eq!(
        cobotusr::rbac::get_pure_uri("/api".into(), "/api/users?x=1".into()),
        "/users"
    );
    assert_eq!(
        cobotusr::rbac::get_pure_uri("/api".into(), "/api?x=1".into()),
        "/"
    );
}

#[tokio::test]
async fn security_t19_registration_repairs_missing_methods_without_granting_user_management() {
    use cobotusr::{
        entity::{authority, authority_role},
        migration::{Migrator, MigratorTrait},
    };
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    let migrations = Migrator::migrations();
    let operation_seed = migrations
        .iter()
        .position(|migration| migration.name() == "m20261107_000003_security_operations")
        .unwrap();
    assert!(
        operation_seed
            > migrations
                .iter()
                .position(|migration| migration.name() == "m20261107_000001_security_identity")
                .unwrap()
    );
    Migrator::up(&db, Some(operation_seed as u32))
        .await
        .unwrap();
    // Simulate a pre-existing administrator-managed registration with no User
    // grant. Backfilling other omitted methods must preserve this restriction.
    let restricted = authority::Entity::find()
        .filter(authority::Column::Method.eq("GET"))
        .filter(authority::Column::AuthorityUri.eq("/user"))
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .id;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM authority_role WHERE role_id=3 AND authority_id=$1",
        [restricted.into()],
    ))
    .await
    .unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    assert!(
        authority_role::Entity::find_by_id((3, restricted))
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );
    for (method, path) in [
        ("GET", "/authority"),
        ("PATCH", "/authority"),
        ("DELETE", "/authority"),
        ("PATCH", "/user"),
        ("PATCH", "/role"),
        ("DELETE", "/application"),
    ] {
        let row = authority::Entity::find()
            .filter(authority::Column::Method.eq(method))
            .filter(authority::Column::AuthorityUri.eq(path))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(
            authority_role::Entity::find()
                .filter(authority_role::Column::AuthorityId.eq(row.id))
                .filter(authority_role::Column::RoleId.eq(3))
                .one(&db)
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn security_t19_live_legacy_routes_cannot_bypass_missing_registration() {
    let fixture = crate::cobotusr::common::IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let db = fixture.db.clone();
    let base = format!("{}/api/v1", fixture.base_url);
    let client = fixture.client.clone();
    let before = client
        .get(format!("{base}/user"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(before.status().is_success());
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT id FROM authority WHERE method='GET' AND authority_uri='/user'".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    let id: i32 = row.try_get("", "id").unwrap();
    for sql in [
        "DELETE FROM authority_role WHERE authority_id=$1",
        "DELETE FROM user_authority WHERE authority_id=$1",
        "DELETE FROM authority WHERE id=$1",
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [id.into()],
        ))
        .await
        .unwrap();
    }
    let after = client
        .get(format!("{base}/user"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), reqwest::StatusCode::FORBIDDEN);
    let unknown: serde_json::Value = client
        .post(format!("{base}/rbac/abac"))
        .json(&serde_json::json!({"method":"GET","req_path":"/unregistered"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(unknown["allow"], false);
    let legacy = client
        .post(format!("{base}/enforce"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"application":"resource-app","subject":1,"action":"read"}))
        .send()
        .await
        .unwrap();
    assert_eq!(legacy.status(), reqwest::StatusCode::UNAUTHORIZED);
    fixture.close().await;
}

fn policy_device(gateway: &str, device: &str) -> auth::security::policy::ResourceRef {
    use auth::security::policy::{ResourceRef, ResourceTarget};
    ResourceRef::new(
        "resource-app",
        ResourceTarget::Device {
            gateway_id: gateway.into(),
            device_id: device.into(),
        },
    )
    .unwrap()
}
fn scoped_grant(
    operation: &str,
    scope: auth::security::policy::ResourceScope,
) -> auth::security::policy::ScopedOperation {
    use auth::security::policy::{OperationId, ResourceSelector, ScopedOperation};
    ScopedOperation::new(
        OperationId::new(operation).unwrap(),
        vec![ResourceSelector::new("resource-app", scope).unwrap()],
    )
    .unwrap()
}
fn policy_context(
    operation: &str,
    resource: auth::security::policy::ResourceRef,
) -> auth::security::policy::OperationContext {
    auth::security::policy::OperationContext {
        operation: auth::security::policy::OperationId::new(operation).unwrap(),
        resource,
        facts: Default::default(),
    }
}
async fn application_permission(
    db: &DatabaseConnection,
    name: &str,
    action: &str,
    kind: &str,
    scope: Option<auth::security::policy::ResourceScope>,
) -> i32 {
    use auth::security::policy::ResourceSelector;
    use cobotusr::entity::{application, permission};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    let app = application::Entity::find()
        .filter(application::Column::AppKey.eq("resource-app"))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let now = chrono::Utc::now().naive_utc();
    permission::ActiveModel {
        id: sea_orm::NotSet,
        application_id: Set(app.id),
        name: Set(name.into()),
        action: Set(action.into()),
        resource_type: Set(Some(kind.into())),
        resource_pattern: Set(scope.map(|scope| {
            serde_json::to_string(&ResourceSelector::new("resource-app", scope).unwrap()).unwrap()
        })),
        description: Set(None),
        create_time: Set(now),
        update_time: Set(now),
    }
    .insert(db)
    .await
    .unwrap()
    .id
}
async fn grant_role_permission(db: &DatabaseConnection, role: i32, permission: i32) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO role_permission (role_id,permission_id,create_time) VALUES ($1,$2,NOW())",
        [role.into(), permission.into()],
    ))
    .await
    .unwrap();
}
async fn user_permission_override(db: &DatabaseConnection, permission: i32, allow: bool) {
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO user_permission (user_id,permission_id,allow,create_time,update_time) VALUES (100,$1,$2,NOW(),NOW()) ON CONFLICT (user_id,permission_id) DO UPDATE SET allow=$2",[permission.into(),allow.into()])).await.unwrap();
}

#[tokio::test]
async fn security_t20_overlapping_permissions_scan_denies_before_any_allow() {
    use auth::security::policy::{
        CredentialBounds, PolicyAllowReason, PolicyDenyReason, PolicyEffect, ResourceKind,
        ResourceScope,
    };
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::{
            authorization::AuthorizationService, repository::PgSecurityRepository,
        },
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let broad = ResourceScope::Application {
        kind: ResourceKind::Device,
    };
    let target = policy_device("g1", "d");
    let allow =
        application_permission(&db, "broad-read", "read", "device", Some(broad.clone())).await;
    let deny = application_permission(
        &db,
        "deny-one-device",
        "read",
        "device",
        Some(ResourceScope::Exact {
            target: target.target().clone(),
        }),
    )
    .await;
    grant_role_permission(&db, 3, allow).await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let context = authenticated_policy_user(
        &db,
        "resource-app",
        CredentialBounds::new(vec![scoped_grant("device:read", broad)]),
        clock.clone(),
        3,
    )
    .await;
    let service = AuthorizationService::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
        "cobotusr",
        "cobotusr",
    )
    .unwrap();
    user_permission_override(&db, deny, false).await;
    assert_eq!(
        service
            .authorize_application(
                &context,
                &policy_context("device:read", target.clone()),
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
    );
    assert_eq!(
        service
            .authorize_application(
                &context,
                &policy_context("device:read", policy_device("g2", "d")),
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Allow(PolicyAllowReason::Role)
    );
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM role_permission WHERE role_id=3 AND permission_id=$1",
        [allow.into()],
    ))
    .await
    .unwrap();
    assert_eq!(
        service
            .authorize_application(
                &context,
                &policy_context("device:read", policy_device("g2", "d")),
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::NoGrant)
    );
    grant_role_permission(&db, 3, allow).await;
    user_permission_override(&db, allow, true).await;
    assert_eq!(
        service
            .authorize_application(
                &context,
                &policy_context("device:read", target.clone()),
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
    );
    execute(&db, "UPDATE \"user\" SET role_id=1 WHERE id=100")
        .await
        .unwrap();
    assert_eq!(
        service
            .authorize_application(
                &context,
                &policy_context("device:read", target),
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::ExplicitDeny)
    );
}

#[tokio::test]
async fn security_t20_bounds_missing_scopes_and_unknown_operations_fail_closed() {
    use auth::security::{
        error::SecurityError,
        policy::{CredentialBounds, PolicyDenyReason, PolicyEffect, ResourceKind, ResourceScope},
    };
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::{
            authorization::AuthorizationService, repository::PgSecurityRepository,
        },
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let target = policy_device("g1", "d");
    let permission = application_permission(
        &db,
        "all-device-read",
        "read",
        "device",
        Some(ResourceScope::Application {
            kind: ResourceKind::Device,
        }),
    )
    .await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let context = authenticated_policy_user(
        &db,
        "resource-app",
        CredentialBounds::new(vec![scoped_grant(
            "device:read",
            ResourceScope::Exact {
                target: target.target().clone(),
            },
        )]),
        clock.clone(),
        1,
    )
    .await;
    let service = AuthorizationService::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
        "cobotusr",
        "cobotusr",
    )
    .unwrap();
    assert_eq!(
        service
            .authorize_application(
                &context,
                &policy_context("device:read", policy_device("g2", "d")),
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::CredentialBounds)
    );
    assert_eq!(
        service
            .authorize_application(
                &context,
                &policy_context("device:write", target.clone()),
                &security_budget(&clock)
            )
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::UnknownOperation)
    );
    let requested = policy_context("device:read", target);
    for pattern in [
        None,
        Some("*"),
        Some("g1:*"),
        Some(
            "{\"application\":\"other\",\"scope\":{\"scope\":\"application\",\"kind\":\"device\"}}",
        ),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE permission SET resource_pattern=$1 WHERE id=$2",
            [pattern.map(str::to_owned).into(), permission.into()],
        ))
        .await
        .unwrap();
        assert_eq!(
            service
                .authorize_application(&context, &requested, &security_budget(&clock))
                .await
                .err(),
            Some(SecurityError::DependencyUnavailable)
        );
    }
    let legacy = cobotusr::service::permission::EnforceRequest {
        application: "resource-app".into(),
        subject: 100,
        action: "read".into(),
        resource_type: None,
        resource_id: None,
    };
    assert!(matches!(
        cobotusr::service::permission::enforce(&db, &legacy).await,
        Err(cobotusr::error::AppError::Unauthorized(_))
    ));
}

#[tokio::test]
async fn security_t20_task_read_requires_owner_and_read_any_requires_separate_grant() {
    use auth::security::{
        identity::{PrincipalKind, PrincipalRef},
        policy::{
            CredentialBounds, PolicyAllowReason, PolicyDenyReason, PolicyEffect, ResourceKind,
            ResourceRef, ResourceScope, ResourceTarget,
        },
    };
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::security::{
            authorization::AuthorizationService, repository::PgSecurityRepository,
        },
    };
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let scope = ResourceScope::Application {
        kind: ResourceKind::Task,
    };
    let read =
        application_permission(&db, "own-tasks", "task:read", "task", Some(scope.clone())).await;
    let any = application_permission(
        &db,
        "manage-tasks",
        "task:read-any",
        "task",
        Some(scope.clone()),
    )
    .await;
    grant_role_permission(&db, 3, read).await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let context = authenticated_policy_user(
        &db,
        "resource-app",
        CredentialBounds::new(vec![
            scoped_grant("task:read", scope.clone()),
            scoped_grant("task:read-any", scope),
        ]),
        clock.clone(),
        3,
    )
    .await;
    let service = AuthorizationService::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
        "cobotusr",
        "cobotusr",
    )
    .unwrap();
    let resource = ResourceRef::new(
        "resource-app",
        ResourceTarget::Task {
            gateway_id: "g".into(),
            device_id: "d".into(),
            task_id: "task".into(),
        },
    )
    .unwrap();
    let mut request = policy_context("task:read", resource.clone());
    assert_eq!(
        service
            .authorize_application(&context, &request, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::MissingResourceFacts)
    );
    request.facts.owner = Some(context.principal().clone());
    assert_eq!(
        service
            .authorize_application(&context, &request, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Allow(PolicyAllowReason::Role)
    );
    request.facts.owner =
        Some(PrincipalRef::new("https://idp.test", PrincipalKind::User, "101").unwrap());
    assert_eq!(
        service
            .authorize_application(&context, &request, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::ResourceConstraint)
    );
    let mut admin = policy_context("task:read-any", resource);
    admin.facts = request.facts;
    assert_eq!(
        service
            .authorize_application(&context, &admin, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::NoGrant)
    );
    grant_role_permission(&db, 3, any).await;
    assert_eq!(
        service
            .authorize_application(&context, &admin, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Allow(PolicyAllowReason::Role)
    );
}

#[tokio::test]
async fn security_t20_long_structured_scopes_are_persisted_without_alias_concatenation() {
    use auth::security::policy::ResourceScope;
    use cobotusr::{
        entity::permission,
        migration::{Migrator, MigratorTrait},
    };
    use sea_orm::EntityTrait;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let target = policy_device(&"g:".repeat(100), &"d:".repeat(100));
    let id = application_permission(
        &db,
        "long-target",
        "read",
        "device",
        Some(ResourceScope::Exact {
            target: target.target().clone(),
        }),
    )
    .await;
    let row = permission::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let raw = row.resource_pattern.unwrap();
    assert!(raw.len() > 255);
    let selector: auth::security::policy::ResourceSelector = serde_json::from_str(&raw).unwrap();
    assert!(selector.matches(&target));
    assert!(!selector.matches(&policy_device(
        "g",
        &format!("{}:{}", "g:".repeat(99), "d:".repeat(100))
    )));
}

#[tokio::test]
async fn security_t20_machine_uses_current_binding_without_falling_back_to_same_named_user() {
    use auth::security::{
        credential::{CredentialExpectation, CredentialProfile, StrictAccessState},
        error::SecurityError,
        identity::{PrincipalKind, TokenUse},
        policy::{
            CredentialBounds, PolicyAllowReason, PolicyDenyReason, PolicyEffect, ResourceKind,
            ResourceScope,
        },
    };
    use cobotusr::{
        entity::{security_binding, security_credential as credential},
        migration::{Migrator, MigratorTrait},
        service::security::{
            authorization::AuthorizationService, lifecycle::CredentialLifecycle,
            repository::PgSecurityRepository,
        },
    };
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    let broad = ResourceScope::Application {
        kind: ResourceKind::Device,
    };
    application_permission(
        &db,
        "known-device-read",
        "read",
        "device",
        Some(broad.clone()),
    )
    .await;
    let clock = Arc::new(crate::cobotusr::common::SecurityTestClock::new(2000));
    let user = authenticated_policy_user(
        &db,
        "resource-app",
        CredentialBounds::new(vec![scoped_grant("device:read", broad)]),
        clock.clone(),
        1,
    )
    .await;
    let grant = uuid::Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at) VALUES ($1,'https://idp.test','machine','100','resource-app','service',1000)",[grant.into()])).await.unwrap();
    let mut row: credential::ActiveModel =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "policy-access".to_owned()))
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into();
    row.jti = Set("machine-access".into());
    row.principal_kind = Set("machine".into());
    row.token_use = Set("security_service".into());
    row.grant_id = Set(grant);
    row.session_id = Set(None);
    row.family_id = Set(None);
    row.insert(&db).await.unwrap();
    let current = scoped_grant(
        "device:read",
        ResourceScope::Exact {
            target: policy_device("g1", "d").target().clone(),
        },
    );
    security_binding::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        issuer: Set("https://idp.test".into()),
        client_id: Set("100".into()),
        token_use: Set("security_service".into()),
        application: Set("resource-app".into()),
        audience: Set("resource-app".into()),
        allowed_operations: Set(serde_json::to_value(vec![current]).unwrap()),
        identity_binding: Set(serde_json::json!({"kind":"none"})),
        enabled: Set(true),
    }
    .insert(&db)
    .await
    .unwrap();
    let mut claims = user.verified().claims().clone();
    claims.jti = "machine-access".into();
    claims.principal_kind = PrincipalKind::Machine;
    claims.token_use = TokenUse::SecurityService;
    claims.grant_id = grant.to_string();
    claims.session_id = None;
    claims.family_id = None;
    let (jwks, _stop) = security_jwks().await;
    let checker = Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    ));
    let expected = CredentialExpectation::new(
        "https://idp.test",
        "resource-app",
        "resource-app",
        vec![CredentialProfile::new(PrincipalKind::Machine, TokenUse::SecurityService).unwrap()],
    )
    .unwrap();
    let state = StrictAccessState::new(jwks, expected, clock.clone(), checker);
    let token = sign_security_claims(&claims, "key");
    let headers = axum::http::HeaderMap::from_iter([(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", token.expose_secret()).parse().unwrap(),
    )]);
    let machine = state.authenticate(&headers).await.unwrap();
    let service = AuthorizationService::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
        "cobotusr",
        "cobotusr",
    )
    .unwrap();
    let g1 = policy_context("device:read", policy_device("g1", "d"));
    let g2 = policy_context("device:read", policy_device("g2", "d"));
    assert_eq!(
        service
            .authorize_application(&machine, &g1, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Allow(PolicyAllowReason::Principal)
    );
    assert_eq!(
        service
            .authorize_application(&machine, &g2, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::NoGrant)
    );
    execute(&db, "UPDATE security_binding SET allowed_operations='[]'")
        .await
        .unwrap();
    assert_eq!(
        service
            .authorize_application(&machine, &g1, &security_budget(&clock))
            .await
            .unwrap()
            .effect,
        PolicyEffect::Deny(PolicyDenyReason::NoGrant)
    );
    execute(&db, "UPDATE security_binding SET enabled=false")
        .await
        .unwrap();
    assert_eq!(
        service
            .authorize_application(&machine, &g1, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
}

struct SessionTestSigner {
    delay: std::time::Duration,
    fail: std::sync::atomic::AtomicBool,
}
impl SessionTestSigner {
    fn new() -> Self {
        Self {
            delay: std::time::Duration::ZERO,
            fail: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
#[sea_orm::prelude::async_trait::async_trait]
impl cobotusr::service::security::lifecycle::SessionTokenSigner for SessionTestSigner {
    fn key_id(&self) -> &str {
        "key"
    }
    async fn sign_access(
        &self,
        claims: &auth::security::credential::SecurityAccessClaims,
    ) -> Result<auth::security::identity::SecretToken, auth::security::error::SecurityError> {
        tokio::time::sleep(self.delay).await;
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(auth::security::error::SecurityError::DependencyUnavailable);
        }
        Ok(sign_security_claims(claims, "key"))
    }
    async fn sign_id(
        &self,
        claims: &auth::security::credential::SecurityAccessClaims,
        client_id: &str,
        nonce: Option<&str>,
        _access: &auth::security::identity::SecretToken,
    ) -> Result<auth::security::identity::SecretToken, auth::security::error::SecurityError> {
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some("key".into());
        let id = auth::claims::IdTokenClaims {
            iss: claims.iss.clone(),
            sub: claims.sub.clone(),
            aud: Some(client_id.into()),
            iat: claims.iat,
            exp: claims.exp,
            nonce: nonce.map(str::to_owned),
            at_hash: None,
            ..Default::default()
        };
        auth::security::identity::SecretToken::new(
            jsonwebtoken::encode(&header, &id, &security_signing_fixture().encoding).unwrap(),
        )
    }
}
const SESSION_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const LOCAL_PASSWORD: &str = "test password";

async fn local_fixture() -> (
    testcontainers::ContainerAsync<testcontainers::GenericImage>,
    DatabaseConnection,
    std::sync::Arc<crate::cobotusr::common::SecurityTestClock>,
    std::sync::Arc<cobotusr::service::security::lifecycle::CredentialLifecycle>,
) {
    let fixture = session_fixture().await;
    // Fixed bcrypt fixture for "test password"; production registration uses DEFAULT_COST.
    let hash = "$2y$04$FAiHSb5/5zp.oDR1GjBYSex5RVBJYW1BdEVBwqpdg4Phfk1i7YIPu";
    fixture
        .1
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE \"user\" SET password=$1 WHERE id=1",
            [hash.into()],
        ))
        .await
        .unwrap();
    execute(
        &fixture.1,
        "SELECT setval(pg_get_serial_sequence('\"user\"','id'),100)",
    )
    .await
    .unwrap();
    fixture
}
async fn login_local_session(
    lifecycle: &cobotusr::service::security::lifecycle::CredentialLifecycle,
    signer: &dyn cobotusr::service::security::lifecycle::SessionTokenSigner,
    budget: &auth::security::budget::SecurityBudget,
) -> Result<
    cobotusr::service::security::lifecycle::LocalSession,
    auth::security::error::SecurityError,
> {
    use cobotusr::service::security::lifecycle::{LocalLoginRequest, LocalPassword};
    let password = LocalPassword::new(LOCAL_PASSWORD.into()).unwrap();
    lifecycle
        .login_local(
            LocalLoginRequest {
                username: "session-user",
                password: &password,
            },
            &session_config(),
            signer,
            budget,
        )
        .await
}
async fn register_local_session(
    lifecycle: &cobotusr::service::security::lifecycle::CredentialLifecycle,
    username: &str,
    signer: &dyn cobotusr::service::security::lifecycle::SessionTokenSigner,
    budget: &auth::security::budget::SecurityBudget,
) -> Result<
    cobotusr::service::security::lifecycle::LocalSession,
    auth::security::error::SecurityError,
> {
    use cobotusr::service::security::lifecycle::{LocalPassword, LocalRegisterRequest};
    let password = LocalPassword::new(LOCAL_PASSWORD.into()).unwrap();
    lifecycle
        .register_local(
            LocalRegisterRequest {
                username,
                password: &password,
                nickname: "local-user",
            },
            &session_config(),
            signer,
            budget,
        )
        .await
}
#[tokio::test]
async fn security_t24_local_registration_and_login_commit_registered_idp_sessions() {
    use auth::security::{credential::CredentialRef, identity::TokenUse};
    let (_container, db, clock, lifecycle) = local_fixture().await;
    execute(&db, "INSERT INTO authority (id,name,authority_uri,method,create_time,update_time) VALUES (9001,'local-allow','/local-allow','GET',NOW(),NOW()),(9002,'local-deny','/local-deny','GET',NOW(),NOW())").await.unwrap();
    execute(&db, "INSERT INTO user_authority (user_id,authority_id,allow,create_time,update_time) VALUES (1,9002,FALSE,NOW(),NOW())").await.unwrap();
    let signer = SessionTestSigner::new();
    let registered =
        register_local_session(&lifecycle, "local-new", &signer, &security_budget(&clock))
            .await
            .unwrap();
    assert_eq!(registered.user.role_id, 3);
    let logged = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap();
    for local in [&registered, &logged] {
        let claims = &local.tokens.claims;
        assert_eq!(claims.aud, "cobotusr");
        assert_eq!(claims.application, "cobotusr");
        assert_eq!(claims.token_use, TokenUse::UserSession);
        assert!(local.tokens.id_token.is_none());
        assert!(claims.session_id.is_some() && claims.family_id.is_some());
        assert_eq!(local.tokens.refresh_token.expose_secret().len(), 64);
        lifecycle
            .check_registered(
                &CredentialRef::new(&claims.iss, &claims.jti).unwrap(),
                &security_budget(&clock),
            )
            .await
            .unwrap();
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_audience(&["cobotusr"]);
        validation.set_issuer(&["https://idp.test"]);
        let decoded = jsonwebtoken::decode::<auth::security::credential::SecurityAccessClaims>(
            local.tokens.access_token.expose_secret(),
            &jsonwebtoken::DecodingKey::from_rsa_pem(
                security_signing_fixture().public_pem.as_bytes(),
            )
            .unwrap(),
            &validation,
        )
        .unwrap();
        assert_eq!(decoded.header.typ.as_deref(), Some("at+jwt"));
        assert_eq!(decoded.claims.jti, claims.jti);
    }
    let bounds = serde_json::to_string(&logged.tokens.claims.bounds).unwrap();
    assert!(bounds.contains("uri:9001"));
    assert!(
        !bounds.contains("uri:9002"),
        "explicit deny survives superadmin"
    );
    assert_eq!(
        session_count(&db, "security_credential WHERE oidc_client_id IS NULL").await,
        4
    );
    for table in ["refresh_token", "oauth_token"] {
        assert_eq!(session_count(&db, table).await, 0);
    }
}
#[tokio::test]
async fn security_t24_authentication_failures_and_signing_commit_failures_leave_no_session() {
    use auth::security::error::SecurityError;
    use cobotusr::service::security::lifecycle::{LocalLoginRequest, LocalPassword};
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = SessionTestSigner::new();
    for (username, password) in [("unknown", LOCAL_PASSWORD), ("session-user", "wrong")] {
        let password = LocalPassword::new(password.into()).unwrap();
        assert_eq!(
            lifecycle
                .login_local(
                    LocalLoginRequest {
                        username,
                        password: &password
                    },
                    &session_config(),
                    &signer,
                    &security_budget(&clock)
                )
                .await
                .err(),
            Some(SecurityError::InvalidCredential)
        );
    }
    signer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        register_local_session(
            &lifecycle,
            "signing-failed",
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    assert_eq!(
        login_local_session(&lifecycle, &signer, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    signer
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);
    execute(&db, "CREATE FUNCTION reject_local_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test commit rejected'; END $$").await.unwrap();
    execute(&db, "CREATE CONSTRAINT TRIGGER reject_local_commit AFTER INSERT ON security_credential DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_local_commit()").await.unwrap();
    assert_eq!(
        register_local_session(
            &lifecycle,
            "commit-failed",
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    assert_eq!(
        login_local_session(&lifecycle, &signer, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    assert_eq!(session_count(&db, "\"user\"").await, 1);
    for table in [
        "security_credential",
        "security_session",
        "security_grant",
        "security_refresh_family",
        "refresh_token",
    ] {
        assert_eq!(session_count(&db, table).await, 0);
    }
    execute(
        &db,
        "DROP TRIGGER reject_local_commit ON security_credential",
    )
    .await
    .unwrap();
    register_local_session(
        &lifecycle,
        "commit-failed",
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t24_concurrent_logins_for_one_account_all_succeed() {
    use cobotusr::service::security::lifecycle::{LocalLoginRequest, LocalPassword};
    let (_container, _db, clock, lifecycle) = local_fixture().await;
    let signer = std::sync::Arc::new(SessionTestSigner::new());
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(4));
    let mut jobs = vec![];
    for _ in 0..4 {
        let lifecycle = lifecycle.clone();
        let signer = signer.clone();
        let clock = clock.clone();
        let barrier = barrier.clone();
        jobs.push(tokio::spawn(async move {
            let password = LocalPassword::new(LOCAL_PASSWORD.into()).unwrap();
            barrier.wait().await;
            lifecycle
                .login_local(
                    LocalLoginRequest {
                        username: "session-user",
                        password: &password,
                    },
                    &session_config(),
                    &*signer,
                    &security_budget(&clock),
                )
                .await
                .map(|session| session.tokens.refresh_token.expose_secret().to_owned())
        }));
    }
    let mut refresh = vec![];
    for job in jobs {
        refresh.push(
            job.await
                .unwrap()
                .expect("a concurrent login for the same account still mints"),
        );
    }
    refresh.sort();
    refresh.dedup();
    assert_eq!(refresh.len(), 4, "each login mints its own session");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t24_concurrent_registration_and_local_refresh_have_one_replacement() {
    use auth::security::{error::SecurityError, identity::SecretToken};
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = std::sync::Arc::new(SessionTestSigner::new());
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut jobs = vec![];
    for _ in 0..2 {
        let (lifecycle, signer, clock, barrier) = (
            lifecycle.clone(),
            signer.clone(),
            clock.clone(),
            barrier.clone(),
        );
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            register_local_session(
                &lifecycle,
                "same-user",
                signer.as_ref(),
                &security_budget(&clock),
            )
            .await
        }));
    }
    let mut sessions = vec![];
    for job in jobs {
        match job.await.unwrap() {
            Ok(session) => sessions.push(session),
            Err(error) => assert_eq!(error, SecurityError::Conflict),
        }
    }
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        session_count(&db, "\"user\" WHERE username='same-user'").await,
        1
    );
    let original = sessions.pop().unwrap();
    let refresh = std::sync::Arc::new(
        SecretToken::new(original.tokens.refresh_token.expose_secret().into()).unwrap(),
    );
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(16));
    let mut jobs = vec![];
    for _ in 0..16 {
        let (lifecycle, signer, clock, barrier, refresh) = (
            lifecycle.clone(),
            signer.clone(),
            clock.clone(),
            barrier.clone(),
            refresh.clone(),
        );
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            lifecycle
                .rotate_local_refresh(
                    &refresh,
                    &session_config(),
                    signer.as_ref(),
                    &security_budget(&clock),
                )
                .await
        }));
    }
    let mut replacements = vec![];
    for job in jobs {
        match job.await.unwrap() {
            Ok(set) => replacements.push(set),
            Err(error) => assert!(matches!(
                error,
                SecurityError::CredentialRevoked | SecurityError::Conflict
            )),
        }
    }
    assert_eq!(replacements.len(), 1);
    assert_eq!(
        replacements[0].claims.grant_id,
        original.tokens.claims.grant_id
    );
    assert_eq!(
        replacements[0].claims.session_id,
        original.tokens.claims.session_id
    );
    assert_eq!(
        replacements[0].claims.family_id,
        original.tokens.claims.family_id
    );
    assert_eq!(session_count(&db, "security_credential").await, 4);
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        1
    );
}
#[tokio::test]
async fn security_t24_refresh_preserves_bounds_rejects_oidc_and_observes_revoked_roots() {
    use auth::security::{credential::CredentialRef, error::SecurityError};
    let (_container, db, clock, lifecycle) = local_fixture().await;
    let signer = SessionTestSigner::new();
    let original = login_local_session(&lifecycle, &signer, &security_budget(&clock))
        .await
        .unwrap()
        .tokens;
    let bounds = original.claims.bounds.clone();
    execute(&db, "INSERT INTO authority (id,name,authority_uri,method,create_time,update_time) VALUES (9003,'new-authority','/new-authority','POST',NOW(),NOW())").await.unwrap();
    let rotated = lifecycle
        .rotate_local_refresh(
            &original.refresh_token,
            &session_config(),
            &signer,
            &security_budget(&clock),
        )
        .await
        .unwrap();
    assert_eq!(rotated.claims.bounds, bounds);
    assert_eq!(
        lifecycle
            .check_registered(
                &CredentialRef::new(&original.claims.iss, &original.claims.jti).unwrap(),
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        rotate_session(
            &lifecycle,
            &rotated.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::InvalidCredential)
    );
    execute(
        &db,
        "UPDATE security_session_client SET audience='cobotusr',application='cobotusr'",
    )
    .await
    .unwrap();
    let code = session_code(&db, "session-test").await;
    let oidc = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    assert_eq!(
        lifecycle
            .rotate_local_refresh(
                &oidc.refresh_token,
                &session_config(),
                &signer,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    lifecycle
        .check_registered(
            &CredentialRef::new(&oidc.claims.iss, &oidc.claims.jti).unwrap(),
            &security_budget(&clock),
        )
        .await
        .unwrap();
    execute(
        &db,
        &format!(
            "UPDATE security_session SET revoked_at=created_at WHERE id='{}'",
            rotated.claims.session_id.as_ref().unwrap()
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        lifecycle
            .rotate_local_refresh(
                &rotated.refresh_token,
                &session_config(),
                &signer,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
}
#[tokio::test]
async fn security_t24_inherited_budget_and_invalid_password_fail_without_partial_user() {
    use auth::security::error::SecurityError;
    use cobotusr::service::security::lifecycle::LocalPassword;
    let (_container, db, clock, lifecycle) = local_fixture().await;
    assert_eq!(
        LocalPassword::new("".into()).err(),
        Some(SecurityError::InvalidCredential)
    );
    assert_eq!(
        LocalPassword::new("x".repeat(73)).err(),
        Some(SecurityError::InvalidCredential)
    );
    assert!(LocalPassword::new("x".repeat(72)).is_ok());
    assert_eq!(
        format!("{:?}", LocalPassword::new(LOCAL_PASSWORD.into()).unwrap()),
        "LocalPassword([REDACTED])"
    );
    let signer = SessionTestSigner {
        delay: std::time::Duration::from_millis(600),
        fail: std::sync::atomic::AtomicBool::new(false),
    };
    let budget = security_budget(&clock)
        .child(std::time::Duration::from_millis(100))
        .unwrap();
    let started = std::time::Instant::now();
    assert_eq!(
        login_local_session(&lifecycle, &signer, &budget)
            .await
            .err(),
        Some(SecurityError::DeadlineExceeded)
    );
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
    let budget = security_budget(&clock)
        .child(std::time::Duration::from_millis(1))
        .unwrap();
    assert_eq!(
        register_local_session(&lifecycle, "too-slow", &SessionTestSigner::new(), &budget)
            .await
            .err(),
        Some(SecurityError::DeadlineExceeded)
    );
    assert_eq!(session_count(&db, "\"user\"").await, 1);
    assert_eq!(session_count(&db, "security_credential").await, 0);
}
const SESSION_REDIRECT: &str = "http://localhost/callback";
fn session_config() -> cobotusr::service::security::lifecycle::SessionTokenConfig {
    cobotusr::service::security::lifecycle::SessionTokenConfig {
        issuer: "https://idp.test".into(),
        access_ttl_seconds: 300,
        refresh_ttl_seconds: 3600,
    }
}
async fn session_code(
    db: &DatabaseConnection,
    client: &str,
) -> auth::security::identity::SecretToken {
    use cobotusr::service::oidc::{CreateAuthCodeParams, OidcService};
    auth::security::identity::SecretToken::new(
        OidcService::create_authorization_code(
            db,
            CreateAuthCodeParams {
                user_id: 1,
                client_id: client.into(),
                redirect_uri: Some(SESSION_REDIRECT.into()),
                code_challenge: Some("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".into()),
                code_challenge_method: "S256".into(),
                scope: Some("openid".into()),
                state: None,
                nonce: Some("nonce".into()),
            },
        )
        .await
        .unwrap(),
    )
    .unwrap()
}
async fn session_fixture() -> (
    testcontainers::ContainerAsync<testcontainers::GenericImage>,
    DatabaseConnection,
    std::sync::Arc<crate::cobotusr::common::SecurityTestClock>,
    std::sync::Arc<cobotusr::service::security::lifecycle::CredentialLifecycle>,
) {
    use cobotusr::{
        migration::{Migrator, MigratorTrait},
        service::{
            oidc::OidcService,
            security::{lifecycle::CredentialLifecycle, repository::PgSecurityRepository},
        },
    };
    let (container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    crate::cobotusr::common::seed_resource_application(&db).await;
    execute(&db, "INSERT INTO \"user\" (id,username,role_id,create_time,update_time) VALUES (1,'session-user',1,NOW(),NOW())").await.unwrap();
    OidcService::register_client(
        &db,
        1,
        "session-test".into(),
        "session".into(),
        vec![SESSION_REDIRECT.into()],
        vec!["openid".into()],
        vec!["authorization_code".into(), "refresh_token".into()],
        true,
    )
    .await
    .unwrap();
    execute(&db, "INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ('https://idp.test','session-test','resource-app','resource-app',TRUE)").await.unwrap();
    let clock = std::sync::Arc::new(crate::cobotusr::common::SecurityTestClock::new(
        chrono::Utc::now().timestamp() as u64 * 1000,
    ));
    let repo = std::sync::Arc::new(PgSecurityRepository::new(db.clone(), clock.clone()));
    let lifecycle = std::sync::Arc::new(CredentialLifecycle::new(repo, clock.clone()));
    (container, db, clock, lifecycle)
}
async fn redeem_session(
    lifecycle: &cobotusr::service::security::lifecycle::CredentialLifecycle,
    code: &auth::security::identity::SecretToken,
    signer: &dyn cobotusr::service::security::lifecycle::SessionTokenSigner,
    budget: &auth::security::budget::SecurityBudget,
) -> Result<
    cobotusr::service::security::lifecycle::SessionTokenSet,
    auth::security::error::SecurityError,
> {
    use cobotusr::service::security::lifecycle::CodeExchangeRequest;
    let verifier = auth::security::identity::SecretToken::new(SESSION_VERIFIER.into()).unwrap();
    lifecycle
        .consume_authorization_code(
            CodeExchangeRequest {
                code,
                client_id: "session-test",
                client_secret: None,
                redirect_uri: Some(SESSION_REDIRECT),
                code_verifier: Some(&verifier),
            },
            &session_config(),
            signer,
            budget,
        )
        .await
}
async fn session_count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        format!("SELECT count(*) AS count FROM {table}"),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "count")
    .unwrap()
}
#[tokio::test]
async fn security_t21_wrong_client_redirect_or_pkce_never_burns_code() {
    use auth::security::{error::SecurityError, identity::SecretToken};
    use cobotusr::service::security::lifecycle::CodeExchangeRequest;
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let code = session_code(&db, "session-test").await;
    let signer = SessionTestSigner::new();
    let good = SecretToken::new(SESSION_VERIFIER.into()).unwrap();
    let bad = SecretToken::new("wrong-verifier".into()).unwrap();
    for (client, redirect, verifier) in [
        ("unknown", Some(SESSION_REDIRECT), Some(&good)),
        ("session-test", Some("http://localhost/other"), Some(&good)),
        ("session-test", None, Some(&good)),
        ("session-test", Some(SESSION_REDIRECT), Some(&bad)),
        ("session-test", Some(SESSION_REDIRECT), None),
    ] {
        assert_eq!(
            lifecycle
                .consume_authorization_code(
                    CodeExchangeRequest {
                        code: &code,
                        client_id: client,
                        client_secret: None,
                        redirect_uri: redirect,
                        code_verifier: verifier
                    },
                    &session_config(),
                    &signer,
                    &security_budget(&clock)
                )
                .await
                .err(),
            Some(SecurityError::InvalidCredential)
        );
        assert_eq!(session_count(&db, "security_credential").await, 0);
        assert_eq!(
            session_count(&db, "oauth_authorization_code WHERE used").await,
            0
        );
    }
    let issued = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    assert_eq!(issued.claims.sub, "1");
    assert_eq!(
        issued.claims.token_use,
        auth::security::identity::TokenUse::UserSession
    );
    assert_eq!(session_count(&db, "security_credential").await, 2);
    assert_eq!(session_count(&db, "security_grant").await, 1);
    assert_eq!(
        session_count(&db, "oauth_authorization_code WHERE used").await,
        1
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t21_concurrent_code_exchange_commits_one_registered_pair() {
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let code = std::sync::Arc::new(session_code(&db, "session-test").await);
    let signer = std::sync::Arc::new(SessionTestSigner::new());
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(16));
    let mut jobs = Vec::new();
    for _ in 0..16 {
        let (code, lifecycle, signer, clock, barrier) = (
            code.clone(),
            lifecycle.clone(),
            signer.clone(),
            clock.clone(),
            barrier.clone(),
        );
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            redeem_session(&lifecycle, &code, signer.as_ref(), &security_budget(&clock)).await
        }));
    }
    let mut successes = Vec::new();
    for job in jobs {
        if let Ok(set) = job.await.unwrap() {
            successes.push(set);
        }
    }
    assert_eq!(successes.len(), 1);
    assert_eq!(session_count(&db, "security_credential").await, 2);
    assert_eq!(session_count(&db, "security_session").await, 1);
    let reference = auth::security::credential::CredentialRef::new(
        "https://idp.test",
        &successes[0].claims.jti,
    )
    .unwrap();
    lifecycle
        .check_registered(&reference, &security_budget(&clock))
        .await
        .unwrap();
}
#[tokio::test]
async fn security_t21_signing_and_deferred_commit_failure_roll_back_code_and_roots() {
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let code = session_code(&db, "session-test").await;
    let signer = SessionTestSigner::new();
    signer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
            .await
            .is_err()
    );
    signer
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);
    execute(&db, "CREATE FUNCTION fail_session_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test commit rejected'; END $$;").await.unwrap();
    execute(&db, "CREATE CONSTRAINT TRIGGER reject_session_commit AFTER INSERT ON security_credential DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_session_commit()").await.unwrap();
    assert!(
        redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
            .await
            .is_err()
    );
    for table in [
        "security_credential",
        "security_grant",
        "security_session",
        "security_refresh_family",
        "oauth_authorization_code WHERE used",
    ] {
        assert_eq!(session_count(&db, table).await, 0);
    }
    execute(
        &db,
        "DROP TRIGGER reject_session_commit ON security_credential",
    )
    .await
    .unwrap();
    redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
}

async fn rotate_session(
    lifecycle: &cobotusr::service::security::lifecycle::CredentialLifecycle,
    refresh: &auth::security::identity::SecretToken,
    signer: &dyn cobotusr::service::security::lifecycle::SessionTokenSigner,
    budget: &auth::security::budget::SecurityBudget,
) -> Result<
    cobotusr::service::security::lifecycle::SessionTokenSet,
    auth::security::error::SecurityError,
> {
    use cobotusr::service::security::lifecycle::RefreshRequest;
    lifecycle
        .rotate_refresh(
            RefreshRequest {
                refresh_token: refresh,
                client_id: "session-test",
                client_secret: None,
            },
            &session_config(),
            signer,
            budget,
        )
        .await
}
#[tokio::test]
async fn security_t22_rotation_keeps_roots_bounds_and_explicit_access_pair() {
    use cobotusr::entity::security_credential as credential;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let (_container, db, clock, lifecycle) = session_fixture().await;
    execute(&db, r#"INSERT INTO permission (id,application_id,name,action,resource_type,resource_pattern,create_time,update_time) VALUES (777,(SELECT id FROM application WHERE app_key='resource-app'),'device:read','read','device','{"application":"resource-app","scope":{"scope":"gateway","gateway_id":"g1","kind":"device"}}',NOW(),NOW())"#).await.unwrap();
    execute(
        &db,
        "INSERT INTO role_permission (role_id,permission_id,create_time) VALUES (1,777,NOW())",
    )
    .await
    .unwrap();
    let code = session_code(&db, "session-test").await;
    let signer = SessionTestSigner::new();
    let first = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    assert_eq!(first.claims.aud, "resource-app");
    assert_eq!(first.claims.application, "resource-app");
    assert!(!first.claims.bounds.grants().is_empty());
    execute(&db, r#"UPDATE permission SET resource_pattern='{"application":"resource-app","scope":{"scope":"gateway","gateway_id":"g2","kind":"device"}}' WHERE id=777"#).await.unwrap();
    let second = rotate_session(
        &lifecycle,
        &first.refresh_token,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
    assert_eq!(first.claims.grant_id, second.claims.grant_id);
    assert_eq!(first.claims.session_id, second.claims.session_id);
    assert_eq!(first.claims.family_id, second.claims.family_id);
    assert_eq!(first.claims.bounds, second.claims.bounds);
    assert_ne!(first.claims.jti, second.claims.jti);
    let old_access =
        auth::security::credential::CredentialRef::new("https://idp.test", &first.claims.jti)
            .unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&old_access, &security_budget(&clock))
            .await
            .err(),
        Some(auth::security::error::SecurityError::CredentialRevoked)
    );
    let current =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), second.claims.jti.clone()))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
    assert!(current.revocation_parent_jti.is_none());
    assert!(current.minted_by.is_some());
    let consumed = credential::Entity::find()
        .filter(credential::Column::ConsumedAt.is_not_null())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        consumed.paired_access_jti.as_deref(),
        Some(first.claims.jti.as_str())
    );
    assert!(consumed.secret_digest.is_some());
    assert!(consumed.replacement_jti.is_some());
    lifecycle
        .check_registered(
            &auth::security::credential::CredentialRef::new("https://idp.test", &second.claims.jti)
                .unwrap(),
            &security_budget(&clock),
        )
        .await
        .unwrap();
    // Expired access does not prevent a still-live refresh from rotating.
    clock.set_wall_time((second.claims.exp + 2) * 1000);
    let third = rotate_session(
        &lifecycle,
        &second.refresh_token,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
    assert_eq!(second.claims.bounds, third.claims.bounds);
    assert_eq!(session_count(&db, "security_credential").await, 6);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t22_concurrent_refresh_creates_only_one_replacement_pair() {
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let signer = std::sync::Arc::new(SessionTestSigner::new());
    let code = session_code(&db, "session-test").await;
    let first = std::sync::Arc::new(
        redeem_session(&lifecycle, &code, signer.as_ref(), &security_budget(&clock))
            .await
            .unwrap(),
    );
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(16));
    let mut jobs = Vec::new();
    for _ in 0..16 {
        let (lifecycle, first, signer, clock, barrier) = (
            lifecycle.clone(),
            first.clone(),
            signer.clone(),
            clock.clone(),
            barrier.clone(),
        );
        jobs.push(tokio::spawn(async move {
            barrier.wait().await;
            rotate_session(
                &lifecycle,
                &first.refresh_token,
                signer.as_ref(),
                &security_budget(&clock),
            )
            .await
        }));
    }
    let mut successes = 0;
    for job in jobs {
        if job.await.unwrap().is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1);
    assert_eq!(session_count(&db, "security_credential").await, 4);
    assert_eq!(
        session_count(&db, "security_credential WHERE consumed_at IS NOT NULL").await,
        1
    );
    assert_eq!(session_count(&db, "security_refresh_family").await, 1);
}
#[tokio::test]
async fn security_t22_failed_signing_or_commit_leaves_old_pair_usable() {
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let signer = SessionTestSigner::new();
    let code = session_code(&db, "session-test").await;
    let first = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    signer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .is_err()
    );
    signer
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);
    execute(&db, "CREATE FUNCTION fail_refresh_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test commit rejected'; END $$;").await.unwrap();
    execute(&db, "CREATE CONSTRAINT TRIGGER reject_refresh_commit AFTER INSERT ON security_credential DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_refresh_commit()").await.unwrap();
    assert!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .is_err()
    );
    assert_eq!(session_count(&db, "security_credential").await, 2);
    assert_eq!(
        session_count(
            &db,
            "security_credential WHERE consumed_at IS NOT NULL OR revoked_at IS NOT NULL"
        )
        .await,
        0
    );
    execute(
        &db,
        "DROP TRIGGER reject_refresh_commit ON security_credential",
    )
    .await
    .unwrap();
    rotate_session(
        &lifecycle,
        &first.refresh_token,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
}
#[tokio::test]
async fn security_t22_wrong_client_revoked_roots_and_inherited_lock_budget_fail_closed() {
    use auth::security::{error::SecurityError, identity::SecretToken};
    use cobotusr::service::security::lifecycle::RefreshRequest;
    use sea_orm::TransactionTrait;
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let signer = SessionTestSigner::new();
    let code = session_code(&db, "session-test").await;
    let first = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    assert_eq!(
        lifecycle
            .rotate_refresh(
                RefreshRequest {
                    refresh_token: &first.refresh_token,
                    client_id: "unknown",
                    client_secret: None
                },
                &session_config(),
                &signer,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    let unknown = SecretToken::new("unknown".into()).unwrap();
    assert_eq!(
        rotate_session(&lifecycle, &unknown, &signer, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    let lock = db.begin().await.unwrap();
    lock.execute_raw(Statement::from_string(
        DbBackend::Postgres,
        "LOCK TABLE security_credential IN ACCESS EXCLUSIVE MODE".to_owned(),
    ))
    .await
    .unwrap();
    let short = security_budget(&clock)
        .child(std::time::Duration::from_millis(100))
        .unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(
        rotate_session(&lifecycle, &first.refresh_token, &signer, &short).await,
        Err(SecurityError::DeadlineExceeded | SecurityError::DependencyUnavailable)
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    lock.rollback().await.unwrap();
    execute(
        &db,
        &format!(
            "UPDATE security_session SET revoked_at={} WHERE id='{}'",
            first.claims.iat * 1000,
            first.claims.session_id.as_ref().unwrap()
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        session_count(&db, "security_credential WHERE consumed_at IS NOT NULL").await,
        0
    );
}

#[tokio::test]
async fn security_t23_consumed_refresh_revokes_only_its_family_and_descendants() {
    use auth::security::{credential::CredentialRef, error::SecurityError};
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let signer = SessionTestSigner::new();
    let first_code = session_code(&db, "session-test").await;
    let unrelated_code = session_code(&db, "session-test").await;
    let first = redeem_session(&lifecycle, &first_code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    let unrelated = redeem_session(
        &lifecycle,
        &unrelated_code,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
    let second = rotate_session(
        &lifecycle,
        &first.refresh_token,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
    let third = rotate_session(
        &lifecycle,
        &second.refresh_token,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
    use cobotusr::entity::security_credential as credential;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    let mut child: credential::ActiveModel =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), third.claims.jti.clone()))
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into();
    child.jti = Set("delegated-child".into());
    child.token_use = Set("gateway_operation".into());
    child.oidc_client_id = Set(None);
    child.audience = Set("legacy-gateway:g1".into());
    child.binding = Set(serde_json::json!({"kind":"gateway","gateway_id":"g1"}));
    child.revocation_parent_jti = Set(Some(third.claims.jti.clone()));
    child.insert(&db).await.unwrap();
    // Historical descendants can still be revoked, but cannot authenticate.
    assert_eq!(
        lifecycle
            .check_registered(
                &CredentialRef::new("https://idp.test", "delegated-child").unwrap(),
                &security_budget(&clock),
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    execute(&db, &format!("UPDATE security_credential SET minted_by='{{\"issuer\":\"https://idp.test\",\"jti\":\"{}\"}}' WHERE jti='{}'", third.claims.jti, unrelated.claims.jti)).await.unwrap();
    assert_eq!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        lifecycle
            .check_registered(
                &CredentialRef::new("https://idp.test", &third.claims.jti).unwrap(),
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        rotate_session(
            &lifecycle,
            &third.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        lifecycle
            .check_registered(
                &CredentialRef::new("https://idp.test", "delegated-child").unwrap(),
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        session_count(
            &db,
            "security_credential WHERE jti='delegated-child' AND revoked_at IS NOT NULL"
        )
        .await,
        1
    );
    lifecycle
        .check_registered(
            &CredentialRef::new("https://idp.test", &unrelated.claims.jti).unwrap(),
            &security_budget(&clock),
        )
        .await
        .unwrap();
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        1
    );
    assert_eq!(
        session_count(&db, "security_grant WHERE revoked_at IS NOT NULL").await,
        0
    );
    assert_eq!(
        session_count(&db, "security_session WHERE revoked_at IS NOT NULL").await,
        0
    );
    assert_eq!(
        session_count(&db, "security_credential WHERE consumed_at IS NOT NULL").await,
        2
    );
}
#[tokio::test]
async fn security_t23_wrong_client_and_unknown_digest_cannot_revoke_consumed_family() {
    use auth::security::{credential::CredentialRef, error::SecurityError, identity::SecretToken};
    use cobotusr::service::{oidc::OidcService, security::lifecycle::RefreshRequest};
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let signer = SessionTestSigner::new();
    let code = session_code(&db, "session-test").await;
    let first = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    clock.set_wall_time((first.claims.iat + session_config().refresh_ttl_seconds - 100) * 1000);
    let second = rotate_session(
        &lifecycle,
        &first.refresh_token,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
    OidcService::register_client(
        &db,
        1,
        "other-session".into(),
        "other".into(),
        vec![SESSION_REDIRECT.into()],
        vec!["openid".into()],
        vec!["refresh_token".into()],
        true,
    )
    .await
    .unwrap();
    execute(&db, "INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ('https://idp.test','other-session','resource-app','resource-app',TRUE)").await.unwrap();
    assert_eq!(
        lifecycle
            .rotate_refresh(
                RefreshRequest {
                    refresh_token: &first.refresh_token,
                    client_id: "other-session",
                    client_secret: None
                },
                &session_config(),
                &signer,
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    assert_eq!(
        rotate_session(
            &lifecycle,
            &SecretToken::new("unknown-refresh-marker".into()).unwrap(),
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::InvalidCredential)
    );
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        0
    );
    lifecycle
        .check_registered(
            &CredentialRef::new("https://idp.test", &second.claims.jti).unwrap(),
            &security_budget(&clock),
        )
        .await
        .unwrap();
    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_jsonb(c) AS data FROM security_credential c".to_owned(),
        ))
        .await
        .unwrap();
    for row in rows {
        let value: serde_json::Value = row.try_get("", "data").unwrap();
        assert!(
            !value
                .to_string()
                .contains(first.refresh_token.expose_secret())
        );
        assert!(
            !value
                .to_string()
                .contains(second.refresh_token.expose_secret())
        );
    }
    // Replaying an expired consumed ancestor still protects its live descendants.
    clock.set_wall_time((first.claims.iat + session_config().refresh_ttl_seconds + 10) * 1000);
    assert_eq!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert!(
        second.claims.exp * 1000
            > (first.claims.iat + session_config().refresh_ttl_seconds + 10) * 1000
    );
    assert_eq!(
        rotate_session(
            &lifecycle,
            &second.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        1
    );
}
#[tokio::test]
async fn security_t23_replay_revocation_commit_failure_is_not_reported_as_durable() {
    use auth::security::{credential::CredentialRef, error::SecurityError};
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let signer = SessionTestSigner::new();
    let code = session_code(&db, "session-test").await;
    let first = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    let second = rotate_session(
        &lifecycle,
        &first.refresh_token,
        &signer,
        &security_budget(&clock),
    )
    .await
    .unwrap();
    execute(&db, "CREATE FUNCTION fail_replay_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test commit rejected'; END $$;").await.unwrap();
    execute(&db, "CREATE CONSTRAINT TRIGGER reject_replay_commit AFTER UPDATE ON security_refresh_family DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_replay_commit()").await.unwrap();
    assert_eq!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        0
    );
    lifecycle
        .check_registered(
            &CredentialRef::new("https://idp.test", &second.claims.jti).unwrap(),
            &security_budget(&clock),
        )
        .await
        .unwrap();
    execute(
        &db,
        "DROP TRIGGER reject_replay_commit ON security_refresh_family",
    )
    .await
    .unwrap();
    assert_eq!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::CredentialRevoked)
    );
    assert_eq!(session_count(&db, "security_credential").await, 4);
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        1
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn security_t23_replay_and_rotation_across_instances_cannot_restore_family() {
    use auth::security::credential::CredentialRef;
    use cobotusr::service::security::{
        lifecycle::CredentialLifecycle, repository::PgSecurityRepository,
    };
    use std::sync::Arc;
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let second_instance = Arc::new(CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    ));
    let signer = Arc::new(SessionTestSigner::new());
    let code = session_code(&db, "session-test").await;
    let first = redeem_session(&lifecycle, &code, signer.as_ref(), &security_budget(&clock))
        .await
        .unwrap();
    let second = rotate_session(
        &lifecycle,
        &first.refresh_token,
        signer.as_ref(),
        &security_budget(&clock),
    )
    .await
    .unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let (b1, b2) = (barrier.clone(), barrier.clone());
    let (s1, s2) = (signer.clone(), signer.clone());
    let (c1, c2) = (clock.clone(), clock.clone());
    let replay = tokio::spawn(async move {
        b1.wait().await;
        rotate_session(
            &lifecycle,
            &first.refresh_token,
            s1.as_ref(),
            &security_budget(&c1),
        )
        .await
    });
    let rotation = tokio::spawn(async move {
        b2.wait().await;
        rotate_session(
            &second_instance,
            &second.refresh_token,
            s2.as_ref(),
            &security_budget(&c2),
        )
        .await
    });
    assert!(replay.await.unwrap().is_err());
    let checker = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    );
    if let Ok(set) = rotation.await.unwrap() {
        assert!(
            checker
                .check_registered(
                    &CredentialRef::new("https://idp.test", &set.claims.jti).unwrap(),
                    &security_budget(&clock)
                )
                .await
                .is_err()
        );
    }
    assert_eq!(
        session_count(&db, "security_refresh_family WHERE revoked_at IS NOT NULL").await,
        1
    );
    assert!(session_count(&db, "security_credential").await <= 6);
}

#[tokio::test]
async fn security_t21_signing_wait_uses_original_budget_and_rolls_back() {
    use auth::security::{budget::SecurityBudget, error::SecurityError};
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let code = session_code(&db, "session-test").await;
    let signer = SessionTestSigner {
        delay: std::time::Duration::from_millis(600),
        ..SessionTestSigner::new()
    };
    let budget = SecurityBudget::new(clock.clone())
        .unwrap()
        .child(std::time::Duration::from_millis(100))
        .unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        redeem_session(&lifecycle, &code, &signer, &budget)
            .await
            .err(),
        Some(SecurityError::DeadlineExceeded)
    );
    assert!(start.elapsed() < std::time::Duration::from_millis(400));
    assert_eq!(session_count(&db, "security_credential").await, 0);
    redeem_session(
        &lifecycle,
        &code,
        &SessionTestSigner::new(),
        &security_budget(&clock),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn security_t21_client_binding_is_required_and_cannot_change_refresh_audience() {
    use auth::security::error::SecurityError;
    let (_container, db, clock, lifecycle) = session_fixture().await;
    let code = session_code(&db, "session-test").await;
    let signer = SessionTestSigner::new();
    execute(&db, "UPDATE security_session_client SET enabled=FALSE")
        .await
        .unwrap();
    assert_eq!(
        redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    assert_eq!(session_count(&db, "security_credential").await, 0);
    execute(&db, "DELETE FROM security_session_client")
        .await
        .unwrap();
    assert_eq!(
        redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    execute(&db, "INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ('https://idp.test','session-test','resource-app','resource-app',TRUE)").await.unwrap();
    let set = redeem_session(&lifecycle, &code, &signer, &security_budget(&clock))
        .await
        .unwrap();
    assert_eq!(set.claims.aud, "resource-app");
    execute(
        &db,
        "UPDATE security_session_client SET audience='cobotusr',application='cobotusr'",
    )
    .await
    .unwrap();
    assert_eq!(
        rotate_session(
            &lifecycle,
            &set.refresh_token,
            &signer,
            &security_budget(&clock)
        )
        .await
        .err(),
        Some(SecurityError::InvalidCredential)
    );
    assert_eq!(
        session_count(&db, "security_credential WHERE consumed_at IS NOT NULL").await,
        0
    );
}
