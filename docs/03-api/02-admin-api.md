# rs-gate Web 控制台与自服务门户 REST API 契约规范 (Web Console & Portal REST API Specification)

- **文档编号**：`02-admin-api.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 2 步：控制台与门户 REST 契约篇)
- **基准依赖**：[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md), [`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/02-storage-and-models.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/02-storage-and-models.md), [`docs/02-design/03-routing-and-proxy.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/03-routing-and-proxy.md), [`docs/02-design/04-limiting-and-audit.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/04-limiting-and-audit.md), [`docs/02-design/05-web-console.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/05-web-console.md)
- **状态**：定稿待审 (Ready for Review)

---

## 1. 规范概述与统一交互契约 (Overview & Protocol Standards)

### 1.1 系统定位与协议边界
Web 控制台与自服务门户提供面向企业内部不同角色（超级管理员、系统管理员与业务研发者）的统一 HTTP REST API 契约。
所有控制台端点均以 `/api/` 为根路由，与网关模型透明代理前缀 `/v1/` 进行强物理隔离：
- `/api/auth/*`：统一用户登录鉴权、Token 刷新与个人身份查询；
- `/api/user/*`：面向普通研发者（`user` 角色）的自服务门户接口，严格实现用户级数据隔离（自建 Key、看自用配额、查本人审计明细、获取代码接入指引）；
- `/api/admin/*`：面向系统运维人员（`admin`、`superadmin` 角色）的全局集群治理接口（用户管理、物理节点探活、逻辑模型拓扑热配、配额一键重置、全量审计下钻与统计图表）；
- `/health/*` 与 `/metrics`：面向容器编排（K8s）与监控体系（Prometheus）的运维观测端点。

### 1.2 统一响应报文格式 (Unified Envelope)

所有控制台 REST API 响应均封装为统一的 JSON 信封结构：

#### 1. 成功响应规范 (`code == 0`)
```json
{
  "code": 0,
  "message": "success",
  "data": { ... }
}
```

#### 2. 失败错误响应规范 (`code != 0`)
```json
{
  "code": 40101,
  "message": "Incorrect username or password",
  "data": null
}
```

#### 3. 分页列表响应规范 (Pagination Envelope)
```json
{
  "code": 0,
  "message": "success",
  "data": {
    "items": [ ... ],
    "total": 128,
    "page": 1,
    "page_size": 20,
    "has_more": true
  }
}
```

### 1.3 身份认证与 RBAC 权限矩阵

1. **认证机制 (Bearer JWT & HttpOnly Cookie)**：
   - 登录成功后，服务端下发基于 HMAC-SHA256 签名的 JWT Token；
   - 客户端在后续请求中通过 `Authorization: Bearer <jwt_token>` 头传递（亦支持同构 HttpOnly Cookie `rsgate_token`）；
   - Token Payload 包含：`sub` (user_id), `username`, `role`, `exp` (默认 7 天有效期)。
2. **角色权限级别定义**：
   - `superadmin`：超级管理员，享有全部权限，包括创建/注销管理员账号、重置系统配置；
   - `admin`：系统管理员，可管理后端服务器、模型路由热配、查看全量审计、代建/重置所有 Key；
   - `user`：普通研发者，仅可访问 `/api/user/*` 自服务接口，所有数据操作隐式注入 `WHERE user_id = :current_user_id`。

---

## 2. 统一用户认证契约 (`/api/auth/*`)

### 2.1 用户登录：`POST /api/auth/login`

**接口说明**：校验用户名与密码哈希（Argon2id / PBKDF2），签发 JWT 凭证。

- **权限要求**：公开 (Public)
- **请求体 (Request Body)**：
  ```json
  {
    "username": "developer_alice",
    "password": "SecurePassword123!"
  }
  ```

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
      "expires_at": 1729900800,
      "user": {
        "id": "usr-9a8b7c6d",
        "username": "developer_alice",
        "display_name": "Alice Chen",
        "email": "alice@internal.corp",
        "role": "user",
        "created_at": 1729296000
      }
    }
  }
  ```

### 2.2 用户登出：`POST /api/auth/logout`

**接口说明**：注销登录状态，清除服务端 Session 或客户端 Cookie。

- **权限要求**：已登录用户 (`user` / `admin` / `superadmin`)
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": null
  }
  ```

### 2.3 获取当前身份：`GET /api/auth/me`

**接口说明**：根据携带的 JWT 获取当前登录用户信息及所属权限概览。

- **权限要求**：已登录用户
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "user": {
        "id": "usr-9a8b7c6d",
        "username": "developer_alice",
        "display_name": "Alice Chen",
        "email": "alice@internal.corp",
        "role": "user"
      },
      "quota_summary": {
        "total_keys": 2,
        "daily_tokens_used": 257850,
        "daily_tokens_limit": 1000000,
        "weekly_tokens_used": 1589800,
        "weekly_tokens_limit": 5000000
      }
    }
  }
  ```

---

## 3. 普通开发者自服务门户契约 (`/api/user/*`)

本组端点面向普通研发人员，仅能操作与当前登录用户归属（`user_id`）相关的数据。

### 3.1 获取可用模型列表与接入指南：`GET /api/user/models`

**接口说明**：查询网关中所有当前处于激活状态（`is_active == 1`）的逻辑模型，供自服务门户展示模型广场与接入指引。

- **权限要求**：`user` / `admin` / `superadmin`
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": [
      {
        "id": "deepseek-r1",
        "display_name": "DeepSeek R1 (推理强化旗舰)",
        "description": "具备强劲推理链思考能力，推荐复杂数学分析与逻辑推理使用。",
        "capabilities": ["chat", "responses"],
        "max_retries": 2,
        "is_active": true,
        "code_snippets": {
          "base_url": "http://gate.internal.corp:8080/v1",
          "python": "from openai import OpenAI\n\nclient = OpenAI(\n    base_url=\"http://gate.internal.corp:8080/v1\",\n    api_key=\"YOUR_API_KEY\",\n)\nresponse = client.chat.completions.create(\n    model=\"deepseek-r1\",\n    messages=[{\"role\": \"user\", \"content\": \"你好\"}]\n)\nprint(response.choices[0].message.content)",
          "curl": "curl http://gate.internal.corp:8080/v1/chat/completions \\\n  -H \"Authorization: Bearer YOUR_API_KEY\" \\\n  -H \"Content-Type: application/json\" \\\n  -d '{\"model\": \"deepseek-r1\", \"messages\": [{\"role\": \"user\", \"content\": \"你好\"}]}'"
        }
      },
      {
        "id": "qwen-2.5-72b-instruct",
        "display_name": "Qwen 2.5 72B (通用指令大模型)",
        "description": "全栈通用业务对话与代码编写模型，支持结构化 JSON 输出与长文本。",
        "capabilities": ["chat", "responses", "completions"],
        "max_retries": 2,
        "is_active": true,
        "code_snippets": { ... }
      }
    ]
  }
  ```

### 3.2 获取本人 API Key 列表与配额进度：`GET /api/user/keys`

**接口说明**：获取当前登录用户名下的所有虚拟 API Key，并附带日/周配额的实时已消耗进度与重置倒计时。

- **权限要求**：`user` / `admin` / `superadmin`
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": [
      {
        "id": "key-0192a3b4-c5d6",
        "name": "电商搜索推荐微服务",
        "key_prefix": "sk-gate-7a8f***",
        "daily_quota": 1000000,
        "weekly_quota": 5000000,
        "rpm_limit": 60,
        "max_concurrency": 5,
        "model_whitelist": ["deepseek-r1", "qwen-2.5-72b-instruct"],
        "is_active": true,
        "expires_at": null,
        "created_at": 1729296000,
        "quota_status": {
          "daily_used": 257850,
          "daily_remaining": 742150,
          "daily_percentage": 25.79,
          "daily_reset_at": 1729353599,
          "weekly_used": 1589800,
          "weekly_remaining": 3410200,
          "weekly_percentage": 31.80,
          "weekly_reset_at": 1729439999
        }
      }
    ]
  }
  ```

### 3.3 自助申领创建新 Key：`POST /api/user/keys`

**接口说明**：普通用户自助申领虚拟 Key。**明文密钥（`plaintext_key`）仅在此次请求成功中返回一次**，后续不可逆哈希存储。

- **权限要求**：`user` / `admin` / `superadmin`
- **请求体 (Request Body)**：
  ```json
  {
    "name": "知识库 RAG 评测脚本",
    "model_whitelist": ["deepseek-r1", "bge-m3"],
    "expires_at": 1731888000
  }
  ```
  *(注：普通用户创建时，`daily_quota`、`weekly_quota`、`rpm_limit` 默认由系统策略模板自动注入，防止超额预分配)*

- **响应体 (201 Created)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "id": "key-0192a3b4-e7f8",
      "name": "知识库 RAG 评测脚本",
      "plaintext_key": "sk-gate-9f8e7d6c5b4a312012345678abcdef01",
      "key_prefix": "sk-gate-9f8e***",
      "daily_quota": 1000000,
      "weekly_quota": 5000000,
      "rpm_limit": 60,
      "max_concurrency": 5,
      "model_whitelist": ["deepseek-r1", "bge-m3"],
      "created_at": 1729350000
    }
  }
  ```

### 3.4 修改本人 Key 配置：`PUT /api/user/keys/:id`

**接口说明**：修改本人名下指定 Key 的显示名称与可用模型白名单。

- **请求体 (Request Body)**：
  ```json
  {
    "name": "知识库 RAG 评测脚本 (更新)",
    "model_whitelist": ["deepseek-r1", "bge-m3", "bge-reranker-large"]
  }
  ```
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "id": "key-0192a3b4-e7f8",
      "name": "知识库 RAG 评测脚本 (更新)",
      "model_whitelist": ["deepseek-r1", "bge-m3", "bge-reranker-large"]
    }
  }
  ```

### 3.5 删除/停用本人 Key：`DELETE /api/user/keys/:id`

**接口说明**：注销或删除本人指定的 API Key。

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Key revoked successfully",
    "data": null
  }
  ```

### 3.6 查询本人调用历史流水：`GET /api/user/audits`

**接口说明**：按分页查询当前用户所属 Key 产生的调用记录与报错信息，用于自助排查业务调用失败原因。

- **查询参数 (Query Parameters)**：
  - `key_id` (String, 可选)：按特定虚拟 Key ID 过滤；
  - `model` (String, 可选)：按调用的模型名称过滤；
  - `status` (Integer, 可选)：过滤状态码（如 `200`、`429`、`502`）；
  - `start_time` (Integer, 可选)：起始时间戳（毫秒）；
  - `end_time` (Integer, 可选)：截止时间戳（毫秒）；
  - `page` (Integer, 默认 1)；
  - `page_size` (Integer, 默认 20，最大 100)。

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "items": [
        {
          "id": "aud-0192a3b4-1122",
          "trace_id": "c8b415a7-96a9-4b68-b762-d3a956321f00",
          "key_id": "key-0192a3b4-c5d6",
          "key_name": "电商搜索推荐微服务",
          "model": "deepseek-r1",
          "protocol": "chat",
          "http_status": 200,
          "prompt_tokens": 128,
          "completion_tokens": 512,
          "total_tokens": 640,
          "request_duration_ms": 1420,
          "ttft_ms": 280,
          "is_stream": 1,
          "retry_count": 0,
          "error_message": null,
          "created_at": 1729350120000
        },
        {
          "id": "aud-0192a3b4-3344",
          "trace_id": "9a8b7c6d-5e4f-3a2b-1c0d-e1f2a3b4c5d6",
          "key_id": "key-0192a3b4-c5d6",
          "key_name": "电商搜索推荐微服务",
          "model": "deepseek-r1",
          "protocol": "chat",
          "http_status": 429,
          "prompt_tokens": 0,
          "completion_tokens": 0,
          "total_tokens": 0,
          "request_duration_ms": 2,
          "ttft_ms": null,
          "is_stream": 0,
          "retry_count": 0,
          "error_message": "Rate limit exceeded (RPM limit reached)",
          "created_at": 1729350180000
        }
      ],
      "total": 52,
      "page": 1,
      "page_size": 20,
      "has_more": true
    }
  }
  ```

---

## 4. 管理员运维与治理管控契约 (`/api/admin/*`)

本组端点受严格的 RBAC 控制，仅限 `admin` 或 `superadmin` 角色访问。

### 4.1 用户账户与权限治理 (`/api/admin/users`)

#### 1. 查询用户列表：`GET /api/admin/users`
- **查询参数**：`keyword`（模糊搜索用户名/邮箱/昵称）、`role`（`superadmin`|`admin`|`user`）、`page`、`page_size`。
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "items": [
        {
          "id": "usr-superadmin-01",
          "username": "admin",
          "display_name": "System SuperAdmin",
          "email": "ops-admin@internal.corp",
          "role": "superadmin",
          "is_active": 1,
          "keys_count": 5,
          "created_at": 1729296000,
          "updated_at": 1729296000
        },
        {
          "id": "usr-0192a3b4-5566",
          "username": "developer_alice",
          "display_name": "Alice Chen",
          "email": "alice@internal.corp",
          "role": "user",
          "is_active": 1,
          "keys_count": 2,
          "created_at": 1729300000,
          "updated_at": 1729300000
        }
      ],
      "total": 2,
      "page": 1,
      "page_size": 20,
      "has_more": false
    }
  }
  ```

#### 2. 创建新用户：`POST /api/admin/users`
- **请求体 (Request Body)**：
  ```json
  {
    "username": "developer_bob",
    "password": "InitialPassword123!",
    "role": "user",
    "display_name": "Bob Wang",
    "email": "bob@internal.corp"
  }
  ```
- **响应体 (201 Created)**：返回创建成功的用户对象（不包含敏感密码哈希）。

#### 3. 编辑用户及密码重置：`PUT /api/admin/users/:id`
- **请求体 (Request Body)**：
  ```json
  {
    "role": "admin",
    "display_name": "Bob Wang (Promoted)",
    "email": "bob@internal.corp",
    "is_active": 1,
    "password": "NewSecurePassword456!"
  }
  ```
  *(注：`password` 可选填，不填表示保持原密码不变)*

#### 4. 删除/停用用户：`DELETE /api/admin/users/:id`
- **说明**：禁用该用户并级联失效其所有的 API Key（`superadmin` 账号具备防误删保护，不可删除自身）。
- **路径参数**：`id` (String，目标用户 ID)
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "User deactivated successfully and all associated API keys disabled",
    "data": null
  }
  ```

---

### 4.2 全局虚拟 Key 管控与一键配额纠偏 (`/api/admin/keys`)

#### 1. 全局 Key 监控检索：`GET /api/admin/keys`
- **查询参数**：
  - `user_id` (String, 可选)：按特定归属用户筛选；
  - `keyword` (String, 可选)：按 Key 名称或脱敏前缀模糊检索；
  - `is_active` (Integer, 可选)：按状态筛选 (`1` 启用，`0` 禁用)；
  - `page` (Integer, 默认 1)；
  - `page_size` (Integer, 默认 20)。
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "items": [
        {
          "id": "key-0192a3b4-c5d6",
          "user_id": "usr-0192a3b4-5566",
          "user_username": "developer_alice",
          "user_display_name": "Alice Chen",
          "name": "电商搜索推荐微服务",
          "key_prefix": "sk-gate-7a8f***",
          "daily_quota": 5000000,
          "weekly_quota": 25000000,
          "rpm_limit": 300,
          "max_concurrency": 20,
          "model_whitelist": ["deepseek-r1", "qwen-2.5-72b-instruct"],
          "ip_whitelist": "10.0.0.0/8",
          "is_active": 1,
          "expires_at": null,
          "created_at": 1729296000,
          "updated_at": 1729350000,
          "quota_status": {
            "daily_used": 1250400,
            "daily_remaining": 3749600,
            "daily_percentage": 25.01,
            "daily_reset_at": 1729353599,
            "weekly_used": 6840000,
            "weekly_remaining": 18160000,
            "weekly_percentage": 27.36,
            "weekly_reset_at": 1729439999
          }
        }
      ],
      "total": 12,
      "page": 1,
      "page_size": 20,
      "has_more": false
    }
  }
  ```

#### 2. 管理员代建 Key：`POST /api/admin/keys`
- **请求体 (Request Body)**：
  ```json
  {
    "user_id": "usr-0192a3b4-5566",
    "name": "重点项目核心链路 Key",
    "daily_quota": 5000000,
    "weekly_quota": 25000000,
    "rpm_limit": 300,
    "max_concurrency": 20,
    "model_whitelist": ["deepseek-r1", "qwen-2.5-72b-instruct"],
    "ip_whitelist": "10.0.0.0/8,192.168.1.0/24"
  }
  ```
- **响应体 (201 Created)**：仅本次返回 `plaintext_key`。

#### 3. 强制修改 Key 参数：`PUT /api/admin/keys/:id`
- **请求体 (Request Body)**：
  ```json
  {
    "name": "重点项目核心链路 Key (配额调整)",
    "daily_quota": 8000000,
    "weekly_quota": 40000000,
    "rpm_limit": 500,
    "max_concurrency": 30,
    "model_whitelist": ["deepseek-r1", "qwen-2.5-72b-instruct", "qwen-2.5-coder-32b"],
    "ip_whitelist": "10.0.0.0/8",
    "is_active": 1
  }
  ```
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Key configuration updated successfully",
    "data": {
      "id": "key-0192a3b4-c5d6",
      "name": "重点项目核心链路 Key (配额调整)",
      "daily_quota": 8000000,
      "weekly_quota": 40000000,
      "rpm_limit": 500,
      "max_concurrency": 30,
      "model_whitelist": ["deepseek-r1", "qwen-2.5-72b-instruct", "qwen-2.5-coder-32b"],
      "is_active": 1,
      "updated_at": 1729350250
    }
  }
  ```

#### 4. 强制吊销 Key：`DELETE /api/admin/keys/:id`
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Key permanently revoked",
    "data": null
  }
  ```

#### 5. 一键配额手动重置 (Quota Manual Reset)：`POST /api/admin/keys/:id/reset-quota`
**接口说明**：解决突发业务压测或紧急保障导致日/周配额超限阻断问题。清零指定周期已用 Token，实时恢复通行。

- **请求体 (Request Body)**：
  ```json
  {
    "period_type": "daily"
  }
  ```
  *(参数可选值：`"daily"` 重置今日已用、`"weekly"` 重置本周已用、`"both"` 双周期全量清零)*

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Daily quota usage has been reset to 0",
    "data": {
      "key_id": "key-0192a3b4-c5d6",
      "period_type": "daily",
      "prompt_tokens": 0,
      "completion_tokens": 0,
      "total_tokens": 0,
      "reset_timestamp": 1729350300000
    }
  }
  ```

---

### 4.3 物理后端实例与协议能力看板 (`/api/admin/backends`)

#### 1. 查询物理后端实例列表：`GET /api/admin/backends`
**接口说明**：展示后端实例运行状态、权重、实时活跃连接数以及基于能力探针生成的**协议徽章位图**。

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": [
      {
        "id": "be-vllm-01",
        "name": "GPU-Cluster-Node-01 (vLLM)",
        "endpoint_url": "http://10.0.1.10:8000",
        "weight": 10,
        "capabilities_mask": 3,
        "capabilities": ["chat", "responses"],
        "status": "Healthy",
        "consecutive_failures": 0,
        "active_connections": 4,
        "created_at": 1729296000,
        "updated_at": 1729350000
      },
      {
        "id": "be-sglang-02",
        "name": "GPU-Cluster-Node-02 (SGLang)",
        "endpoint_url": "http://10.0.1.11:30000",
        "weight": 15,
        "capabilities_mask": 7,
        "capabilities": ["chat", "responses", "completions"],
        "status": "Healthy",
        "consecutive_failures": 0,
        "active_connections": 8,
        "created_at": 1729296000,
        "updated_at": 1729350000
      },
      {
        "id": "be-tei-03",
        "name": "Embedding-TEI-Node-01",
        "endpoint_url": "http://10.0.1.12:8080",
        "weight": 5,
        "capabilities_mask": 24,
        "capabilities": ["embeddings", "rerank"],
        "status": "CircuitBroken",
        "consecutive_failures": 3,
        "active_connections": 0,
        "created_at": 1729296000,
        "updated_at": 1729350200
      }
    ]
  }
  ```

#### 2. 注册物理后端：`POST /api/admin/backends`
**接口说明**：添加物理实例，保存后立即异步触发初次协议探活并设置徽章与健康状态。

- **请求体 (Request Body)**：
  ```json
  {
    "id": "be-vllm-03",
    "name": "GPU-Cluster-Node-03 (New)",
    "endpoint_url": "http://10.0.1.13:8000",
    "upstream_api_key": "vllm-secret-token",
    "weight": 10
  }
  ```
- **响应体 (201 Created)**：返回包含探活初值的后端对象。

#### 3. 编辑物理后端：`PUT /api/admin/backends/:id`
- **请求体 (Request Body)**：
  ```json
  {
    "name": "GPU-Cluster-Node-01 (vLLM-Updated)",
    "endpoint_url": "http://10.0.1.10:8000",
    "weight": 20,
    "upstream_api_key": "new-vllm-secret-token",
    "health_status": "healthy"
  }
  ```
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Backend updated successfully",
    "data": {
      "id": "be-vllm-01",
      "name": "GPU-Cluster-Node-01 (vLLM-Updated)",
      "endpoint_url": "http://10.0.1.10:8000",
      "weight": 20,
      "health_status": "healthy",
      "updated_at": 1729350350
    }
  }
  ```

#### 4. 下线并移除后端：`DELETE /api/admin/backends/:id`
- **说明**：从数据库及所有关联的模型后端池中解绑并移除目标后端。
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Backend server removed successfully and unmounted from all models",
    "data": null
  }
  ```

#### 5. 即时能力与健康探活：`POST /api/admin/backends/:id/probe`
**接口说明**：管理员在界面点击“立即探测”，网关即刻向该后端并发执行协议探测套件，刷新 `capabilities_mask` 与健康状态机。

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Probe completed successfully",
    "data": {
      "backend_id": "be-vllm-01",
      "latency_ms": 12,
      "capabilities_mask": 3,
      "capabilities": ["chat", "responses"],
      "status": "Healthy",
      "probed_endpoints": {
        "/v1/chat/completions": { "status_code": 400, "supported": true },
        "/v1/responses": { "status_code": 400, "supported": true },
        "/v1/completions": { "status_code": 404, "supported": false },
        "/v1/embeddings": { "status_code": 404, "supported": false },
        "/v1/rerank": { "status_code": 404, "supported": false }
      }
    }
  }
  ```

---

### 4.4 逻辑模型集群与重试容错热配置 (`/api/admin/models`)

#### 1. 查询逻辑模型列表及后端映射：`GET /api/admin/models`
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": [
      {
        "id": "deepseek-r1",
        "display_name": "DeepSeek R1 (推理强化旗舰)",
        "description": "企业级 DeepSeek 推理模型",
        "max_retries": 2,
        "retry_interval_ms": 500,
        "is_active": 1,
        "created_at": 1729296000,
        "updated_at": 1729350000,
        "backends": [
          {
            "backend_id": "be-vllm-01",
            "name": "GPU-Cluster-Node-01 (vLLM)",
            "weight": 10,
            "priority": 1,
            "status": "Healthy",
            "active_connections": 4
          },
          {
            "backend_id": "be-sglang-02",
            "name": "GPU-Cluster-Node-02 (SGLang)",
            "weight": 15,
            "priority": 1,
            "status": "Healthy",
            "active_connections": 8
          }
        ]
      }
    ]
  }
  ```

#### 2. 创建逻辑模型：`POST /api/admin/models`
- **请求体 (Request Body)**：
  ```json
  {
    "id": "qwen-2.5-coder-32b",
    "display_name": "Qwen 2.5 Coder 32B",
    "description": "专精代码生成与审查模型",
    "max_retries": 2,
    "retry_interval_ms": 500,
    "is_active": 1
  }
  ```
- **响应体 (201 Created)**：
  ```json
  {
    "code": 0,
    "message": "Model created successfully",
    "data": {
      "id": "qwen-2.5-coder-32b",
      "display_name": "Qwen 2.5 Coder 32B",
      "description": "专精代码生成与审查模型",
      "max_retries": 2,
      "retry_interval_ms": 500,
      "is_active": 1,
      "created_at": 1729350400000,
      "updated_at": 1729350400000
    }
  }
  ```

#### 3. 修改逻辑模型参数：`PUT /api/admin/models/:id`
- **请求体 (Request Body)**：
  ```json
  {
    "display_name": "Qwen 2.5 Coder 32B (High Priority)",
    "description": "专精代码生成与审查模型 (优化升级版)",
    "max_retries": 3,
    "retry_interval_ms": 300,
    "is_active": 1
  }
  ```
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Model updated successfully and routing table reloaded",
    "data": {
      "id": "qwen-2.5-coder-32b",
      "display_name": "Qwen 2.5 Coder 32B (High Priority)",
      "description": "专精代码生成与审查模型 (优化升级版)",
      "max_retries": 3,
      "retry_interval_ms": 300,
      "is_active": 1,
      "updated_at": 1729350450000
    }
  }
  ```

#### 4. 绑定后端实例至模型并配置权重：`POST /api/admin/models/:id/backends`
**接口说明**：为模型挂载物理后端，配置调度权重。操作成功后**自动触发内存路由表热重载**，WLAC 调度器即刻生效。

- **请求体 (Request Body)**：
  ```json
  {
    "backend_id": "be-vllm-01",
    "weight": 20,
    "priority": 1
  }
  ```
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Backend bound to model successfully and routing table reloaded",
    "data": null
  }
  ```

#### 5. 从模型中解绑后端实例：`DELETE /api/admin/models/:id/backends/:backend_id`
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "Backend unmounted from model successfully and routing table reloaded",
    "data": null
  }
  ```

---

### 4.5 全局调用审计与账单导出 (`/api/admin/audits`)

#### 1. 全量多维审计检索：`GET /api/admin/audits`
**接口说明**：面向运维排查的高性能分页检索，支持下钻查看**重试次数（`retry_count`）**与**故障转移节点轨迹（`failover_path`）**。

- **查询参数 (Query Parameters)**：
  - `user_id` (String, 可选)
  - `key_id` (String, 可选)
  - `model` (String, 可选)
  - `backend_id` (String, 可选)
  - `http_status` (Integer, 可选)
  - `has_retried` (Boolean, 可选，只看发生过重试的请求)
  - `min_duration_ms` (Integer, 可选)
  - `start_time` (Integer, 可选)
  - `end_time` (Integer, 可选)
  - `page` (Integer, 默认 1)
  - `page_size` (Integer, 默认 50)

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "items": [
        {
          "id": "aud-0192a3b4-8899",
          "trace_id": "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d",
          "key_id": "key-0192a3b4-c5d6",
          "user_id": "usr-0192a3b4-5566",
          "username": "developer_alice",
          "model": "deepseek-r1",
          "backend_id": "be-sglang-02",
          "protocol": "chat",
          "client_ip": "10.244.2.15",
          "http_status": 200,
          "prompt_tokens": 256,
          "completion_tokens": 1024,
          "total_tokens": 1280,
          "request_duration_ms": 2340,
          "ttft_ms": 310,
          "is_stream": 1,
          "retry_count": 1,
          "failover_path": "[\"be-vllm-01 (503)\", \"be-sglang-02 (200)\"]",
          "error_message": null,
          "created_at": 1729350400000
        }
      ],
      "total": 14205,
      "page": 1,
      "page_size": 50,
      "has_more": true
    }
  }
  ```

#### 2. 审计流水流式导出：`GET /api/admin/audits/export`
**接口说明**：按指定时间范围与过滤条件将调用日志流式导出为 CSV 文本，供企业财务对账、部门内部成本核算与故障排查存档。

- **查询参数 (Query Parameters)**：
  - `start_time` (Integer, 必填)：导出起始毫秒时间戳；
  - `end_time` (Integer, 必填)：导出截止毫秒时间戳；
  - `user_id` (String, 可选)：按特定用户筛选；
  - `key_id` (String, 可选)：按特定 Key 筛选；
  - `model` (String, 可选)：按调用模型筛选；
  - `http_status` (Integer, 可选)：按状态码筛选。

- **响应格式 (200 OK 流式输出)**：
  - **响应头**：
    ```http
    HTTP/1.1 200 OK
    Content-Type: text/csv; charset=utf-8
    Content-Disposition: attachment; filename="rs-gate-audits-20261019.csv"
    Transfer-Encoding: chunked
    ```
  - **CSV 数据流内容示例**：
    ```csv
    id,trace_id,key_id,user_id,username,model,backend_id,protocol,client_ip,http_status,prompt_tokens,completion_tokens,total_tokens,request_duration_ms,ttft_ms,is_stream,retry_count,failover_path,error_message,created_at
    aud-0192a3b4-8899,a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d,key-0192a3b4-c5d6,usr-0192a3b4-5566,developer_alice,deepseek-r1,be-sglang-02,chat,10.244.2.15,200,256,1024,1280,2340,310,1,1,"[""be-vllm-01 (503)"", ""be-sglang-02 (200)""]",,1729350400000
    aud-0192a3b4-8900,b2c3d4e5-f6a7-8b9c-0d1e-2f3a4b5c6d7e,key-0192a3b4-c5d6,usr-0192a3b4-5566,developer_alice,qwen-2.5-72b-instruct,be-vllm-01,chat,10.244.2.15,200,128,512,640,1180,195,1,0,,,1729350410000
    ```

---

### 4.6 监控运维大盘与指标时序 (`/api/admin/stats`)

#### 1. 监控大盘实时概览：`GET /api/admin/stats/overview`
**接口说明**：前端控制台顶部核心指标卡片，每 5 秒静默轮询。

- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "current_qps": 48.5,
      "active_connections": 12,
      "today_total_requests": 158420,
      "today_total_tokens": 68421500,
      "today_error_rate": 0.02,
      "avg_p99_latency_ms": 185,
      "healthy_backends_count": 5,
      "total_backends_count": 6
    }
  }
  ```

#### 2. 吞吐与 Token 消耗趋势时序：`GET /api/admin/stats/trends`
- **查询参数**：`interval` (`"1h"` | `"24h"` | `"7d"`)
- **响应体 (200 OK)**：
  ```json
  {
    "code": 0,
    "message": "success",
    "data": {
      "timeline": ["10:00", "11:00", "12:00", "13:00", "14:00", "15:00"],
      "series": {
        "qps": [22.4, 38.1, 45.6, 32.0, 48.5, 52.3],
        "tokens_per_sec": [12400, 24100, 31200, 18500, 34200, 41000],
        "errors": [0, 1, 0, 0, 2, 0]
      }
    }
  }
  ```

---

## 5. 系统监控与运维基础设施端点 (System & Ops Endpoints)

### 5.1 进程存活探针：`GET /health/live`
- **状态码**：`200 OK`
- **响应体**：`{"status": "alive"}`

### 5.2 组件就绪探针：`GET /health/ready`
- **说明**：检查 SQLite WAL 连接池健康度与内存路由表完整性。
- **状态码**：健康返回 `200 OK`，数据库卡死或无后端时返回 `503 Service Unavailable`。
- **响应体**：
  ```json
  {
    "status": "ready",
    "database": "connected",
    "active_models": 4,
    "healthy_backends": 5
  }
  ```

### 5.3 Prometheus OpenMetrics 导出：`GET /metrics`
- **说明**：输出 Prometheus 兼容纯文本度量指标。
- **内容示例**：
  ```text
  # HELP rsgate_http_requests_total Total number of HTTP proxy requests processed
  # TYPE rsgate_http_requests_total counter
  rsgate_http_requests_total{model="deepseek-r1",status="200"} 12450
  rsgate_http_requests_total{model="deepseek-r1",status="429"} 32
  rsgate_http_requests_total{model="deepseek-r1",status="502"} 5

  # HELP rsgate_active_connections Current active in-flight proxy connections
  # TYPE rsgate_active_connections gauge
  rsgate_active_connections{backend="be-vllm-01"} 4
  rsgate_active_connections{backend="be-sglang-02"} 8

  # HELP rsgate_upstream_retries_total Total silent retry attempts performed
  # TYPE rsgate_upstream_retries_total counter
  rsgate_upstream_retries_total{model="deepseek-r1",attempt="1"} 18
  rsgate_upstream_retries_total{model="deepseek-r1",attempt="2"} 3
  ```

---

## 6. 控制台统一业务错误码字典 (Error Code Dictionary)

| 业务错误码 (`code`) | HTTP 状态码 | 英文标识符 | 含义与排查指南 |
| :---: | :---: | :--- | :--- |
| `0` | 200 | `SUCCESS` | 请求处理成功。 |
| `40001` | 400 | `BAD_REQUEST_PARAM` | 请求体 JSON 解析失败或字段缺失。 |
| `40101` | 401 | `UNAUTHORIZED` | 账号密码错误或未登录。 |
| `40102` | 401 | `TOKEN_EXPIRED` | JWT 鉴权凭据过期，需重新登录。 |
| `40301` | 403 | `FORBIDDEN` | 用户角色权限不足（如普通用户尝试调用管理员端点）。 |
| `40401` | 404 | `USER_NOT_FOUND` | 指定 ID 的用户不存在。 |
| `40402` | 404 | `KEY_NOT_FOUND` | 指定 ID 的虚拟 Key 不存在。 |
| `40403` | 404 | `BACKEND_NOT_FOUND` | 指定 ID 的物理后端不存在。 |
| `40404` | 404 | `MODEL_NOT_FOUND` | 指定 ID 的逻辑模型不存在。 |
| `40901` | 409 | `RESOURCE_CONFLICT` | 用户名或模型 ID 已存在冲突。 |
| `50001` | 500 | `INTERNAL_DATABASE_ERROR` | SQLite 数据库执行异常或写入冲突。 |
| `50201` | 502 | `PROBE_FAILED` | 后端即时探活网络握手失败。 |

---

## 7. 契约验收标准与后续规划 (Verification Checklist & Next Steps)

### 7.1 控制台 REST API 契约验收标准 (AC Checklist)

- [x] **信封与 RBAC 规范统一**：标准统一的 `code/message/data` 信封响应与 JWT/Cookie 鉴权机制。
- [x] **自服务门户闭环**：完整覆盖普通开发者自服务所需的模型广场（`/api/user/models`）、Key 申领（仅明文返回一次）、日/周双进度条计算（`/api/user/keys`）与个人报错下钻（`/api/user/audits`）。
- [x] **管理员管控全景**：覆盖用户管理、Key 全局治理与一键清零配额（`/reset-quota`）、物理后端能力徽章与即时探活（`/probe`）、逻辑模型集群绑定与重试参数热配。
- [x] **重试与故障转移下钻支持**：在模型配置中支持 `max_retries` / `retry_interval_ms`，在审计接口中精准输出 `retry_count` 与 `failover_path`。
- [x] **系统观测完备**：覆盖 `/health/live`、`/health/ready` 及 Prometheus `/metrics` 指标输出。

### 7.2 下一步工作计划

根据阶段三编制节奏：
👉 **执行阶段三第 3 步**：编制 [`docs/03-api/03-database-schema.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/03-database-schema.md)（【存储数据契约】数据库 DDL 与字段字典契约规范，全面对齐今日新增字段 `max_retries`、`retry_interval_ms`、`retry_count`、`failover_path` 与初始种子数据）。
