use crate::entity::role::{self, Entity as Role, RoleVO};
use crate::entity::user::{self, Entity as User, UserVO};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use crate::{AppState, IdType};
use anyhow::anyhow;
use bcrypt::{DEFAULT_COST, hash};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, QueryFilter, Set};
use sea_orm::{DatabaseConnection, EntityTrait};

use super::security::{
    lifecycle::{
        CredentialLifecycle, LocalLoginRequest, LocalPassword, LocalRegisterRequest,
        SessionTokenConfig,
    },
    repository::PgSecurityRepository,
};
use auth::security::{budget::SecurityBudget, error::SecurityError, identity::SecretToken};
use std::sync::Arc;

#[derive(Clone)]
pub struct UserService;

impl UserService {
    pub async fn auth(
        state: &AppState,
        username: &str,
        password: &str,
        budget: &SecurityBudget,
    ) -> Result<(String, String, UserVO)> {
        let (lifecycle, config) = Self::local_lifecycle(state)?;
        let password = LocalPassword::new(password.to_owned())?;
        let session = lifecycle
            .login_local(
                LocalLoginRequest {
                    username,
                    password: &password,
                },
                &config,
                state.id_token_signer().as_ref(),
                budget,
            )
            .await?;
        Ok((
            session.tokens.access_token.expose_secret().to_owned(),
            session.tokens.refresh_token.expose_secret().to_owned(),
            session.user,
        ))
    }

    pub async fn register(
        state: &AppState,
        username: &str,
        password: &str,
        nickname: &str,
        budget: &SecurityBudget,
    ) -> Result<(String, String)> {
        let (lifecycle, config) = Self::local_lifecycle(state)?;
        let password = LocalPassword::new(password.to_owned()).map_err(local_registration_error)?;
        let session = lifecycle
            .register_local(
                LocalRegisterRequest {
                    username,
                    password: &password,
                    nickname,
                },
                &config,
                state.id_token_signer().as_ref(),
                budget,
            )
            .await
            .map_err(local_registration_error)?;
        Ok((
            session.tokens.access_token.expose_secret().to_owned(),
            session.tokens.refresh_token.expose_secret().to_owned(),
        ))
    }

    pub async fn add_user_with_role(
        state: &AppState,
        username: &str,
        password: &str,
        nickname: &str,
        role_id: IdType,
    ) -> Result<UserVO> {
        LocalPassword::new(password.to_owned()).map_err(local_registration_error)?;
        let db = &state.database.connection;
        // 1. 检查role是否存在
        let role = role::Entity::find_by_id(role_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::NotFound("role_id does not exist".to_string()))?;

        // 2. 构造 ActiveModel
        let now = Utc::now().naive_utc();
        let hashed_password = hash(password, DEFAULT_COST)
            .map_err(|e| AppError::InternalServerError(anyhow!("Password hash failed: {}", e)))?;
        let new_user = user::ActiveModel {
            id: ActiveValue::NotSet,
            username: Set(username.to_owned()),
            password: Set(Some(hashed_password)),
            create_time: Set(now),
            update_time: Set(now),
            nickname: Set(Some(nickname.to_owned())),
            role_id: Set(role.id.to_owned()),
            description: Set(None),
        };

        // 3. 插入数据库
        let inserted = new_user
            .insert(db)
            .await
            .into_app_with_conflict_msg(format!("用户名 {}，已经存在", username))?;

        let mut user_vo = UserVO::from(inserted);
        user_vo.set_role(role::RoleVO::from(role));
        Ok(user_vo)
    }

    pub async fn get_user_by_id(db: &DatabaseConnection, user_id: IdType) -> Result<UserVO> {
        if let Some((user_db, role_db_opt)) =
            user::Entity::find_with_role_by_id(db, user_id).await?
        {
            let mut user_vo = UserVO::from(user_db);
            if let Some(role) = role_db_opt {
                user_vo.set_role(RoleVO::from(role));
            }
            Ok(user_vo)
        } else {
            Err(AppError::InternalServerError(anyhow!("获取用户失败")))
        }
    }

    // 获取刷新token
    pub async fn refresh_tokens(
        state: &AppState,
        refresh_token: &str,
        budget: &SecurityBudget,
    ) -> Result<(String, String)> {
        let (lifecycle, config) = Self::local_lifecycle(state)?;
        let refresh = SecretToken::new(refresh_token.to_owned())?;
        let tokens = lifecycle
            .rotate_local_refresh(&refresh, &config, state.id_token_signer().as_ref(), budget)
            .await?;
        Ok((
            tokens.access_token.expose_secret().to_owned(),
            tokens.refresh_token.expose_secret().to_owned(),
        ))
    }
    fn local_lifecycle(state: &AppState) -> Result<(CredentialLifecycle, SessionTokenConfig)> {
        let clock = state.security_clock()?;
        let config = SessionTokenConfig {
            issuer: state.issuer(),
            access_ttl_seconds: u64::try_from(state.config.auth.exp_seconds)
                .map_err(|_| SecurityError::DependencyUnavailable)?,
            refresh_ttl_seconds: u64::try_from(state.config.auth.refresh_exp_seconds)
                .map_err(|_| SecurityError::DependencyUnavailable)?,
        };
        let repository = Arc::new(PgSecurityRepository::new(
            state.database.connection.clone(),
            clock.clone(),
        ));
        Ok((CredentialLifecycle::new(repository, clock), config))
    }

    /// End the registered local session; never infer a user from a legacy JWT.
    pub async fn logout_by_refresh(
        state: &AppState,
        refresh: &SecretToken,
        redirect: Option<(&str, &str)>,
        budget: &SecurityBudget,
    ) -> Result<super::security::revocation::RevocationReceipt> {
        Ok(super::security::browser::lifecycle(state)?
            .logout_local(refresh, &state.issuer(), redirect, budget)
            .await?)
    }

    /// 获取用户角色
    pub async fn _get_user_role(db: &DatabaseConnection, user_id: IdType) -> Result<RoleVO> {
        // 查询用户
        let user = match User::find_by_id(user_id).one(db).await? {
            Some(u) => u,
            None => return Err(AppError::NotFound("user not found".into())),
        };

        // 获取 role_id
        let role_id = user.role_id;

        // 查询角色
        let role_db = match Role::find()
            .filter(role::Column::Id.eq(role_id))
            .one(db)
            .await?
        {
            Some(r) => r,
            None => return Err(AppError::InternalServerError(anyhow!("role is not found"))),
        };

        // 转换为 VO
        Ok(role_db.into())
    }
}
fn local_registration_error(error: SecurityError) -> AppError {
    match error {
        SecurityError::InvalidCredential => {
            AppError::BadRequest("invalid registration data".into())
        }
        error => AppError::Security(error),
    }
}
