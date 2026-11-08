use cobotusr::migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

#[tokio::test]
async fn auth_center_fresh_database_has_only_authentication_schema() {
    let (_container, url) = super::super::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    for table in [
        "conversation",
        "conversation_member",
        "message",
        "group",
        "user_conversation",
        "security_device_binding",
        "security_gateway_signing_key",
        "security_permit",
    ] {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT to_regclass($1)::text AS name",
                [table.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<Option<String>>("", "name").unwrap(),
            None,
            "legacy table {table}"
        );
    }
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS count FROM application WHERE app_key <> 'cobotusr'".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
    Migrator::up(&db, None).await.unwrap();
}

#[tokio::test]
async fn auth_center_upgrades_legacy_audience_catalog_without_erasing_data() {
    let (_container, url) = super::super::common::get_postgres_container()
        .await
        .unwrap();
    let db = Database::connect(url).await.unwrap();
    let old_count = Migrator::migrations().len() - 1;
    Migrator::up(&db, Some(old_count as u32)).await.unwrap();
    db.execute_unprepared("INSERT INTO oauth_client (client_id,user_id,name,redirect_uris,allowed_scopes,client_secret_hash,public_client,grant_types,create_time,update_time) SELECT cid,1,cid,'http://localhost/callback','openid','',TRUE,'authorization_code',NOW(),NOW() FROM (VALUES ('legacy-client'),('new-client'),('invalid')) AS clients(cid);").await.unwrap();
    db.execute_unprepared("ALTER TABLE security_session_client DROP CONSTRAINT security_session_client_audience_check; ALTER TABLE security_session_client ADD CONSTRAINT security_session_client_audience_check CHECK (audience IN ('cobotusr','legacy-app')); CREATE TABLE conversation (id INTEGER PRIMARY KEY); INSERT INTO conversation VALUES (42); INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ('https://idp.test','legacy-client','legacy-app','legacy-app',TRUE);").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared("INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ('https://idp.test','new-client','registered-app','registered-app',TRUE);").await.unwrap();
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT id FROM conversation".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i32>("", "id").unwrap(), 42);
    assert!(db.execute_unprepared("INSERT INTO security_session_client (issuer,client_id,audience,application,enabled) VALUES ('https://idp.test','invalid','','',TRUE)").await.is_err());
}

#[test]
fn auth_center_example_config_has_no_implicit_business_clients() {
    let config: cobotusr::config::Config =
        toml::from_str(include_str!("../../../conf/cobotusr.example.toml")).unwrap();
    assert!(config.oidc.bootstrap_clients.is_empty());
    assert!(config.validate_browser_cookie_policy().is_ok());
}
