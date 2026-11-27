use super::repository::pg::{get, signed, unsigned};
use crate::domain::identity::{
    account::{
        InvitationListQuery, InvitationPurpose, InvitationRow, InvitationStatus, PersonKind,
    },
    error::IdentityError,
};
use crate::service::security::repository::PgSecurityTransaction;
use uuid::Uuid;

impl PgSecurityTransaction {
    pub(crate) async fn identity_list_invitations(
        &mut self,
        query: InvitationListQuery,
    ) -> Result<Vec<InvitationRow>, IdentityError> {
        let (after_created, after_id) = match query.after.as_deref() {
            Some(value) => {
                let (millis, id) = value.split_once(':').ok_or(IdentityError::InvalidInput)?;
                let millis = millis
                    .parse::<u64>()
                    .map_err(|_| IdentityError::InvalidInput)?;
                let id = Uuid::parse_str(id).map_err(|_| IdentityError::InvalidInput)?;
                (Some(signed(millis)?), Some(id))
            }
            None => (None, None),
        };
        let status = query.status.map(|value| value.as_str().to_owned());
        let purpose = query.purpose.map(|value| match value {
            InvitationPurpose::Onboarding => "onboarding".to_owned(),
            InvitationPurpose::Qualification => "qualification".to_owned(),
        });
        let person_kind = query.person_kind.map(|value| match value {
            PersonKind::Employee => "employee".to_owned(),
            PersonKind::Collaborator => "collaborator".to_owned(),
        });
        let rows = self
            .identity_rows(
                "SELECT i.id,i.purpose,i.target_user_id,i.person_kind,i.developer_grant,i.created_by,i.created_at,i.expires_at,i.consumed_at,i.accepted_user_id,i.revoked_at,tu.username AS target_username,tu.nickname AS target_nickname,cu.username AS creator_username,cu.nickname AS creator_nickname,au.username AS accepted_username,au.nickname AS accepted_nickname,CASE WHEN i.consumed_at IS NOT NULL THEN 'accepted' WHEN i.revoked_at IS NOT NULL THEN 'revoked' WHEN i.expires_at <= $2 THEN 'expired' ELSE 'pending' END AS status FROM identity_invitation i LEFT JOIN \"user\" tu ON tu.id=i.target_user_id LEFT JOIN \"user\" cu ON cu.id=i.created_by LEFT JOIN \"user\" au ON au.id=i.accepted_user_id WHERE ($3::text IS NULL OR (CASE WHEN i.consumed_at IS NOT NULL THEN 'accepted' WHEN i.revoked_at IS NOT NULL THEN 'revoked' WHEN i.expires_at <= $2 THEN 'expired' ELSE 'pending' END)=$3) AND ($4::text IS NULL OR i.purpose=$4) AND ($5::text IS NULL OR i.person_kind=$5) AND ($6::text IS NULL OR POSITION(lower($6) IN lower(tu.username))>0 OR POSITION(lower($6) IN lower(COALESCE(tu.nickname,'')))>0 OR POSITION(lower($6) IN lower(cu.username))>0 OR POSITION(lower($6) IN lower(COALESCE(cu.nickname,'')))>0 OR POSITION(lower($6) IN lower(au.username))>0 OR POSITION(lower($6) IN lower(COALESCE(au.nickname,'')))>0 OR i.id::text=$6) AND ($7::bigint IS NULL OR i.created_at<$7 OR (i.created_at=$7 AND i.id<$8::uuid)) ORDER BY i.created_at DESC,i.id DESC LIMIT $1",
                vec![
                    (i64::from(query.limit) + 1).into(),
                    signed(query.now_millis)?.into(),
                    status.into(),
                    purpose.into(),
                    person_kind.into(),
                    query.search.into(),
                    after_created.into(),
                    after_id.into(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| {
                let purpose = match get::<String>(&row, "purpose")?.as_str() {
                    "onboarding" => InvitationPurpose::Onboarding,
                    "qualification" => InvitationPurpose::Qualification,
                    _ => return Err(IdentityError::DependencyUnavailable),
                };
                let person_kind = match get::<String>(&row, "person_kind")?.as_str() {
                    "employee" => PersonKind::Employee,
                    "collaborator" => PersonKind::Collaborator,
                    _ => return Err(IdentityError::DependencyUnavailable),
                };
                let status = get::<String>(&row, "status")?
                    .parse::<InvitationStatus>()
                    .map_err(|_| IdentityError::DependencyUnavailable)?;
                Ok(InvitationRow {
                    id: get(&row, "id")?,
                    purpose,
                    target_user_id: get(&row, "target_user_id")?,
                    target_username: get(&row, "target_username")?,
                    target_nickname: get(&row, "target_nickname")?,
                    person_kind,
                    developer_grant: get(&row, "developer_grant")?,
                    created_by: get(&row, "created_by")?,
                    creator_username: get(&row, "creator_username")?,
                    creator_nickname: get::<Option<String>>(&row, "creator_nickname")?
                        .unwrap_or_default(),
                    created_at_millis: unsigned(get(&row, "created_at")?)?,
                    expires_at_millis: unsigned(get(&row, "expires_at")?)?,
                    status,
                    accepted_user_id: get(&row, "accepted_user_id")?,
                    accepted_username: get(&row, "accepted_username")?,
                    accepted_nickname: get(&row, "accepted_nickname")?,
                    accepted_at_millis: get::<Option<i64>>(&row, "consumed_at")?
                        .map(unsigned)
                        .transpose()?,
                    revoked_at_millis: get::<Option<i64>>(&row, "revoked_at")?
                        .map(unsigned)
                        .transpose()?,
                })
            })
            .collect()
    }
}
