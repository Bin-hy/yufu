#[cfg(feature = "openapi")]
pub mod doc {
    use crate::route::auth::AuthApi;
    use crate::route::authority::AuthorityApi;
    use crate::route::rbac::RbacApi;
    use crate::route::recording::RecordingApi;
    use crate::route::role::RoleApi;
    use crate::route::user::UserApi;
    use crate::route::user_authority::UserAuthorityApi;
    use utoipa::OpenApi;

    // 使用derive宏来创建OpenAPI文档
    #[derive(OpenApi)]
    #[openapi(
        info(
            title = "后端接口文档",
            description = "这是服务器的API文档",
            version = "0.0.1"
        ),
        servers((url= "http://localhost:4000/api/v1")),
        paths(),
        nest(
            (path ="/api/v1",api = AuthApi, tags= ["Auth身份校验"]),
            (path ="/api/v1",api = UserApi, tags= ["User 用户"]),
            (path ="/api/v1",api = RoleApi, tags= ["角色信息相关CRUD"]),
            (path ="/api/v1",api = AuthorityApi, tags= ["所有请求的权限信息CRUD"]),
            (path ="/api/v1",api = RecordingApi, tags= ["用户操作记录查询"]),
            (path ="/api/v1",api = RbacApi, tags= ["RBAC权限校验"]),
            (path ="/api/v1",api = UserAuthorityApi, tags= ["用户权限覆写"])
        ),
    )]
    pub struct ApiDoc;
}
