use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr, Statement};

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
async fn security_t14_machine_device_and_gateway_key_bindings_are_constrained() {
    use cobotusr::migration::{Migrator, MigratorTrait};
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
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
    let device = "INSERT INTO security_device_binding (id,issuer,subject,application,gateway_id,device_id,binding_revision)
        VALUES ('00000000-0000-4000-8000-000000000003','https://idp.test','device-principal','app','gateway-a','device-a','revision1')";
    execute(&db, device).await.unwrap();
    // One authenticated device principal cannot acquire another ownership row,
    // and a second principal cannot seize an already registered physical target.
    assert!(
        execute(
            &db,
            &device
                .replace(
                    "00000000-0000-4000-8000-000000000003",
                    "00000000-0000-4000-8000-000000000004"
                )
                .replace("gateway-a", "gateway-b")
        )
        .await
        .is_err()
    );
    assert!(
        execute(
            &db,
            &device
                .replace(
                    "00000000-0000-4000-8000-000000000003",
                    "00000000-0000-4000-8000-000000000004"
                )
                .replace("device-principal", "other-principal")
        )
        .await
        .is_err()
    );
    let key = "INSERT INTO security_gateway_signing_key (id,issuer,application,gateway_id,kid,public_key,public_key_fingerprint,valid_from,valid_until)
        VALUES ('00000000-0000-4000-8000-000000000005','https://idp.test','app','gateway-a','key-a','test-public-key',decode(repeat('ab',32),'hex'),1000,9000)";
    execute(&db, key).await.unwrap();
    assert!(
        execute(
            &db,
            &key.replace(
                "00000000-0000-4000-8000-000000000005",
                "00000000-0000-4000-8000-000000000006"
            )
            .replace("gateway-a", "gateway-b")
        )
        .await
        .is_err()
    );
    assert!(
        execute(
            &db,
            &key.replace(
                "00000000-0000-4000-8000-000000000005",
                "00000000-0000-4000-8000-000000000006"
            )
            .replace("key-a", "key-b")
            .replace("gateway-a", "gateway-b")
        )
        .await
        .is_err()
    );
    for table in [
        "security_binding",
        "security_device_binding",
        "security_gateway_signing_key",
    ] {
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT enabled FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(!row.try_get::<bool>("", "enabled").unwrap());
        execute(&db, &format!("UPDATE {table} SET enabled=true"))
            .await
            .unwrap();
        execute(&db, &format!("UPDATE {table} SET enabled=false"))
            .await
            .unwrap();
        let row = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT enabled FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(!row.try_get::<bool>("", "enabled").unwrap());
    }
    use cobotusr::entity::security_binding::{self, device as device_binding, gateway_key};
    use sea_orm::EntityTrait;
    let binding = security_binding::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(!binding.enabled);
    assert_eq!(binding.token_use, "permission_sync");
    assert_eq!(binding.allowed_operations, serde_json::json!([]));
    let device = device_binding::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(!device.enabled);
    assert_eq!(
        (device.gateway_id.as_str(), device.device_id.as_str()),
        ("gateway-a", "device-a")
    );
    let key = gateway_key::Entity::find().one(&db).await.unwrap().unwrap();
    assert!(!key.enabled);
    assert_eq!(key.public_key_fingerprint, vec![0xab; 32]);
    assert_eq!((key.valid_from, key.valid_until), (1000, 9000));
}

async fn insert_permit_parent(db: &DatabaseConnection) {
    execute(db, "INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at)
        VALUES ('00000000-0000-4000-8000-000000000001','https://idp.test','machine','service','app','service',1000)").await.unwrap();
    execute(db, "INSERT INTO security_credential (issuer,jti,principal_kind,subject,credential_kind,token_use,audience,application,bounds,binding,grant_id,signing_key_id,issued_at,expires_at)
        VALUES ('https://idp.test','parent','machine','service','access','security_service','aud','app','{\"grants\":[]}','{\"kind\":\"none\"}','00000000-0000-4000-8000-000000000001','key',1000,9000)").await.unwrap();
}

const PERMIT_INSERT: &str = "INSERT INTO security_permit (issuer,jti,parent_jti,grant_id,application,request_id,task_id,actor,presenter,operation,resource,request_digest,signing_key_id,signed_permit,checked_at,valid_until,issued_at,expires_at)
    VALUES ('https://idp.test','permit-jti','parent','00000000-0000-4000-8000-000000000001','app',
    '00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000003',
    '{\"issuer\":\"https://idp.test\",\"kind\":\"machine\",\"subject\":\"service\"}',
    '{\"issuer\":\"https://idp.test\",\"kind\":\"machine\",\"subject\":\"cloud\"}',
    'task:execute','{\"application\":\"app\",\"target\":{\"kind\":\"device\",\"gateway_id\":\"g\",\"device_id\":\"d\"}}',
    decode(repeat('ab',32),'hex'),'key','test-signed-permit',1000,6000,1000,6000)";

#[tokio::test]
async fn security_t15_permit_task_request_and_parent_bindings_are_unique() {
    use cobotusr::migration::{Migrator, MigratorTrait};
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    insert_permit_parent(&db).await;
    assert!(
        execute(&db, &PERMIT_INSERT.replace("'parent'", "'missing'"))
            .await
            .is_err()
    );
    assert!(
        execute(
            &db,
            &PERMIT_INSERT.replace("1000,6000,1000,6000", "1000,6000,1000,7000")
        )
        .await
        .is_err()
    );
    execute(&db, PERMIT_INSERT).await.unwrap();
    // Neither a new request nor a new JWT can renew an already authorized task.
    assert!(
        execute(
            &db,
            &PERMIT_INSERT.replace("permit-jti", "new-permit").replace(
                "00000000-0000-4000-8000-000000000002",
                "00000000-0000-4000-8000-000000000004"
            )
        )
        .await
        .is_err()
    );
    assert!(
        execute(
            &db,
            &PERMIT_INSERT.replace("permit-jti", "new-permit").replace(
                "00000000-0000-4000-8000-000000000003",
                "00000000-0000-4000-8000-000000000004"
            )
        )
        .await
        .is_err()
    );
    let non_task = PERMIT_INSERT
        .replace("permit-jti", "read-permit")
        .replace("task:execute", "device:read")
        .replace("'00000000-0000-4000-8000-000000000003'", "NULL")
        .replace(
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000005",
        );
    execute(&db, &non_task).await.unwrap();
    assert!(
        execute(&db, &non_task.replace("read-permit", "other-read-permit"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn security_t15_audit_and_security_fact_commit_or_rollback_together() {
    use cobotusr::migration::{Migrator, MigratorTrait};
    use sea_orm::TransactionTrait;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    insert_permit_parent(&db).await;
    let audit = "INSERT INTO security_audit (id,event_type,actor,operation,resource,outcome,reason_code,occurred_at)
        VALUES ('00000000-0000-4000-8000-000000000006','permit_issued',
        '{\"issuer\":\"https://idp.test\",\"kind\":\"machine\",\"subject\":\"service\"}',
        'task:execute','{\"application\":\"app\"}','success','issued',1000)";
    for commit in [false, true] {
        let txn = db.begin().await.unwrap();
        for sql in [PERMIT_INSERT, audit] {
            txn.execute_raw(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
                .await
                .unwrap();
        }
        if commit {
            txn.commit().await.unwrap();
        } else {
            txn.rollback().await.unwrap();
        }
        for table in ["security_permit", "security_audit"] {
            let row = db
                .query_one_raw(Statement::from_string(
                    DbBackend::Postgres,
                    format!("SELECT COUNT(*) AS count FROM {table}"),
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<i64>("", "count").unwrap(), i64::from(commit));
        }
    }
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
    insert_permit_parent(&db).await;
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
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
    assert!(
        txn.permit("app", uuid::Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    txn.rollback().await.unwrap();
    execute(&db, PERMIT_INSERT).await.unwrap();
    let mut txn = repository.begin(&budget).await.unwrap();
    let permit = txn
        .permit(
            "app",
            "00000000-0000-4000-8000-000000000002".parse().unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(!format!("{permit:?}").contains("test-signed-permit"));
    let active: cobotusr::entity::security_permit::ActiveModel = permit.clone().into();
    assert!(!format!("{active:?}").contains("test-signed-permit"));
    assert!(
        cobotusr::entity::security_permit::StoredPermitValue::new(
            "invalid token with whitespace".into()
        )
        .is_err()
    );
    assert!(
        txn.permit("other", permit.request_id)
            .await
            .unwrap()
            .is_none()
    );
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
    insert_permit_parent(&db).await;
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
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
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
    clock: &std::sync::Arc<crate::cobotsrv::common::SecurityTestClock>,
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
    insert_permit_parent(&db).await;
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
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
    insert_permit_parent(&db).await;
    insert_user_access(&db).await;
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
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
async fn security_t18_delegation_checks_parent_and_rejects_redelegation_or_cycles() {
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
    insert_permit_parent(&db).await;
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
    row.audience = Set("cobotlet:g".into());
    row.clone().insert(&db).await.unwrap();
    row.jti = Set("grandchild".into());
    row.revocation_parent_jti = Set(Some("child".into()));
    row.insert(&db).await.unwrap();
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    );
    let child = CredentialRef::new("https://idp.test", "child").unwrap();
    assert!(
        lifecycle
            .check_registered(&child, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
    assert_eq!(
        lifecycle
            .check_registered(
                &CredentialRef::new("https://idp.test", "grandchild").unwrap(),
                &security_budget(&clock)
            )
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    execute(
        &db,
        "UPDATE security_credential SET revoked_at=2000 WHERE jti='parent'",
    )
    .await
    .unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&child, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    execute(
        &db,
        "UPDATE security_credential SET revoked_at=NULL,expires_at=5000 WHERE jti='parent'",
    )
    .await
    .unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&child, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
    execute(
        &db,
        "UPDATE security_credential SET expires_at=4000 WHERE jti='child'",
    )
    .await
    .unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&child, &security_budget(&clock))
            .await
            .unwrap()
            .validity
            .valid_until(),
        4000
    );
    execute(&db,"UPDATE security_credential SET token_use='gateway_operation',revocation_parent_jti='child',binding='{\"kind\":\"gateway\",\"gateway_id\":\"g\"}',audience='cobotlet:g' WHERE jti='parent'").await.unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&child, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
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
        let private=rsa::RsaPrivateKey::new(&mut rand::rngs::OsRng,2048).unwrap();
        let pem=private.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let public=rsa::RsaPublicKey::from(&private);
        let n=URL_SAFE_NO_PAD.encode(private.n().to_bytes_be());let e=URL_SAFE_NO_PAD.encode(private.e().to_bytes_be());
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
async fn security_t17_device_and_gateway_key_registration_remains_scoped_and_current() {
    use auth::security::{
        error::SecurityError,
        identity::{PrincipalKind, PrincipalRef},
    };
    use cobotusr::{
        entity::security_binding::gateway_key,
        migration::{Migrator, MigratorTrait},
        service::security::repository::{
            PgSecurityRepository, SecurityRepository, gateway_key_fingerprint,
        },
    };
    use sea_orm::{ActiveModelTrait, Set};
    use std::sync::Arc;
    let (_container, url) = crate::cobotusr::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
    let repo = PgSecurityRepository::new(db.clone(), clock.clone());
    execute(&db,"INSERT INTO security_device_binding (id,issuer,subject,application,gateway_id,device_id,binding_revision)
        VALUES ('00000000-0000-4000-8000-000000000001','https://idp.test','dev','app','g','d','r1')").await.unwrap();
    let principal = PrincipalRef::new("https://idp.test", PrincipalKind::Device, "dev").unwrap();
    let mut txn = repo.begin(&security_budget(&clock)).await.unwrap();
    assert!(
        txn.device_binding(&principal, "app")
            .await
            .unwrap()
            .is_none()
    );
    txn.rollback().await.unwrap();
    execute(&db, "UPDATE security_device_binding SET enabled=true")
        .await
        .unwrap();
    let mut txn = repo.begin(&security_budget(&clock)).await.unwrap();
    assert_eq!(
        txn.device_binding(&principal, "app")
            .await
            .unwrap()
            .unwrap()
            .device_id,
        "d"
    );
    assert!(
        txn.device_binding(&principal, "other")
            .await
            .unwrap()
            .is_none()
    );
    execute(&db, "UPDATE security_device_binding SET enabled=false")
        .await
        .unwrap();
    // The same transaction keeps one source snapshot, rather than mixing a
    // previously read identity with a later root/binding state.
    assert!(
        txn.device_binding(&principal, "app")
            .await
            .unwrap()
            .is_some()
    );
    txn.commit().await.unwrap();
    let mut txn = repo.begin(&security_budget(&clock)).await.unwrap();
    assert!(
        txn.device_binding(&principal, "app")
            .await
            .unwrap()
            .is_none()
    );
    txn.rollback().await.unwrap();
    assert_eq!(
        gateway_key_fingerprint("invalid-public-key").err(),
        Some(SecurityError::InvalidCredential)
    );
    let public = security_signing_fixture().public_pem.clone();
    gateway_key::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        issuer: Set("https://idp.test".into()),
        application: Set("app".into()),
        gateway_id: Set("g".into()),
        kid: Set("gateway-key".into()),
        public_key: Set(public.clone()),
        public_key_fingerprint: Set(gateway_key_fingerprint(&public).unwrap().to_vec()),
        enabled: Set(true),
        valid_from: Set(1000),
        valid_until: Set(9000),
    }
    .insert(&db)
    .await
    .unwrap();
    let mut txn = repo.begin(&security_budget(&clock)).await.unwrap();
    assert!(
        txn.gateway_key("https://idp.test", "app", "g", "gateway-key")
            .await
            .unwrap()
            .is_some()
    );
    for (issuer, app, gateway, kid) in [
        ("https://other.test", "app", "g", "gateway-key"),
        ("https://idp.test", "other", "g", "gateway-key"),
        ("https://idp.test", "app", "other", "gateway-key"),
        ("https://idp.test", "app", "g", "other-key"),
    ] {
        assert!(
            txn.gateway_key(issuer, app, gateway, kid)
                .await
                .unwrap()
                .is_none()
        );
    }
    txn.rollback().await.unwrap();
    for (change, reset) in [
        (
            "UPDATE security_gateway_signing_key SET valid_from=3000",
            "UPDATE security_gateway_signing_key SET valid_from=1000",
        ),
        (
            "UPDATE security_gateway_signing_key SET valid_until=3000",
            "UPDATE security_gateway_signing_key SET valid_until=9000",
        ),
    ] {
        execute(&db, change).await.unwrap();
        let mut txn = repo.begin(&security_budget(&clock)).await.unwrap();
        assert!(
            txn.gateway_key("https://idp.test", "app", "g", "gateway-key")
                .await
                .unwrap()
                .is_none()
        );
        txn.rollback().await.unwrap();
        execute(&db, reset).await.unwrap();
    }
    execute(&db,"UPDATE security_gateway_signing_key SET public_key_fingerprint=decode(repeat('00',32),'hex')").await.unwrap();
    let mut txn = repo.begin(&security_budget(&clock)).await.unwrap();
    assert_eq!(
        txn.gateway_key("https://idp.test", "app", "g", "gateway-key")
            .await
            .err(),
        Some(SecurityError::DependencyUnavailable)
    );
    txn.rollback().await.unwrap();
    execute(&db, "UPDATE security_gateway_signing_key SET enabled=false")
        .await
        .unwrap();
    let mut txn = repo.begin(&security_budget(&clock)).await.unwrap();
    assert!(
        txn.gateway_key("https://idp.test", "app", "g", "gateway-key")
            .await
            .unwrap()
            .is_none()
    );
    txn.rollback().await.unwrap();
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
    insert_permit_parent(&db).await;
    enable_service_binding(&db).await;
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
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
    insert_permit_parent(&db).await;
    enable_service_binding(&db).await;
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
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

#[tokio::test]
async fn security_t18_device_access_requires_current_exact_device_binding() {
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
    insert_permit_parent(&db).await;
    execute(&db,"INSERT INTO security_grant (id,issuer,principal_kind,subject,application,grant_kind,created_at)
        VALUES ('00000000-0000-4000-8000-000000000015','https://idp.test','device','dev','app','device',1000)").await.unwrap();
    execute(&db,"INSERT INTO security_device_binding (id,issuer,subject,application,gateway_id,device_id,binding_revision)
        VALUES ('00000000-0000-4000-8000-000000000016','https://idp.test','dev','app','g','device-a','r1')").await.unwrap();
    let mut row: credential::ActiveModel =
        credential::Entity::find_by_id(("https://idp.test".to_owned(), "parent".to_owned()))
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into();
    row.jti = Set("device-token".into());
    row.subject = Set("dev".into());
    row.principal_kind = Set("device".into());
    row.token_use = Set("device_access".into());
    row.audience = Set("cobotlet:g".into());
    row.binding = Set(serde_json::json!({"kind":"device","gateway_id":"g","device_id":"device-a"}));
    row.grant_id = Set("00000000-0000-4000-8000-000000000015".parse().unwrap());
    row.insert(&db).await.unwrap();
    let clock = Arc::new(crate::cobotsrv::common::SecurityTestClock::new(2000));
    let lifecycle = CredentialLifecycle::new(
        Arc::new(PgSecurityRepository::new(db.clone(), clock.clone())),
        clock.clone(),
    );
    let reference = CredentialRef::new("https://idp.test", "device-token").unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&reference, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::CredentialRevoked)
    );
    execute(&db, "UPDATE security_device_binding SET enabled=true")
        .await
        .unwrap();
    assert!(
        lifecycle
            .check_registered(&reference, &security_budget(&clock))
            .await
            .unwrap()
            .active
    );
    execute(
        &db,
        "UPDATE security_device_binding SET device_id='device-b'",
    )
    .await
    .unwrap();
    assert_eq!(
        lifecycle
            .check_registered(&reference, &security_budget(&clock))
            .await
            .err(),
        Some(SecurityError::InvalidCredential)
    );
}
