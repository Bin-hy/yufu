use super::super::common::*;
use reqwest::Client;
use yufu::entity::recording::RecordingVO;
use yufu::entity::role::RoleEnum;
use yufu::entity::{PageSelectQuery, PageSelectQueryResult};
use yufu::error::AppError;

#[tokio::test]
async fn test_get_recording_by_page() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;
    let uri = "/recordings?page_number=1&page_size=10";

    let client = Client::new();
    let resp = client
        .get(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());
    let result: PageSelectQueryResult<RecordingVO> = resp.json().await?;
    assert_eq!(result.page_number, 1);
    assert_eq!(result.page_size, 10);

    fixture.close().await;
    Ok(())
}

#[tokio::test]
async fn test_get_recording_by_user_id() -> Result<(), AppError> {
    let fixture = IdentityFixture::start().await;
    let token = super::identity_support::management_access(&fixture).await;

    let client = Client::new();

    // 然后查询该用户的操作记录
    let uri = format!("/recordings/{}", RoleEnum::Superadmin.default_id());

    let query_data = PageSelectQuery::default();
    let resp = client
        .get(format!("{}/api/v1{}", fixture.base_url, uri))
        .header("Authorization", format!("Bearer {}", token))
        .query(&query_data)
        .send()
        .await?
        .error_for_status()?;

    assert!(resp.status().is_success());

    fixture.close().await;
    Ok(())
}
