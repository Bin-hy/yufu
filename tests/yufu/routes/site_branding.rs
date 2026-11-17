use super::{
    super::common::IdentityFixture,
    identity_browser::{BrowserCookies, http_login},
    identity_support::*,
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{Value, json};
use yufu::service::identity::repository::IdentityRepository;
const PNG: &[u8] = include_bytes!("../../fixtures/site-branding/logo.png");
async fn settings(f: &IdentityFixture, c: &BrowserCookies) -> Value {
    f.client
        .get(format!("{}/api/v1/admin/site-settings", f.base_url))
        .header("cookie", c.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
fn save(
    f: &IdentityFixture,
    c: &BrowserCookies,
    csrf: &str,
    value: Value,
) -> reqwest::RequestBuilder {
    f.client
        .put(format!("{}/api/v1/admin/site-settings", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", csrf)
        .json(&value)
}
async fn sql(f: &IdentityFixture, sql: &str) {
    f.db.execute_unprepared(sql).await.unwrap();
}
#[tokio::test]
async fn site_branding_upload_publish_inherit_reset_and_conditional_visibility() {
    let f = IdentityFixture::start().await;
    let mut c = BrowserCookies::default();
    let csrf = http_login(&f, &mut c).await;
    let session: Value = f
        .client
        .get(format!("{}/api/v1/account/session", f.base_url))
        .header("cookie", c.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(session["super_administrator"], true);
    let read = f
        .client
        .get(format!("{}/api/v1/admin/site-settings", f.base_url))
        .header("cookie", c.header())
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), 200);
    assert_eq!(read.headers()["cache-control"], "no-store");
    let current: Value = read.json().await.unwrap();
    assert_eq!(current["settings"]["revision"], 1);
    let upload = f
        .client
        .post(format!(
            "{}/api/v1/admin/site-assets?kind=cover",
            f.base_url
        ))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .header("content-type", "application/octet-stream")
        .body(PNG)
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), 201);
    assert_eq!(upload.headers()["cache-control"], "no-store");
    let asset: Value = upload.json().await.unwrap();
    let id = asset["id"].as_str().unwrap();
    let public = format!("{}/api/v1/site/assets/{id}", f.base_url);
    assert_eq!(f.client.get(&public).send().await.unwrap().status(), 404);
    let preview = f
        .client
        .get(format!("{}/api/v1/admin/site-assets/{id}", f.base_url))
        .header("cookie", c.header())
        .send()
        .await
        .unwrap();
    assert_eq!(preview.status(), 200);
    assert_eq!(preview.headers()["cache-control"], "no-store");
    assert!(preview.headers().get("etag").is_none());
    assert_eq!(preview.bytes().await.unwrap().as_ref(), PNG);
    let mut doc = current["settings"]["document"].clone();
    doc["branding"]["site_name"] = json!("企业身份平台");
    doc["branding"]["cover_asset_id"] = json!(id);
    doc["oidc"]["welcome_title"] = json!("应用登录");
    let saved = save(&f, &c, &csrf, json!({"expected_revision":1,"document":doc}))
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    assert_eq!(saved.headers()["cache-control"], "no-store");
    let public_response = f
        .client
        .get(format!("{}/api/v1/site/branding", f.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(public_response.headers()["cache-control"], "no-store");
    let public_config: Value = public_response.json().await.unwrap();
    assert_eq!(public_config["branding"]["site_name"], "企业身份平台");
    assert_eq!(public_config["oidc"]["welcome_title"], "应用登录");
    assert_eq!(
        public_config["oidc"]["cover_url"],
        public_config["login"]["cover_url"]
    );
    for forbidden in [
        "updated_by",
        "updated_at",
        "limits",
        "document",
        "logo_asset_id",
        "favicon_asset_id",
    ] {
        assert!(
            public_config.get(forbidden).is_none(),
            "unexpected public field: {forbidden}"
        );
    }
    for slot in ["branding", "login", "oidc"] {
        for forbidden in [
            "logo_asset_id",
            "favicon_asset_id",
            "cover_asset_id",
            "welcome_title_override",
            "welcome_description_override",
        ] {
            assert!(
                public_config[slot].get(forbidden).is_none(),
                "unexpected public field: {slot}.{forbidden}"
            );
        }
    }
    assert_eq!(
        f.client
            .get(format!(
                "{}/api/v1/site/assets/{}",
                f.base_url,
                uuid::Uuid::new_v4()
            ))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let image = f.client.get(&public).send().await.unwrap();
    assert_eq!(image.status(), 200);
    assert_eq!(image.headers()["content-type"], "image/png");
    assert_eq!(image.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        image.headers()["cache-control"],
        "no-cache, must-revalidate"
    );
    let etag = image.headers()["etag"].to_str().unwrap().to_owned();
    assert_eq!(etag, format!("\"{}\"", asset["sha256"].as_str().unwrap()));
    assert_eq!(
        f.client
            .get(&public)
            .header("if-none-match", &etag)
            .send()
            .await
            .unwrap()
            .status(),
        304
    );
    let reset = f
        .client
        .post(format!("{}/api/v1/admin/site-settings/reset", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .json(&json!({"expected_revision":2}))
        .send()
        .await
        .unwrap();
    assert_eq!(reset.status(), 200);
    assert_eq!(reset.headers()["cache-control"], "no-store");
    assert_eq!(
        f.client
            .get(&public)
            .header("if-none-match", etag)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(scalar(&f,"SELECT count(*) AS value FROM identity_audit_event WHERE object_kind='site_settings' AND result='success'").await,2);
    f.close().await;
}
#[tokio::test]
async fn site_branding_permission_origin_and_upload_limits() {
    let f = IdentityFixture::start().await;
    let mut c = BrowserCookies::default();
    let csrf = http_login(&f, &mut c).await;
    let current = settings(&f, &c).await;
    let input = json!({"expected_revision":1,"document":current["settings"]["document"]});
    assert_eq!(
        f.client
            .get(format!("{}/api/v1/admin/site-settings", f.base_url))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.client
            .put(format!("{}/api/v1/admin/site-settings", f.base_url))
            .header("cookie", c.header())
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let huge = f
        .client
        .post(format!("{}/api/v1/admin/site-assets?kind=logo", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .body(vec![0_u8; 2 * 1024 * 1024 + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(huge.status(), 413);
    let bad = f
        .client
        .post(format!("{}/api/v1/admin/site-assets?kind=logo", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .body("<svg></svg>")
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);
    let wrong_origin = f
        .client
        .post(format!("{}/api/v1/admin/site-assets?kind=logo", f.base_url))
        .header("cookie", c.header())
        .header("origin", "https://evil.example")
        .header("x-csrf-token", &csrf)
        .body(PNG)
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_origin.status(), 403);
    let missing_csrf = f
        .client
        .post(format!("{}/api/v1/admin/site-settings/reset", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .json(&json!({"expected_revision":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), 403);
    let wrong_origin_save = f
        .client
        .put(format!("{}/api/v1/admin/site-settings", f.base_url))
        .header("cookie", c.header())
        .header("origin", "https://evil.example")
        .header("x-csrf-token", &csrf)
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_origin_save.status(), 403);
    let wrong_origin_reset = f
        .client
        .post(format!("{}/api/v1/admin/site-settings/reset", f.base_url))
        .header("cookie", c.header())
        .header("origin", "https://evil.example")
        .header("x-csrf-token", &csrf)
        .json(&json!({"expected_revision":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_origin_reset.status(), 403);
    let upload_without_csrf = f
        .client
        .post(format!("{}/api/v1/admin/site-assets?kind=logo", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .body(PNG)
        .send()
        .await
        .unwrap();
    assert_eq!(upload_without_csrf.status(), 403);
    assert_eq!(
        scalar(
            &f,
            "SELECT count(*) AS value FROM identity_audit_event WHERE object_kind='site_settings'"
        )
        .await,
        0
    );
    let admin = admin_id(&f).await;
    sql(&f,&format!("INSERT INTO user_authority(user_id,authority_id,allow,create_time,update_time) SELECT {admin},id,false,now(),now() FROM authority WHERE method='PUT' AND authority_uri='/admin/site-settings'")).await;
    assert_eq!(
        save(&f, &c, &csrf, input.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    sql(&f,&format!("DELETE FROM user_authority WHERE user_id={admin}; UPDATE \"user\" SET role_id=2,developer_enabled=true WHERE id={admin}; INSERT INTO authority_role(role_id,authority_id,create_time,update_time) SELECT 2,id,now(),now() FROM authority WHERE authority_uri LIKE '/admin/site%'; INSERT INTO user_authority(user_id,authority_id,allow,create_time,update_time) SELECT {admin},id,true,now(),now() FROM authority WHERE method='GET' AND authority_uri='/admin/site-settings'")).await;
    // An explicit operation grant cannot replace the super administrator requirement.
    assert_eq!(
        f.client
            .get(format!("{}/api/v1/admin/site-settings", f.base_url))
            .header("cookie", c.header())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        save(&f, &c, &csrf, input.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let reset = f
        .client
        .post(format!("{}/api/v1/admin/site-settings/reset", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .json(&json!({"expected_revision":1}))
        .send()
        .await
        .unwrap();
    assert_eq!(reset.status(), 403);
    let denied_upload = f
        .client
        .post(format!("{}/api/v1/admin/site-assets?kind=logo", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .body(PNG)
        .send()
        .await
        .unwrap();
    assert_eq!(denied_upload.status(), 403);
    let denied_preview = f
        .client
        .get(format!(
            "{}/api/v1/admin/site-assets/00000000-0000-4000-8000-000000000001",
            f.base_url
        ))
        .header("cookie", c.header())
        .send()
        .await
        .unwrap();
    assert_eq!(denied_preview.status(), 403);
    assert_eq!(
        scalar(&f, "SELECT count(*) AS value FROM site_asset").await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT count(*) AS value FROM site_settings WHERE document->'branding'->>'site_name'='yufu' AND revision=1"
        )
        .await,
        1
    );
    sql(&f,&format!("DELETE FROM user_authority WHERE user_id={admin}; UPDATE \"user\" SET role_id=3,developer_enabled=false WHERE id={admin}")).await;
    assert_eq!(
        f.client
            .get(format!("{}/api/v1/admin/site-settings", f.base_url))
            .header("cookie", c.header())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        save(&f, &c, &csrf, input.clone())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.client
            .post(format!("{}/api/v1/admin/site-settings/reset", f.base_url))
            .header("cookie", c.header())
            .header("origin", &f.base_url)
            .header("x-csrf-token", &csrf)
            .json(&json!({"expected_revision":1}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.client
            .post(format!("{}/api/v1/admin/site-assets?kind=logo", f.base_url))
            .header("cookie", c.header())
            .header("origin", &f.base_url)
            .header("x-csrf-token", &csrf)
            .body(PNG)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        f.client
            .get(format!(
                "{}/api/v1/admin/site-assets/00000000-0000-4000-8000-000000000001",
                f.base_url
            ))
            .header("cookie", c.header())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        scalar(&f, "SELECT count(*) AS value FROM site_asset").await,
        0
    );
    assert_eq!(
        scalar(
            &f,
            "SELECT count(*) AS value FROM identity_audit_event WHERE object_kind='site_settings'"
        )
        .await,
        0
    );
    sql(&f, "DELETE FROM identity_sso_session").await;
    assert_eq!(
        f.client
            .get(format!("{}/api/v1/admin/site-settings", f.base_url))
            .header("cookie", c.header())
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.close().await;
}
#[tokio::test]
async fn site_branding_atomic_audit_failure_and_concurrent_revision() {
    let f = IdentityFixture::start().await;
    let mut c = BrowserCookies::default();
    let csrf = http_login(&f, &mut c).await;
    let current = settings(&f, &c).await;
    let mut doc = current["settings"]["document"].clone();
    doc["branding"]["site_name"] = json!("Changed");
    let input = json!({"expected_revision":1,"document":doc});
    sql(&f,"CREATE FUNCTION reject_branding_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.object_kind='site_settings' THEN RAISE EXCEPTION 'fixture audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_branding_audit BEFORE INSERT ON identity_audit_event FOR EACH ROW EXECUTE FUNCTION reject_branding_audit();").await;
    assert_eq!(
        save(&f, &c, &csrf, input.clone())
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    assert_eq!(settings(&f, &c).await["settings"], current["settings"]);
    sql(&f,"DROP TRIGGER reject_branding_audit ON identity_audit_event; DROP FUNCTION reject_branding_audit();").await;
    let (a, b) = tokio::join!(
        save(&f, &c, &csrf, input.clone()).send(),
        save(&f, &c, &csrf, input).send()
    );
    let mut statuses = vec![a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, vec![200, 409]);
    assert_eq!(scalar(&f,"SELECT count(*) AS value FROM identity_audit_event WHERE object_kind='site_settings' AND result='success'").await,1);
    let audit: Value = f
        .client
        .get(format!(
            "{}/api/v1/admin/audit-events/search?object_kind=site_settings",
            f.base_url
        ))
        .header("cookie", c.header())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        audit["items"][0]["change_summary"]["changed_fields"],
        json!(["branding.site_name"])
    );
    f.close().await;
}
#[tokio::test]
async fn site_branding_cleanup_and_migration_preserve_config() {
    use std::sync::Arc;
    use yufu::migration::MigratorTrait;
    use yufu::{
        domain::identity::{account::IssuedSecret, site_branding::*},
        migration::Migrator,
        service::identity::site_settings::SiteSettingsService,
    };
    let f = IdentityFixture::start().await;
    let (_, secret) = center_secret(&f, admin_id(&f).await).await;
    let browser = IssuedSecret::from_entropy([33; 32]).into_response();
    let evidence = write_evidence(&f, Some(&secret), &browser);
    let svc = SiteSettingsService::new(Arc::new(repository(&f)), f.clock.clone());
    let asset = svc
        .upload(&evidence, SiteAssetKind::Logo, PNG.to_vec(), &budget(&f))
        .await
        .unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert_eq!(tx.cleanup_site_assets(1, 100).await.unwrap(), 0);
    tx.commit().await.unwrap();
    let before = svc.admin_settings(&secret, &budget(&f)).await.unwrap();
    let mut doc = before.document.clone();
    doc.branding.logo_asset_id = Some(asset.id);
    let saved = svc
        .save(
            &evidence,
            SaveSiteSettings {
                expected_revision: before.revision,
                document: doc,
            },
            uuid::Uuid::new_v4(),
            &budget(&f),
        )
        .await
        .unwrap();
    Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(
        svc.admin_settings(&secret, &budget(&f))
            .await
            .unwrap()
            .document,
        saved.document
    );
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert_eq!(
        tx.cleanup_site_assets(u64::try_from(i64::MAX).unwrap(), 100)
            .await
            .unwrap(),
        0
    );
    tx.commit().await.unwrap();
    svc.reset(&evidence, saved.revision, uuid::Uuid::new_v4(), &budget(&f))
        .await
        .unwrap();
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    assert_eq!(
        tx.cleanup_site_assets(u64::try_from(i64::MAX).unwrap(), 100)
            .await
            .unwrap(),
        1
    );
    tx.commit().await.unwrap();
    assert_eq!(
        scalar(&f, "SELECT count(*) AS value FROM site_asset").await,
        0
    );
    // Parameterized reads use no user-controlled SQL.
    assert!(
        f.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id FROM site_asset WHERE id=$1",
            [asset.id.into()]
        ))
        .await
        .unwrap()
        .is_none()
    );
    f.close().await;
}

#[tokio::test]
async fn site_branding_real_restart_and_database_backup_restore() {
    use std::{process::Stdio, sync::Arc};
    use tokio::io::AsyncWriteExt;
    use yufu::{
        domain::identity::site_branding::*,
        service::identity::{repository::PgIdentityRepository, site_settings::SiteSettingsService},
    };
    let mut f = IdentityFixture::start().await;
    let mut c = BrowserCookies::default();
    let csrf = http_login(&f, &mut c).await;
    let upload = f
        .client
        .post(format!("{}/api/v1/admin/site-assets?kind=logo", f.base_url))
        .header("cookie", c.header())
        .header("origin", &f.base_url)
        .header("x-csrf-token", &csrf)
        .body(PNG)
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), 201);
    assert_eq!(upload.headers()["cache-control"], "no-store");
    let asset: Value = upload.json().await.unwrap();
    let id = asset["id"].as_str().unwrap();
    let mut document = settings(&f, &c).await["settings"]["document"].clone();
    document["branding"]["logo_asset_id"] = json!(id);
    document["branding"]["site_name"] = json!("Backup platform");
    assert_eq!(
        save(
            &f,
            &c,
            &csrf,
            json!({"expected_revision":1,"document":document})
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    f.restart().await;
    let current = settings(&f, &c).await;
    assert_eq!(current["settings"]["revision"], 2);
    assert_eq!(current["settings"]["document"], document);
    let source_name = f.config.database.url.rsplit('/').next().unwrap();
    let dump = tokio::process::Command::new("docker")
        .args([
            "exec",
            f.container_id(),
            "pg_dump",
            "-U",
            "postgres",
            "--format=custom",
            "--no-owner",
            source_name,
        ])
        .output()
        .await
        .unwrap();
    assert!(
        dump.status.success(),
        "{}",
        String::from_utf8_lossy(&dump.stderr)
    );
    let (target, url) = super::super::common::get_postgres_container()
        .await
        .unwrap();
    let target_name = url.rsplit('/').next().unwrap();
    let mut restore = tokio::process::Command::new("docker")
        .args([
            "exec",
            "-i",
            target.id(),
            "pg_restore",
            "-U",
            "postgres",
            "--no-owner",
            "--exit-on-error",
            "--dbname",
            target_name,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    restore
        .stdin
        .take()
        .unwrap()
        .write_all(&dump.stdout)
        .await
        .unwrap();
    let result = restore.wait_with_output().await.unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let restored = sea_orm::Database::connect(url).await.unwrap();
    let service = SiteSettingsService::new(
        Arc::new(PgIdentityRepository::new(restored.clone(), f.clock.clone())),
        f.clock.clone(),
    );
    let public = service.public_branding(&budget(&f)).await.unwrap();
    assert_eq!(public["branding"]["site_name"], "Backup platform");
    assert_eq!(public["revision"], 2);
    let image = service
        .asset(None, id.parse().unwrap(), &budget(&f))
        .await
        .unwrap();
    assert_eq!(image.bytes, PNG);
    assert_eq!(image.sha256, asset["sha256"].as_str().unwrap());
    let mut tx = repository(&f).begin(&budget(&f)).await.unwrap();
    let saved = tx.site_settings(false).await.unwrap();
    assert_eq!(saved.document.branding.site_name, "Backup platform");
    tx.rollback().await.unwrap();
    // Restore includes the same typed configuration contract, not only raw bytes.
    assert_eq!(
        serde_json::from_value::<SiteSettingsDocument>(document)
            .unwrap()
            .branding
            .logo_asset_id,
        Some(id.parse().unwrap())
    );
    restored.close().await.unwrap();
    target.rm().await.unwrap();
    f.close().await;
}
