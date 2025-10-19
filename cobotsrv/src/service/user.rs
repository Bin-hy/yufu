use crate::entity::user::{self, Entity as User};
use crate::error::AppError;
use crate::result::Result;
use crate::route::user::UserVO;
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
                    let token = token_manager.generate_token(&*exsiting_user.id)?;
                    return Ok((token, exsiting_user.into()));
                } else {
                    // 2.2 失败，密码错误
                    return Err(AppError::Unauthorized("password is error".into()));
                };
            } else {
                // 2. 数据错误，没有设置密码
                return Err(AppError::InternalServerError(anyhow!(
                    "user database error"
                )));
            }
        } else {
            // 用户不存在
            return Err(AppError::Unauthorized("username is not found".into()));
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
        if let Some(_) = User::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await?
        {
            return Err(AppError::AlreadyExists("username already exists".into()));
        }

        // 2. 构造 ActiveModel
        let new_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let new_user = user::ActiveModel {
            id: Set(new_id.clone()),
            username: Set(Some(username.to_owned())),
            password: Set(Some(password.to_owned())), // ⚠️ 可以换成加密密码
            create_time: Set(now),
            update_time: Set(now),
            nickname: Set(Some(nickname.to_owned())),
            group_id: Set(None),
            role_id: Set(None),
            description: Set(None),
        };

        // 3. 插入数据库
        let inserted = new_user.insert(db).await?;

        // 4. 注册后直接生成 token
        let token = token_manager.generate_token(&*inserted.id)?;

        Ok(token)
    }

    pub async fn get_user_by_id(db: &DatabaseConnection, user_id: String) -> Result<UserVO> {
        if let Some(db_user) = User::find()
            .filter(user::Column::Id.eq(user_id))
            .one(db)
            .await?
        {
            Ok(UserVO {
                username: db_user.username.unwrap(),
                nickname: db_user.nickname,
                role: "admin".to_string(), // todo
            })
        } else {
            Err(AppError::InternalServerError(anyhow!("获取用户失败")))
        }
    }
}
