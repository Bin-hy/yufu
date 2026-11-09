use cobotusr::entity::PageSelectQuery;
use cobotusr::entity::role::RoleEnum;
use cobotusr::entity::user::{UserAddWithRoleReq, UserPatchReq, UserVO};
use cobotusr::error::AppError;
use reqwest::Client;

use super::super::common::*;

#[tokio::test]
async fn test_get_page_users() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/users";
    let query_page = PageSelectQuery::default();
    let client = Client::new();
    let resp = client
        .get(format!("{}/api/v1{}", fixture.base_url, uri))
        .query(&query_page)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_patch_user() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;

    // 先注册一个用户
    let client = Client::new();

    // 修改用户信息
    let uri = "/user";
    let new_nickname = "更新后的测试用户";
    let patch_req = UserPatchReq {
        user_id: RoleEnum::Superadmin.default_id(),
        nickname: Some(new_nickname.to_string()),
        password: None,
    };

    let resp = client
        .patch(format!("{}/api/v1{}", fixture.base_url, uri))
        .json(&patch_req)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let updated_user: UserVO = resp.json().await?;
    assert_eq!(updated_user.nickname, Some(new_nickname.to_string()));
    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_get_user_by_id() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;

    let client = Client::new();

    // 获取用户信息
    let uri = "/user".to_string();

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
async fn test_add_user_with_role() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;

    let client = Client::new();

    // 获取用户信息
    let uri = "/user".to_string();

    let test_user_name = uuid::Uuid::new_v4().to_string();
    let test_password = uuid::Uuid::new_v4().to_string();
    let payload = UserAddWithRoleReq {
        username: test_user_name.clone(),
        role_id: RoleEnum::Admin.default_id(),
        password: test_password.clone(),
        nickname: None,
    };
    let resp = client
        .post(format!("{}/api/v1{}", fixture.base_url, uri))
        .json(&payload)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    assert!(resp.text().await?.contains("邀请"));

    fixture.close().await;
    Ok(())
}
