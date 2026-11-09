use super::super::common::*;
use cobotusr::entity::authority::{
    AuthorityDeleteReq, AuthorityInsertReq, AuthorityPatchReq, AuthorityVO,
};
use cobotusr::error::AppError;
use reqwest::Client;

#[tokio::test]
async fn test_add_authority() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/authority";

    let client = Client::new();
    // build body
    let body = AuthorityInsertReq {
        name: "test_authority".to_string(),
        authority_uri: "/test/authority/path".to_string(),
        method: "GET".to_string(),
    };

    let resp = client
        .post(format!("{}/api/v1{}", fixture.base_url, uri))
        .json(&body)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let authority: AuthorityVO = resp.json().await?;
    assert_eq!(authority.name, "test_authority");
    assert_eq!(authority.authority_uri, "/test/authority/path");

    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_get_page_authorities() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/authority?page_number=1&page_size=10";

    let client = Client::new();
    let resp = client
        .get(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_patch_authority() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/authority";

    let client = Client::new();
    // 先创建一个权限
    let create_body = AuthorityInsertReq {
        name: "patch_test_authority".to_string(),
        authority_uri: "/patch/test/authority".to_string(),
        method: "PATCH".to_string(),
    };

    let resp = client
        .post(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&create_body)
        .send()
        .await?
        .error_for_status()?;

    let authority: AuthorityVO = resp.json().await?;

    // 权限变更已撤销创建请求所用的管理会话，重新认证后继续。
    let token = super::identity_support::management_access(&fixture).await;
    // 然后修改这个权限
    let patch_body = AuthorityPatchReq {
        id: authority.id,
        name: Some("patched_authority".to_string()),
        authority_uri: Some("/patch_test_authority".to_string()),
        method: Some("PATCH".to_string()),
    };

    let resp = client
        .patch(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&patch_body)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_delete_authority() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/authority";

    let client = Client::new();
    // 先创建一个权限
    let create_body = AuthorityInsertReq {
        name: "delete_test_authority".to_string(),
        authority_uri: "/delete/test/authority".to_string(),
        method: "DELETE".to_string(),
    };

    let resp = client
        .post(format!("{}/api/v1{}", fixture.base_url, uri))
        .json(&create_body)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    let authority: AuthorityVO = resp.json().await?;

    // 权限变更已撤销创建请求所用的管理会话，重新认证后继续。
    let token = super::identity_support::management_access(&fixture).await;
    // 然后删除这个权限
    let delete_body = AuthorityDeleteReq { id: authority.id };

    let resp = client
        .delete(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&delete_body)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    fixture.close().await;
    Ok(())
}
