# rs-gate M4 作战卡：控制台门户与单二进制嵌入 (Web Console & Single Binary)

- **文档编号**：`05-m4-web-console-portal.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 4 步：M4 独立作战卡篇)
- **基准依赖**：
  - 实施总纲：[`docs/04-tasks/01-roadmap.md`](file:///home/fugui/rusts/rs-gate/docs/04-tasks/01-roadmap.md)
  - 架构设计：[`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/05-web-console.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/05-web-console.md)
  - 接口契约：[`docs/03-api/02-admin-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/02-admin-api.md)
- **状态**：定稿待审 (Ready for Review)

---

## 1. 本里程碑目标与文件变更树 (Target & File Tree)

### 1.1 核心目标与范围边界
本阶段聚焦于构建轻量级现代化 Web 控制台与单二进制内嵌打包体系，实现开发者自服务与管理员全局管控：
1. **统一 Auth 与 JWT RBAC 鉴权拦截**：实现用户登录签发 JWT、Cookie/Header 双重提取与多角色（`superadmin` / `admin` / `user`）访问控制；
2. **普通开发者自服务 REST API**：提供 `/api/user/*` 端点，支持可用模型查询、虚拟 Key 自助生命周期管理（创建仅首次返回明文）、个人日/周配额与调用明细下钻；
3. **系统管理员集群治理 REST API**：提供 `/api/admin/*` 端点，覆盖用户管理、全局 Key 监控、一键重置配额（`/reset-quota`）、物理节点即时探活（`/probe`）、逻辑模型与后端映射热重载、全量审计多维筛选与流式 CSV 导出（`/export`）、大盘趋势统计；
4. **现代化高质感前端控制台 SPA**：遵循 Vanilla CSS 现代设计系统规范，基于原生 JS 实现无外部大型框架依赖的双视角单页应用；
5. **`rust-embed` 单二进制内嵌交付**：将前端产物编译期打包进 Rust 二进制文件，支持 MIME 自动推断、ETag 304 协商缓存与 SPA History 路由无缝回退。

### 1.2 涉及文件变更清单

```text
rs-gate/
├── src/
│   └── admin/
│       ├── mod.rs                  # [新增] /api 路由编排与权限分派
│       ├── auth.rs                 # [新增] JWT 编解码、会话提取与 RBAC 拦截
│       ├── user_api.rs             # [新增] 普通用户自服务 REST API 处理器
│       ├── admin_api.rs            # [新增] 管理员集群治理与统计 REST API 处理器
│       └── embedded.rs             # [新增] rust-embed 静态资源处理与 SPA 回退
├── web/
│   ├── package.json                # [新增] 前端工程配置与精简依赖
│   ├── src/
│   │   ├── index.html              # [新增] 控制台 SPA 宿主 HTML
│   │   ├── styles/
│   │   │   ├── tokens.css          # [新增] 色彩变量、排版与视觉设计令牌
│   │   │   ├── components.css      # [新增] 按钮、徽章、表格、进度条组件样式
│   │   │   └── layout.css          # [新增] 侧边栏导航与主视口网格布局
│   │   └── scripts/
│   │       ├── api.js              # [新增] 统一 Fetch REST 客户端
│   │       ├── router.js           # [新增] 轻量原生 History 路由控制器
│   │       ├── user_view.js        # [新增] 开发者自服务视图组件
│   │       └── admin_view.js       # [新增] 管理员监控治理视图组件
│   └── dist/                       # [构建产物] 嵌入 Rust 二进制的前端静态目录
└── tests/
    ├── admin_auth_rbac_test.rs     # [新增] JWT 鉴权、RBAC 越权拦截与数据隔离测试
    ├── user_portal_api_test.rs     # [新增] 开发者自建 Key 仅一次明文与配额查询测试
    ├── admin_operations_test.rs    # [新增] 一键重置配额、节点探活热更新与 CSV 导出测试
    └── embedded_assets_test.rs     # [新增] 嵌入式静态资源分发与 SPA 路由回退测试
```

---

## 2. 测试夹具与 TDD 先行用例 (TDD Test Harness & Failing Tests - Red Phase)

### 2.1 RBAC 权限与多租户数据隔离测试 (`tests/admin_auth_rbac_test.rs`)

```rust
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn test_user_cannot_access_admin_api() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_token = rs_gate::admin::auth::generate_test_token("usr-1", "user");

    let req = Request::builder()
        .uri("/api/admin/users")
        .method("GET")
        .header("authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    // 普通 user 角色必须被 403 强拦截
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_tenant_data_strict_isolation() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_a_token = rs_gate::admin::auth::generate_test_token("usr-a", "user");

    // 用户 A 查询自身 Key 列表
    let req = Request::builder()
        .uri("/api/user/keys")
        .method("GET")
        .header("authorization", format!("Bearer {}", user_a_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    
    // 断言列表中绝对不包含归属于用户 B 的数据
    for item in json["data"]["items"].as_array().unwrap() {
        assert_eq!(item["user_id"], "usr-a");
    }
}
```

### 2.2 用户自建 Key 仅一次明文返回测试 (`tests/user_portal_api_test.rs`)

```rust
#[tokio::test]
async fn test_create_virtual_key_returns_secret_once() {
    let app = rs_gate::server::build_test_admin_app().await;
    let user_token = rs_gate::admin::auth::generate_test_token("usr-1", "user");

    let create_payload = r#"{"name":"Dev Key","daily_token_quota":100000}"#;
    let req = Request::builder()
        .uri("/api/user/keys")
        .method("POST")
        .header("authorization", format!("Bearer {}", user_token))
        .header("content-type", "application/json")
        .body(Body::from(create_payload))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    
    // 创建响应必须包含一次性完整明文 Key
    let plaintext_key = json["data"]["plaintext_key"].as_str().unwrap();
    assert!(plaintext_key.starts_with("sk-gate-"));

    // 随后再次发起列表查询
    let list_req = Request::builder()
        .uri("/api/user/keys")
        .method("GET")
        .header("authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let list_res = app.oneshot(list_req).await.unwrap();
    let list_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX).await.unwrap();
    let list_json: serde_json::Value = serde_json::from_slice(&list_bytes).unwrap();

    // 列表中只包含脱敏 prefix，明文字段必须不存在或为空
    let item = &list_json["data"]["items"][0];
    assert!(item.get("plaintext_key").is_none());
    assert!(item["key_prefix"].as_str().unwrap().starts_with("sk-gate-"));
}
```

### 2.3 管理员一键配额重置与 CSV 导出测试 (`tests/admin_operations_test.rs`)

```rust
#[tokio::test]
async fn test_admin_one_click_reset_quota() {
    let app = rs_gate::server::build_test_admin_app().await;
    let admin_token = rs_gate::admin::auth::generate_test_token("usr-admin", "admin");

    let req = Request::builder()
        .uri("/api/admin/keys/key-001/reset-quota")
        .method("POST")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_admin_streaming_csv_export() {
    let app = rs_gate::server::build_test_admin_app().await;
    let admin_token = rs_gate::admin::auth::generate_test_token("usr-admin", "admin");

    let req = Request::builder()
        .uri("/api/admin/audits/export?start_time=0")
        .method("GET")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get("content-type").unwrap(), "text/csv; charset=utf-8");
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let csv_content = String::from_utf8_lossy(&bytes);
    assert!(csv_content.starts_with("id,trace_id,key_id,model,backend_id"));
}
```

### 2.4 静态资源内嵌与 SPA History 回退测试 (`tests/embedded_assets_test.rs`)

```rust
#[tokio::test]
async fn test_embedded_root_and_spa_history_fallback() {
    let app = rs_gate::server::build_test_app().await;

    // 1. 请求根路径应返回 index.html
    let res = app.clone().oneshot(
        Request::builder().uri("/").method("GET").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get("content-type").unwrap(), "text/html; charset=utf-8");

    // 2. 请求深层前端路由 /portal/keys，必须 SPA 回退至 index.html
    let res_fallback = app.oneshot(
        Request::builder().uri("/portal/keys").method("GET").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res_fallback.status(), StatusCode::OK);
    assert_eq!(res_fallback.headers().get("content-type").unwrap(), "text/html; charset=utf-8");
}
```

---

## 3. 分步工程实施任务 (Step-by-Step Implementation - Green Phase)

### 3.1 Task 4.1：统一 JWT 鉴权中间件与 RBAC 拦截器
- **目标文件**：`src/admin/auth.rs`
- **核心逻辑**：
  - 定义 `Claims { sub: String, role: String, exp: usize }`；
  - 提取请求头 `Authorization: Bearer <token>` 或 Cookie `rsgate_session`；
  - 实现 Axum Extractor：`RequireRole<const R: Role>`，在处理器入口校验角色级别，权限不足时返回标准错误报文 `{ "code": 40301, "message": "Forbidden", "data": null }`。

### 3.2 Task 4.2：普通开发者自服务 REST API 处理器
- **目标文件**：`src/admin/user_api.rs`
- **实现端点**：
  - `GET /api/user/models`：查询当前租户可见的模型列表；
  - `GET /api/user/keys`：分页列出当前用户的 API Key（脱敏显示 `key_prefix`）；
  - `POST /api/user/keys`：生成 32 字符随机密码学高熵字符串 `sk-gate-...`，将加盐哈希入库，并在响应 JSON 中一次性下发明文；
  - `PUT /api/user/keys/:id`：修改白名单与 Key 别名；
  - `DELETE /api/user/keys/:id`：安全停用或删除 Key；
  - `GET /api/user/audits`：隔离查询本人的调用明细。

### 3.3 Task 4.3：系统管理员运维治理与统计 REST API 处理器
- **目标文件**：`src/admin/admin_api.rs`
- **实现端点**：
  - `GET /api/admin/users`, `POST /api/admin/users`：用户全生命周期管控；
  - `GET /api/admin/keys`：全局 Key 监控；
  - `POST /api/admin/keys/:id/reset-quota`：一键重置当前周期配额；
  - `GET /api/admin/backends`, `POST /api/admin/backends/probe`：物理后端探活与协议徽章计算；
  - `POST /api/admin/models`：配置逻辑模型并关联后端集群，触发内存路由表热更新；
  - `GET /api/admin/audits` 与 `GET /api/admin/audits/export`：流式异步导出 CSV；
  - `GET /api/admin/stats`：集群总 QPS、Token 消耗与成功率大盘统计。

### 3.4 Task 4.4：Vanilla CSS 前端控制台 SPA 构建
- **目标目录**：`web/src/`
- **实现要点**：
  - 核心设计系统（`styles/tokens.css`, `styles/components.css`）：采用现代深色/浅色自适应主题、统一圆角、极速响应式布局；
  - 开发者门户与管理员门户双重视角自适应切换；
  - 零外部沉重打包器与大型 JS 框架，采用标准原生 ES Module，产物极小（压缩后 $\le 1\text{MB}$）。

### 3.5 Task 4.5：`rust-embed` 单二进制内嵌与 SPA 路由回退
- **目标文件**：`src/admin/embedded.rs`, `src/admin/mod.rs`
- **实现方式**：
```rust
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
pub struct Asset;

pub async fn static_handler(uri: axum::http::Uri) -> impl axum::response::IntoResponse {
    let path = uri.path().trim_start_matches('/');
    // 若匹配到静态文件则返回并附带 MIME 与 ETag
    // 若未匹配到且非 /api/ 或 /v1/ 前缀，自动回退到 index.html (SPA History Fallback)
}
```

---

## 4. 自动化验收标准与命令行验证手册 (Automated AC & CLI Playbook)

### 4.1 单元与集成测试自动化验证

```bash
# 1. 运行 RBAC 权限与数据隔离测试
cargo test --test admin_auth_rbac_test -- --nocapture

# 2. 运行开发者自服务 API 测试
cargo test --test user_portal_api_test -- --nocapture

# 3. 运行管理员治理、热重载与 CSV 导出测试
cargo test --test admin_operations_test -- --nocapture

# 4. 运行静态资源内嵌与 SPA 路由测试
cargo test --test embedded_assets_test -- --nocapture
```

**验收门禁通过准则**：
- [ ] 普通用户访问 `/api/admin/*` 返回 403，租户只能获取本账户 Key 与审计日志；
- [ ] 自建 Key 创建时成功下发一次性明文，列表接口严格脱敏；
- [ ] 管理员一键重置配额即时生效；
- [ ] CSV 导出为流式传输并包含规范 CSV 表头与 UTF-8 BOM；
- [ ] 浏览器访问前端任意 History 路径返回 `index.html` 且 HTTP 状态码为 200。

### 4.2 命令行黑盒验证手册 (CLI Blackbox Playbook)

```bash
# 登录超级管理员获取 JWT
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"<AdminPassword>"}' | jq -r '.data.token')

# 一键导出审计 CSV
curl -i -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/admin/audits/export -o audit.csv
head -n 5 audit.csv
```
