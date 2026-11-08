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
