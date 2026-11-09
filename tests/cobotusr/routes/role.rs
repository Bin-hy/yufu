use super::super::common::*;
use cobotusr::entity::PageSelectQueryResult;
use cobotusr::entity::role::{RoleAddReq, RolePatchReq, RoleVO};
use cobotusr::error::AppError;
use reqwest::Client;

#[tokio::test]
async fn test_add_role() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/role";

    let client = Client::new();
    let role_name = format!("test_role_{}", uuid::Uuid::new_v4());

    let role_req = RoleAddReq { name: role_name };

    let resp = client
        .post(format!("{}/api/v1{}", fixture.base_url, uri))
        .json(&role_req)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_get_page_role() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/role?page_number=1&page_size=10";

    let client = Client::new();
    let resp = client
        .get(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let result: PageSelectQueryResult<RoleVO> = resp.json().await?;
    assert_eq!(result.page_number, 1);
    assert_eq!(result.page_size, 10);

    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_patch_role() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/role";

    let client = Client::new();
    // 先创建一个角色
    let role_name = format!("test_role_{}", uuid::Uuid::new_v4());

    let role_req = RoleAddReq { name: role_name };

    let resp = client
        .post(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&role_req)
        .send()
        .await?
        .error_for_status()?;

    let role: RoleVO = resp.json().await?;

    // 修改角色
    let new_name = "更新后的测试角色描述";
    let patch_req = RolePatchReq {
        id: role.id,
        name: new_name.to_string(),
    };

    let resp = client
        .patch(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .json(&patch_req)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let updated_role: RoleVO = resp.json().await?;
    assert_eq!(updated_role.name, new_name.to_string());

    fixture.close().await;
    Ok(())
}
