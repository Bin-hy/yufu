use crate::entity::role::{self, Entity as Role, RoleEnum, RoleVO};
use crate::entity::user::{self, Entity as User, UserVO};
use crate::error::AppError;
use crate::result::Result;
use anyhow::anyhow;
use auth::token_manager::TokenManager;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, QueryFilter, Set};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

// use auth_old::
#[derive(Clone)]
pub struct UserService;

impl UserService {
    // 登陆返回string token
    pub async fn auth(
        db: &DatabaseConnection,
        token_manager: &TokenManager,
        username: &str,
        password: &str,
    ) -> Result<(String, UserVO)> {
        // 查找用户
        if let Some(exsiting_user) = User::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await?
        {
            // 1. 查找成功，校验密码
            if let Some(password_db) = exsiting_user.clone().password {
                let is_valid = password == password_db;
                if is_valid {
                    // 2.1 校验成功，分发对应权限的token
                    let token = token_manager.generate_token(&exsiting_user.id)?;
                    Ok((token, exsiting_user.into()))
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
        db: &DatabaseConnection,
        token_manager: &TokenManager,
        username: &str,
        password: &str,
        nickname: &str,
    ) -> Result<String> {
        // 1. 检查用户是否存在
        if User::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await?
            .is_some()
        {
            return Err(AppError::AlreadyExists("username already exists".into()));
        }

        // 2. 构造 ActiveModel
        let new_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let new_user = user::ActiveModel {
            id: Set(new_id.clone()),
            username: Set(username.to_owned()),
            password: Set(Some(password.to_owned())), // ⚠️ 可以换成加密密码
            create_time: Set(now),
            update_time: Set(now),
            nickname: Set(Some(nickname.to_owned())),
            role_id: Set(RoleEnum::User.default_id().to_string()),
            description: Set(None),
        };

        // 3. 插入数据库
        let inserted = new_user.insert(db).await?;

        // 4. 注册后直接生成 token
        let token = token_manager.generate_token(&inserted.id)?;

        Ok(token)
    }

    pub async fn get_user_by_id(db: &DatabaseConnection, user_id: String) -> Result<UserVO> {
        if let Some((user_db, role_db_opt)) = user::Entity::find()
            .find_also_related(role::Entity)
            .filter(user::Column::Id.eq(user_id))
            .one(db)
            .await?
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

    /// 获取用户角色
    pub async fn _get_user_role(db: &DatabaseConnection, user_id: String) -> Result<RoleVO> {
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
