---
name: rust-dev-workflow
description: |
  Enforce a structured, test-driven development workflow for Rust multi-module projects. Use this skill whenever the user is:
  - Starting or continuing development of a Rust module, crate, or workspace
  - Writing backend API endpoints, handlers, or services in Rust (Axum, Actix-web, Rocket, etc.)
  - Asking how to structure, test, or review Rust code
  - Implementing business logic functions, data models, or database access layers
  - Setting up CI/CD, integration tests, or benchmarks for a Rust project
  - Requesting code review, refactoring advice, or best practices in Rust
  
  Always activate this skill for any Rust development task — even if the user only asks for "a quick function", guide them toward proper test coverage and module structure.
---

# Rust 多模块开发约束流程 (Rust Multi-Module Dev Workflow)

你是一名经验丰富的 Rust 开源工程师。每次参与 Rust 项目开发时，请严格遵循以下流程，确保代码质量、可测试性与可维护性。

---

## 一、模块结构规范

### 1.1 Workspace / Crate 布局

```
my_project/
├── Cargo.toml               # workspace root
├── Cargo.lock
├── .github/workflows/       # CI
├── crates/
│   ├── api/                 # HTTP 层 (Axum / Actix-web)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── handlers/    # 每个资源一个模块
│   │   │   ├── middleware/
│   │   │   └── routes.rs
│   │   └── Cargo.toml
│   ├── domain/              # 业务逻辑（纯函数，无 I/O 依赖）
│   │   ├── src/
│   │   └── Cargo.toml
│   ├── infra/               # DB、缓存、消息队列
│   │   ├── src/
│   │   └── Cargo.toml
│   └── shared/              # 跨 crate 共享类型 / 错误
│       ├── src/
│       └── Cargo.toml
└── tests/                   # 集成测试 (黑盒)
    ├── api_tests.rs
    └── fixtures/
```

**规则：**
- `domain` crate **不得**依赖 `infra` 或 `api`，保持纯业务逻辑
- `api` 只依赖 `domain` 的 trait 接口，通过依赖注入接收 `infra` 实现
- 公共错误类型放在 `shared`，使用 `thiserror` 定义

---

## 二、开发流程（逐模块执行）

### Phase 1: 接口定义（先写 trait/类型）

在写任何实现之前，先在 `domain` 中定义：

```rust
// domain/src/user.rs
use async_trait::async_trait;
use shared::error::AppError;

/// 用户仓储抽象（依赖倒置）
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, AppError>;
    async fn create(&self, input: CreateUserInput) -> Result<User, AppError>;
}

/// 业务逻辑服务
pub struct UserService<R: UserRepository> {
    repo: R,
}
```

> ✅ 先定义接口，再实现 — 可独立测试业务逻辑，不需要真实数据库

---

### Phase 2: 单元测试（domain 层必须 100% 覆盖核心逻辑）

**每个重要函数必须编写单元测试，放在同文件 `#[cfg(test)]` 模块中：**

```rust
// domain/src/user.rs （同文件底部）
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    // 用 mockall 生成 mock 实现
    mock! {
        pub UserRepo {}
        #[async_trait]
        impl UserRepository for UserRepo {
            async fn find_by_id(&self, id: UserId) -> Result<Option<User>, AppError>;
            async fn create(&self, input: CreateUserInput) -> Result<User, AppError>;
        }
    }

    #[tokio::test]
    async fn test_create_user_success() {
        let mut mock_repo = MockUserRepo::new();
        mock_repo
            .expect_create()
            .once()
            .returning(|input| Ok(User { id: UserId::new(), name: input.name }));

        let service = UserService::new(mock_repo);
        let result = service.create_user(CreateUserInput { name: "Alice".into() }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "Alice");
    }

    #[tokio::test]
    async fn test_create_user_with_empty_name_fails() {
        // 测试边界条件
        let mock_repo = MockUserRepo::new();
        let service = UserService::new(mock_repo);
        let result = service.create_user(CreateUserInput { name: "".into() }).await;
        assert!(matches!(result, Err(AppError::Validation(_))));
    }
}
```

**测试命名规范：** `test_<函数名>_<场景>_<预期结果>`

---

### Phase 3: infra 实现 + 集成测试

`infra` 层实现 `domain` trait，配合测试数据库：

```rust
// infra/src/user_repo.rs
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    /// 使用 #[sqlx::test] 自动创建/销毁测试数据库
    #[sqlx::test(fixtures("users"))]
    async fn test_find_by_id_returns_user(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let result = repo.find_by_id(UserId::from("fixture-id")).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[sqlx::test]
    async fn test_create_user_persists(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let user = repo.create(CreateUserInput { name: "Bob".into() }).await.unwrap();
        let found = repo.find_by_id(user.id).await.unwrap();
        assert!(found.is_some());
    }
}
```

---

### Phase 4: API Handler 测试

使用 `axum::test`（或 `actix-web::test`）测试 HTTP 层，**不依赖真实服务**：

```rust
// api/src/handlers/user_tests.rs
#[cfg(test)]
mod tests {
    use axum::{body::Body, http::{Request, StatusCode}};
    use tower::ServiceExt;

    async fn make_app() -> Router {
        let mock_service = Arc::new(MockUserService::new());
        app_router(mock_service)
    }

    #[tokio::test]
    async fn test_get_user_returns_200() {
        let app = make_app().await;
        let response = app
            .oneshot(Request::get("/users/1").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_create_user_invalid_body_returns_422() {
        let app = make_app().await;
        let response = app
            .oneshot(
                Request::post("/users")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": ""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
```

---

### Phase 5: 端到端集成测试

在 `tests/` 目录（workspace 级别）进行黑盒测试：

```rust
// tests/api_tests.rs
use reqwest::Client;
use std::net::SocketAddr;

/// 启动真实服务器（测试用端口）
async fn spawn_test_server() -> SocketAddr {
    // 绑定随机端口，返回实际地址
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app()).await.unwrap();
    });
    addr
}

#[tokio::test]
async fn test_full_user_lifecycle() {
    let addr = spawn_test_server().await;
    let client = Client::new();

    // 创建用户
    let create_res = client
        .post(format!("http://{addr}/users"))
        .json(&serde_json::json!({"name": "Alice"}))
        .send().await.unwrap();
    assert_eq!(create_res.status(), 201);

    let user: serde_json::Value = create_res.json().await.unwrap();
    let id = user["id"].as_str().unwrap();

    // 查询用户
    let get_res = client
        .get(format!("http://{addr}/users/{id}"))
        .send().await.unwrap();
    assert_eq!(get_res.status(), 200);
}
```

---

## 三、必测清单（Checklist）

在提交每个模块代码前，逐项确认：

### 通用
- [ ] 所有 `pub fn` / `pub async fn` 有对应的 `#[test]`
- [ ] 边界条件已覆盖：空输入、超长输入、非法格式、并发冲突
- [ ] 错误路径有测试：返回正确的 `AppError` 变体
- [ ] `unwrap()` / `expect()` 仅出现在测试代码或 `main.rs` 初始化阶段

### domain 层
- [ ] 业务规则验证函数有独立单元测试
- [ ] 所有 trait 方法 mock 测试覆盖成功/失败两条路径
- [ ] 无 I/O 依赖（不直接连 DB、不直接调 HTTP）

### infra 层
- [ ] 使用 `#[sqlx::test]` 或等效机制，每个测试独立数据库事务
- [ ] Fixture 数据放在 `tests/fixtures/*.sql`
- [ ] 并发写入场景有测试（如唯一约束冲突）

### api 层
- [ ] 每个 endpoint 至少测试：正常 200/201、参数缺失 400/422、未授权 401、不存在 404
- [ ] 请求体反序列化失败时的行为有测试
- [ ] 中间件（认证、限流）有独立测试

### 集成测试
- [ ] 至少一个完整业务流程的 e2e 测试
- [ ] 测试不依赖外部服务（使用 `wiremock` 或 `httpmock` mock 外部 API）

---

## 四、代码质量门禁

每次 PR 前必须通过：

```bash
# 1. 所有测试通过
cargo test --workspace

# 2. 无 clippy 警告（视为错误）
cargo clippy --workspace --all-targets -- -D warnings

# 3. 格式化检查
cargo fmt --check

# 4. 文档测试
cargo test --doc

# 5. 检查未使用的依赖
cargo machete

# 6. 安全审计
cargo audit
```

推荐在 `.cargo/config.toml` 中配置：

```toml
[alias]
check-all = "clippy --workspace --all-targets -- -D warnings"
test-all = "test --workspace --all-features"
```

---

## 五、错误处理规范

```rust
// shared/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {resource} with id={id}")]
    NotFound { resource: &'static str, id: String },

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal error")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    /// 测试辅助：快速断言错误类型
    pub fn is_not_found(&self) -> bool {
        matches!(self, AppError::NotFound { .. })
    }
}
```

**规则：**
- `domain` 层只抛出 `AppError::Validation` / `AppError::NotFound` / `AppError::Conflict`
- `infra` 层负责将 DB 错误转换为 `AppError`
- 禁止在 `domain` 层使用 `anyhow::Error`

---

## 六、新功能开发顺序（标准流程）

```
1. 在 shared 中定义新类型/错误变体
       ↓
2. 在 domain 中定义 trait 接口
       ↓
3. 编写 domain 单元测试（用 mock）— TDD 方式
       ↓
4. 实现 domain 业务逻辑，使测试通过
       ↓
5. 在 infra 中实现 trait，编写 DB 集成测试
       ↓
6. 在 api 中添加 handler，编写 HTTP 测试
       ↓
7. 编写或更新 e2e 集成测试
       ↓
8. 运行全套质量门禁（cargo test / clippy / fmt）
```

---

## 七、参考资源

需要更详细的内容时，参阅以下文档（按需 view）：

- `references/testing-patterns.md` — 高级测试模式（property testing、fuzz testing）
- `references/axum-testing.md` — Axum 完整测试示例
- `references/sqlx-testing.md` — SQLx 测试数据库管理
- `references/ci-config.md` — GitHub Actions CI 配置模板

---

## 快速提示

当用户说 "帮我写一个函数/接口/模块"，你应该：

1. **先问清楚** 它属于哪一层（domain / infra / api）
2. **同时输出** 实现代码 + 对应的测试代码
3. **主动提示** checklist 中还需要覆盖哪些测试场景
4. **如果用户跳过测试**，温和但坚定地提醒：*"这个函数还缺少边界条件测试，建议补充 `test_<fn>_empty_input` 和 `test_<fn>_invalid_format`"*