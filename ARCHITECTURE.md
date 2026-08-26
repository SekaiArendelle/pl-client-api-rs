# pl-client-api-rs 架构设想

## 1. 目标与范围

本项目计划发展为 Physics-Lab API 的 Rust 异步 SDK。它应当：

- 提供符合 Rust 习惯的原生异步 API；
- 隐藏 Token、AuthCode、请求头和服务端 JSON 等协议细节；
- 使用强类型输入减少运行时参数错误；
- 保持公共 API 稳定，同时允许服务端协议继续演化；
- 支持登录、用户、作品、评论、消息、关系、图片和分页数据；
- 默认安全、可测试，并能逐步实现，而不要求一次迁移全部 Python 功能。

当前最小实现只有匿名登录。本文描述长期方向，不要求当前代码立即具备所有模块。

## 2. 设计原则

1. **异步优先**：核心实现只使用原生 `async fn`，不通过线程池包装阻塞请求。
2. **职责分离**：`Client` 管理网络环境，`Session` 管理认证状态，领域模型只表示数据。
3. **组合优于大对象**：不使用同时代表 HTTP 客户端、登录凭据和用户资料的 `User` 大对象。
4. **强类型输入**：用枚举、请求结构体和 ID newtype 代替含义不明确的字符串与整数。
5. **协议与公共模型隔离**：私有 wire DTO 忠实映射服务端字段，公共模型保持 Rust 风格。
6. **按需共享所有权**：异步本身不是使用 `Arc` 的理由；只有出现真实的多所有者需求时才引入它。
7. **显式副作用**：可能产生多次网络请求、重试或修改服务端状态的行为应当可以从 API 和文档中识别。
8. **渐进式类型化**：已确认的响应使用具体类型，暂不稳定的字段允许先保留为 `serde_json::Value`。

## 3. 总体结构

```text
应用代码
   │
   ├── Client ── 登录、公开接口、客户端配置
   │      │
   │      └── login().await
   │               │
   │               ▼
   └──────────── Session ── 认证接口、凭据、当前用户快照
                          │
                          ▼
                    Endpoint 层
                          │
                          ▼
                    Transport 层
                          │
                          ▼
                   Physics-Lab API
```

建议的源码布局：

```text
src/
├── lib.rs
├── client.rs
├── session.rs
├── config.rs
├── credentials.rs
├── error.rs
│
├── model/
│   ├── mod.rs
│   ├── ids.rs
│   ├── user.rs
│   ├── content.rs
│   ├── comment.rs
│   ├── message.rs
│   └── relation.rs
│
├── endpoint/
│   ├── mod.rs
│   ├── auth.rs
│   ├── users.rs
│   ├── contents.rs
│   ├── comments.rs
│   ├── messages.rs
│   └── assets.rs
│
├── stream/
│   ├── mod.rs
│   ├── comments.rs
│   ├── experiments.rs
│   └── notifications.rs
│
├── transport/
│   ├── mod.rs
│   ├── request.rs
│   ├── response.rs
│   └── retry.rs
│
└── wire/
    ├── mod.rs
    ├── auth.rs
    ├── users.rs
    └── contents.rs

tests/
├── anonymous_login.rs
├── comments.rs
├── error_response.rs
└── fixtures/
    ├── authenticate_success.json
    └── authenticate_error.json
```

模块只在实际需要时创建，避免为了匹配目录规划而提前生成空文件。

## 4. Client

`Client` 表示访问 Physics-Lab 服务所需的网络环境，不包含登录凭据或用户资料。

```rust
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    config: ClientConfig,
}
```

它负责：

- 持有并复用 `reqwest::Client`；
- 保存 API、静态资源和上传服务地址；
- 保存版本、语言、设备标识、超时等配置；
- 提供匿名、邮箱和 Token 登录；
- 提供不需要登录态的公开接口；
- 创建 `Session`。

成熟 SDK 应通过 Builder 构造：

```rust
let client = Client::builder()
    .timeout(Duration::from_secs(15))
    .plar_version(2411)
    .build()?;
```

建议的配置结构：

```rust
pub struct ClientConfig {
    pub endpoints: Endpoints,
    pub plar_version: u32,
    pub language: Language,
    pub device_id: String,
    pub timeout: Duration,
}

pub struct Endpoints {
    pub api: Url,
    pub static_assets: Url,
    pub upload: Url,
}
```

生产环境使用默认地址，测试可以把 endpoint 指向本地 mock server。

## 5. Session

`Session` 表示一次已经完成的登录。需要 Token 或 AuthCode 的接口都放在 `Session` 上。

```rust
pub struct Session {
    client: Client,
    credentials: Credentials,
    current_user: CurrentUser,
}
```

登录时克隆 `Client`：

```rust
impl Client {
    pub async fn anonymous_login(&self) -> Result<Session, Error> {
        let response = self.authenticate_anonymously().await?;

        Ok(Session {
            client: self.clone(),
            credentials: response.credentials,
            current_user: response.user,
        })
    }
}
```

这里 `reqwest::Client` 的克隆会继续共享其底层连接池。SDK 不额外包裹 `Arc<ClientInner>`，除非以后出现共享限流器、动态凭据或其他必须保持同一身份的共享状态。

`Session` 默认不实现 `Clone`，以避免无意复制认证信息。如果应用确实需要把它放入多个 `'static` 后台任务，可以由应用显式使用 `Arc<Session>`。

```rust
let session = Arc::new(session);
```

普通并发调用不需要 `Arc`：

```rust
let (comments, messages) = tokio::join!(
    session.get_comments(query),
    session.get_messages(),
);
```

## 6. Credentials 与敏感信息

凭据始终为私有字段：

```rust
struct Credentials {
    token: Option<SecretString>,
    auth_code: SecretString,
    device_token: Option<SecretString>,
}
```

要求：

- 不为凭据派生会输出明文的 `Debug`；
- 错误和日志不能包含密码、Token、AuthCode 或完整认证请求头；
- 调用者不需要手动设置认证头；
- 匿名会话的空 Token 按服务端协议序列化为字符串 `"null"`；
- 除非明确实现安全存储，否则 SDK 不负责把凭据持久化到磁盘。

## 7. CurrentUser 是资料快照

`CurrentUser` 只表示登录响应中得到的用户资料：

```rust
pub struct CurrentUser {
    pub id: UserId,
    pub nickname: Option<String>,
    pub signature: Option<String>,
    pub is_bound: bool,
    pub gold: i64,
    pub level: u32,
    pub avatar: u32,
}
```

修改昵称、签名或领取奖励后，旧值可能过期。SDK 不应悄悄维护一份看似永远最新的可变用户对象，而应显式获取新资料：

```rust
let latest = session.fetch_current_user().await?;
```

如果未来确实需要自动同步缓存，应单独设计缓存语义，而不是在 `Session` 中默认引入锁和内部可变性。

## 8. 公共 API 的归属

不需要认证状态的操作属于 `Client`：

```rust
client.get_start_page().await?;
client.get_avatar(request).await?;
client.anonymous_login().await?;
```

需要 Token 或 AuthCode 的操作属于 `Session`：

```rust
session.get_comments(query).await?;
session.post_comment(request).await?;
session.star_content(request).await?;
session.follow(user_id).await?;
```

不同 endpoint 文件可以分别包含 `impl Session`，从而在内部按领域组织代码，同时保持调用方式直接。

## 9. Wire DTO 与公共模型

服务端协议类型放在私有 `wire` 模块中，并忠实保留字段含义：

```rust
#[derive(Deserialize)]
struct WireUser {
    #[serde(rename = "ID")]
    id: String,

    #[serde(rename = "IsBinded")]
    is_bound: bool,

    #[serde(rename = "Nickname")]
    nickname: Option<String>,
}
```

公共模型使用稳定、符合 Rust 习惯的命名：

```rust
pub struct UserProfile {
    pub id: UserId,
    pub is_bound: bool,
    pub nickname: Option<String>,
}
```

两层之间通过 `TryFrom` 转换和验证：

```rust
impl TryFrom<WireUser> for UserProfile {
    type Error = Error;

    fn try_from(value: WireUser) -> Result<Self, Self::Error> {
        // 验证并转换字段
    }
}
```

服务端字段变化应尽量被限制在 wire 和转换层，不轻易传播到公共 API。

## 10. 强类型请求

避免使用含义不明确的 `str` 和整数：

```rust
pub enum TargetType {
    User,
    Discussion,
    Experiment,
}

pub enum RelationType {
    Followers,
    Following,
}

pub enum StarAction {
    Favorite,
    Unfavorite,
    Support,
}

pub struct UserId(String);
pub struct ContentId(String);
pub struct CommentId(String);
```

请求参数较多时使用请求结构体：

```rust
pub struct GetComments {
    pub target_id: ContentId,
    pub target_type: TargetType,
    pub page_size: usize,
    pub cursor: Option<CommentCursor>,
}
```

类型不应为了追求“全部强类型”而过度复杂。尚未确定结构或服务端经常变化的字段可以暂时使用 `serde_json::Value`。

## 11. Transport 层

Transport 层统一负责：

- 构造 URL；
- 添加公共和认证请求头；
- JSON、gzip、multipart 等编码；
- 检查 HTTP 状态；
- 检查服务端 `Status`；
- 解析响应；
- 超时、有限重试和错误脱敏。

`Session` 内部可以提供认证请求构造器：

```rust
impl Session {
    fn authenticated_post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .post(path)
            .header("x-API-Token", self.credentials.serialized_token())
            .header("x-API-AuthCode", self.credentials.auth_code())
    }
}
```

重复的 JSON 发送和响应检查可以泛型化，但不建立一个掩盖业务含义的“万能 Endpoint”。业务请求仍然保留明确的方法和类型。

## 12. 单页 API 与 Stream

分页首先提供单页方法：

```rust
let page = session.get_comments(query).await?;
```

在单页 API 稳定以后，再增加便利 Stream：

```rust
let mut comments = session.comments_stream(query);

while let Some(comment) = comments.try_next().await? {
    println!("{}", comment.content);
}
```

Stream 层负责：

- 更新 cursor 或 skip；
- 保持结果顺序；
- 限制预取并发；
- Stream 被丢弃时停止后续请求；
- 防止一次排队大量任务；
- 在空页、重复 cursor 等异常情况下可靠结束。

Stream 是便利层，不能取代可独立使用和测试的单页 API。

## 13. 错误模型

错误类型至少区分：

```rust
pub enum Error {
    Transport(reqwest::Error),
    HttpStatus { status: StatusCode },
    Api {
        status: i64,
        code: Option<i64>,
        message: String,
    },
    Decode { source: serde_json::Error },
    InvalidInput {
        field: &'static str,
        message: String,
    },
    MissingField(&'static str),
    Io(std::io::Error),
    RetryExhausted { attempts: usize },
}
```

服务端异常、字段缺失或非法输入都返回 `Result`，不能使用 `assert!` 或 panic 处理正常的失败路径。

## 14. 重试与副作用

默认只重试确定安全的只读请求，例如暂时性网络错误、部分 5xx 或服务端明确要求稍后重试的情况。

评论、关注、收藏、金币支持等修改状态的接口默认不自动重试，除非协议提供可靠的幂等保证。

重试必须：

- 有明确次数上限；
- 使用退避策略；
- 支持取消；
- 不提供默认无限重试；
- 在错误中报告最终尝试次数。

## 15. 测试策略

测试分为三层：

1. **单元测试**：放在对应源码模块或 `src/.../tests.rs`，测试私有 DTO、序列化和转换。
2. **集成测试**：放在 `tests/`，只使用公共 API，并通过可配置 endpoint 连接本地 mock server。
3. **在线测试**：默认忽略，只从环境变量读取凭据，不进入普通 `cargo test`。

测试 fixture 必须脱敏。主要覆盖：

- 请求方法、路径、请求头和 JSON；
- HTTP 错误与 API `Status` 错误；
- 字段缺失、未知字段和非法 JSON；
- 分页顺序、终止、取消和重试；
- 日志与错误中不泄露凭据。

CI 至少运行：

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
```

## 16. 可选 Feature

功能增长后可以考虑：

```toml
[features]
default = ["rustls"]
rustls = []
native-tls = []
upload = []
stream = []
blocking = []
```

原则是：

- 异步 API 始终是核心实现；
- blocking API 只是可选适配层；
- TLS 验证默认开启；
- 上传和较重的便利功能可以按需启用；
- 不为尚未实现的功能提前添加空 feature。

## 17. 演进顺序

推荐按以下顺序扩展：

1. 匿名登录与登录响应类型；
2. 邮箱和 Token 登录，共用认证实现；
3. `ClientConfig`、endpoint 配置和 mock 集成测试；
4. 用户、作品、评论等只读单页接口；
5. 评论、关注、收藏等修改接口；
6. 分页 Stream 与有限重试；
7. 图片下载、上传和作品发布；
8. 在出现真实需求后，再评估共享限流、Token 刷新和 SDK 内部 `Arc`。

## 18. 最终调用体验

```rust
let client = Client::builder()
    .timeout(Duration::from_secs(15))
    .build()?;

let session = client.token_login(token, auth_code).await?;

println!(
    "logged in as {}",
    session
        .current_user()
        .nickname
        .as_deref()
        .unwrap_or("<anonymous>")
);

let comments = session
    .get_comments(GetComments::for_experiment(experiment_id))
    .await?;

session
    .star_content(StarContent::favorite(experiment_id))
    .await?;
```

该架构的核心边界是：

```text
Client       = 网络环境和登录入口
Session      = 认证状态和认证 API
CurrentUser  = 登录时的用户资料快照
model        = 稳定的公共领域类型
wire         = 私有的服务端协议映射
transport    = HTTP、认证头、编码和错误处理
stream       = 建立在单页 API 之上的分页便利层
```

在没有真实共享所有权需求前，SDK 自身不额外使用 `Arc`；如果应用需要跨后台任务共享 `Session`，由应用显式选择 `Arc<Session>`。
