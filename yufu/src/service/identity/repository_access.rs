use super::repository::pg::{environment_kind, get};
use crate::{
    domain::identity::{
        admission::ManagedAdmission,
        application::{EnvironmentConfig, LaunchUrl},
        error::IdentityError,
    },
    service::security::repository::PgSecurityTransaction,
};

impl PgSecurityTransaction {
    pub(crate) async fn identity_application_launch(
        &self,
        admission: &ManagedAdmission,
    ) -> Result<LaunchUrl, IdentityError> {
        self.identity_application_entry(admission)
            .await?
            .launch_url
            .ok_or(IdentityError::AccessDenied)
    }

    pub(crate) async fn identity_access_sources(
        &self,
        user_id: i32,
        application_id: i32,
        environment_id: uuid::Uuid,
        kind: &str,
    ) -> Result<Vec<crate::domain::identity::access::AccessSource>, IdentityError> {
        use crate::domain::identity::access::{AccessSource, AccessSourceKind};
        let rows = self.identity_rows(
            "SELECT 'direct' AS kind,user_id::text AS source_id,'直接分配' AS label FROM identity_app_assignment WHERE application_id=$1 AND user_id=$2 AND $4='production' UNION ALL SELECT 'group',g.id::text,g.name FROM identity_app_assignment x JOIN identity_group_member m ON m.group_id=x.group_id JOIN identity_user_group g ON g.id=m.group_id WHERE x.application_id=$1 AND m.user_id=$2 AND $4='production' UNION ALL SELECT 'maintainer',application_id::text,'应用维护者' FROM identity_app_member WHERE application_id=$1 AND user_id=$2 AND $4='development' UNION ALL SELECT 'test_user',environment_id::text,'开发测试访问' FROM identity_app_test_user WHERE environment_id=$3 AND user_id=$2 AND $4='development' ORDER BY kind,source_id LIMIT 1001",
            vec![application_id.into(),user_id.into(),environment_id.into(),kind.into()],
        ).await?;
        if rows.len() > 1000 {
            return Err(IdentityError::CapacityExceeded);
        }
        rows.iter()
            .map(|row| {
                Ok(AccessSource {
                    kind: match get::<String>(row, "kind")?.as_str() {
                        "direct" => AccessSourceKind::Direct,
                        "group" => AccessSourceKind::Group,
                        "maintainer" => AccessSourceKind::Maintainer,
                        "test_user" => AccessSourceKind::TestUser,
                        _ => return Err(IdentityError::DependencyUnavailable),
                    },
                    source_id: get(row, "source_id")?,
                    label: get(row, "label")?,
                })
            })
            .collect()
    }
    pub(crate) async fn identity_application_entry(
        &self,
        admission: &ManagedAdmission,
    ) -> Result<EnvironmentConfig, IdentityError> {
        let row = self.identity_one(
            "SELECT v.payload,e.kind FROM identity_app_environment e JOIN identity_app_config_version v ON v.environment_id=e.id AND v.id=e.active_version_id WHERE e.id=$1 AND v.id=$2",
            vec![admission.environment_id().into(), admission.config_version_id().into()],
        ).await?;
        EnvironmentConfig::from_payload(
            get(&row, "payload")?,
            environment_kind(&get::<String>(&row, "kind")?)?,
            true,
        )
    }
}
