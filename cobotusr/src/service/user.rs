use crate::entity::refresh_token;
use crate::entity::role::{self, Entity as Role, RoleEnum, RoleVO};
use crate::entity::user::{self, Entity as User, UserVO};
use crate::error::{AppError, SeaResultExt};
use crate::result::Result;
use crate::{AppState, IdType};
use anyhow::anyhow;
use auth::token_manager::TokenManager;
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, QueryFilter, Set};
use sea_orm::{DatabaseConnection, EntityTrait};

/// Audience used for tokens issued to cobotsrv's own UI / legacy login flow.
pub const SELF_AUD: &str = "cobotsrv";

#[derive(Clone)]
pub struct UserService;

impl UserService {
    // 登陆返回string token
    pub async fn auth(
        state: &AppState,
        username: &str,
        password: &str,
    ) -> Result<(String, String, UserVO)> {
        let db = &state.database.connection;
        // 查找用户
        if let Some(exsiting_user) = User::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await?
        {
            // 1. 查找成功，校验密码
            if let Some(password_db) = exsiting_user.clone().password {
                let is_valid = verify(password, &password_db).unwrap_or(false);
                if is_valid {
                    // 2.1 校验成功，分发对应权限的token
                    let jti = uuid::Uuid::new_v4().to_string();

                    let access_token = state
                        .sign_access_token(exsiting_user.id, SELF_AUD, "")
                        .await?;
                    let refresh_token = state
                        .refresh_token_manager()
                        .generate_refresh_token::<IdType>(jti.clone(), exsiting_user.id)?;

                    // 2.1.2 插入到 refresh_token 中 用户SSO 单点登陆
                    refresh_token::Entity::upsert_refresh_token(db, jti, exsiting_user.id).await?;

                    Ok((access_token, refresh_token, exsiting_user.into()))
                } else {
                    // 2.2 失败，密码错误
                    Err(AppError::Unauthorized("password is error".into()))
                }
            } else {
                // 2. 数据错误，没有设置密码
                Err(AppError::InternalServerError(anyhow!(
                    "user database error"
                )))
            }
        } else {
            // 用户不存在
            Err(AppError::Unauthorized("username is not found".into()))
        }
    }

    pub async fn register(
        state: &AppState,
        username: &str,
        password: &str,
        nickname: &str,
    ) -> Result<(String, String)> {
        let (user_db, access_token): (UserVO, String) = UserService::add_user_with_role(
            state,
            username,
            password,
            nickname,
            RoleEnum::User.default_id(),
        )
        .await?;

        // 签发 refresh_token
        let jti = uuid::Uuid::new_v4().to_string();
        let refresh_token = state
            .refresh_token_manager()
            .generate_refresh_token::<IdType>(jti.clone(), user_db.id)?;
        refresh_token::Entity::upsert_refresh_token(&state.database.connection, jti, user_db.id)
            .await?;

        Ok((access_token, refresh_token))
    }

    pub async fn add_user_with_role(
        state: &AppState,
        username: &str,
        password: &str,
        nickname: &str,
        role_id: IdType,
    ) -> Result<(UserVO, String)> {
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
        let access_token = state.sign_access_token(user_vo.id, SELF_AUD, "").await?;

        Ok((user_vo, access_token))
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
    pub async fn refresh_tokens(state: &AppState, refresh_token: &str) -> Result<(String, String)> {
        let db = &state.database.connection;
        let refresh_manager = state.refresh_token_manager();
        let claims = match refresh_manager.verify_refresh_token::<IdType>(refresh_token) {
            Ok(c) => c,
            Err(e) => {
                return Err(AppError::Unauthorized(format!(
                    "refresh token invalid: {:?}",
                    e
                )));
            }
        };
        // 1. 查找 refresh_token 中是否存储
        if !refresh_token::Entity::find_exist(db, claims.jti).await? {
            // 无权限 被下线了
            return Err(AppError::Unauthorized("请重新登陆".to_string()));
        }

        // 2. 存储了 refresh token -> 返回 access_token
        let jti = uuid::Uuid::new_v4().to_string();

        let access_token = state.sign_access_token(claims.sub, SELF_AUD, "").await?;
        let refresh_token_new =
            refresh_manager.generate_refresh_token::<IdType>(jti.clone(), claims.sub)?;
        refresh_token::Entity::upsert_refresh_token(db, jti, claims.sub).await?;
        Ok((access_token, refresh_token_new))
    }

    // 登出 & 清除 刷新token
    pub async fn logout_by_refresh(
        db: &DatabaseConnection,
        refresh_manager: &TokenManager,
        refresh_token: &str,
    ) -> Result<()> {
        let claims = match refresh_manager.verify_refresh_token::<IdType>(refresh_token) {
            Ok(c) => c,
            Err(e) => {
                return Err(AppError::Unauthorized(format!(
                    "refresh token invalid: {:?}",
                    e
                )));
            }
        };

        refresh_token::Entity::delete_by_sub(db, claims.sub).await?;
        Ok(())
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
