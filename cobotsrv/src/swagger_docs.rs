#[cfg(feature = "openapi")]
pub mod doc {
    use crate::route::auth::AuthApi;
    use crate::route::user::UserApi;
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
        ),
    )]
    pub struct ApiDoc;
}
