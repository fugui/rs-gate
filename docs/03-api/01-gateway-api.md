# rs-gate 网关代理 API 契约规范 (Gateway Proxy API Specification)

- **文档编号**：`01-gateway-api.md`
- **文档版本**：`v1.0.0`
- **编制日期**：2026-10-19
- **当前阶段**：阶段三：接口契约与任务分解 (第 1 步：代理协议契约篇)
- **基准依赖**：[`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md), [`docs/02-design/01-system-architecture.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md), [`docs/02-design/03-routing-and-proxy.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/03-routing-and-proxy.md), [`docs/02-design/04-limiting-and-audit.md`](file:///home/fugui/rusts/rs-gate/docs/02-design/04-limiting-and-audit.md)
- **状态**：定稿待审 (Ready for Review)

---

## 1. 规范概述与核心设计原则 (Overview & Design Principles)

### 1.1 系统定位与协议边界
**rs-gate** 定位为企业级内部高性能大模型透明中继网关。所有外部应用（业务系统、AI Agent、LangChain/LlamaIndex、OpenAI 官方 SDK、命令行工具等）均通过统一的 HTTP 入口调用大模型服务。

网关全面坚持以下核心协议原则：
1. **零跨协议 AST 转换损耗 (Passthrough-First)**：
   - 网关对 `/v1/*` 代理流量保持原始 JSON 负载或原始数据流透传，严禁在网关内部对请求体或响应体进行昂贵的跨厂商协议 AST 深度反序列化与语义重塑。
   - 网关内部中继处理开销严格控制在 $P99 \le 1.5\text{ms}$，首字透传附加延迟（TTFT Overhead）$\le 1\text{ms}$。
2. **100% 兼容 OpenAI 客户端生态**：
   - 客户端仅需将 `base_url` 指向 rs-gate（如 `http://<gate_host>:8080/v1`），并将 `api_key` 替换为 rs-gate 颁发的虚拟 API Key（`sk-gate-...`），即可无缝接入现有所有标准 SDK 与大模型应用生态。
3. **真实凭据完全脱敏与物理后端隔离**：
   - 客户端使用的虚拟 API Key 绝不触碰物理后端。网关在内存中完成鉴权与配额检查后，向上游物理后端（vLLM、SGLang、TEI、第三方 API 提供商）发起请求时动态替换为后端托管真实凭据。
4. **统一链路追踪与度量透传**：
   - 全链路强制注入并透传 `x-request-id`；出站响应头实时回显日/周配额余量、重置时间及当前 Key 瞬时并发连接数。
5. **统一标准错误协议**：
   - 网关拦截的所有错误（鉴权失败、CIDR 拒绝、限流、配额耗尽、后端全熔断、超时等）均严格封装为标准 OpenAI 错误结构体 `{"error": {"message": "...", "type": "...", "code": ...}}`。

---

## 2. 协议头契约与凭证隔离 (HTTP Headers Contract)

### 2.1 入站请求头规范 (Inbound Request Headers)

客户端向网关发起代理请求时，需遵循以下请求头契约：

| 请求头字段 | 类型 | 是否必填 | 规范说明与约束 |
| :--- | :---: | :---: | :--- |
| `Authorization` | String | **必选** | 统一承载虚拟 API Key，格式必须为 `Bearer sk-gate-<32位随机熵字符串>`。缺失或格式不符将直接返回 401。 |
| `Content-Type` | String | 条件必选 | 对于 `POST` 请求，必须为 `application/json`；非 JSON 请求体将直接返回 400。 |
| `Accept` | String | 可选 | 客户端期望接收的内容格式。流式请求建议显式声明 `text/event-stream`；非流式建议声明 `application/json`。 |
| `x-request-id` | String | 可选 | 客户端自定义的全链路请求追踪 ID（如 UUID 或业务 TraceID）。若客户端未提供，网关将通过 UUIDv4 自动生成并注入。 |
| `X-Forwarded-For` | String | 可选 | 上游反向代理传递的客户端 IP 列表。网关将基于此字段最左侧有效公网/内网 IP 或 TCP 远端地址执行 CIDR 白名单匹配。 |

### 2.2 出站响应头规范 (Outbound Response Headers)

无论是普通非流式 JSON 响应、SSE 流式建立首包响应，还是被网关拦截的 4xx/5xx 错误响应，rs-gate 均保证在出站 HTTP Header 中完整透传配额与度量状态：

| 响应头字段 | 类型 | 示例值 | 业务含义与说明 |
| :--- | :---: | :--- | :--- |
| `x-request-id` | String | `c8b415a7-96a9-4b68-b762-d3a956321f00` | 本次请求在网关全生命周期与审计日志中的全局唯一追踪标识符。 |
| `X-Quota-Daily-Limit` | Integer | `1000000` | 当前 API Key 配置的**日配额总上限**（单位：Token）。`0` 表示不限日额度。 |
| `X-Quota-Daily-Remaining` | Integer | `742150` | 今日**剩余可用日配额**（单位：Token）。 |
| `X-Quota-Daily-Reset` | Integer | `1729353599` | 今日配额重置的 Unix 时间戳（秒级），对齐为当日 `23:59:59`。 |
| `X-Quota-Weekly-Limit` | Integer | `5000000` | 当前 API Key 配置的**周配额总上限**（单位：Token）。`0` 表示不限周额度。 |
| `X-Quota-Weekly-Remaining` | Integer | `3410200` | 本周**剩余可用周配额**（单位：Token）。 |
| `X-Quota-Weekly-Reset` | Integer | `1729439999` | 本周配额重置的 Unix 时间戳（秒级），对齐为本周日 `23:59:59`。 |
| `X-Concurrency-Current` | Integer | `3` | 当前 API Key 正在网关中执行的**实时在途活跃长连接并发数**。 |
| `Retry-After` | Integer | `2` | 仅在触发 `429 Too Many Requests` (RPM 限流) 时返回，指示客户端等待多少秒后可重试。 |

### 2.3 Hop-by-Hop 协议头清洗与真实凭据屏蔽机制

当网关将请求中继转发至物理后端（vLLM / SGLang / TEI / OpenAI）时，在代理管道层执行严格的 Header 转换与清洗：

```mermaid
graph LR
    ClientHeader["入站客户端请求头<br>Authorization: Bearer sk-gate-xxx<br>x-request-id: <uuid><br>Connection: keep-alive"] --> Sanitizer["网关清洗与转换管道 (Header Sanitizer)"]
    
    subgraph SanitizerAction["清洗规则"]
        Sanitizer --> StripHop["1. 强制剥离 Hop-by-Hop 头<br>(Connection, Keep-Alive, Upgrade, Transfer-Encoding)"]
        Sanitizer --> StripAuth["2. 剔除客户端虚拟 API Key<br>Strip: Bearer sk-gate-xxx"]
        Sanitizer --> InjectAuth["3. 注入后端托管真实凭据<br>Inject: Authorization: Bearer <upstream_real_key>"]
        Sanitizer --> InjectTrace["4. 透传或注入 x-request-id<br>Inject: X-Forwarded-For: <client_ip>"]
    end
    
    SanitizerAction --> UpstreamHeader["出站上游后端请求头<br>(物理后端绝无可能探知客户端虚拟凭证)"]
```

- **Hop-by-hop 头剥离清单**：
  `connection`, `keep-alive`, `proxy-authenticate`, `proxy-authorization`, `te`, `trailers`, `transfer-encoding`, `upgrade`。
- **凭据彻底屏蔽**：
  外部客户端发出的 `sk-gate-*` 仅在网关内部作为鉴权凭据，转发至后端时被绝对替换为该后端实例配置的 `upstream_api_key`。即使外部用户抓包，也无法获悉底层物理服务器的私密 Token。

---

## 3. 多协议原生直通端点契约 (Multi-Protocol Passthrough Endpoints)

网关提供 6 大标准端点，全面覆盖文本聊天、Agentic 智能体交互、传统补全、向量嵌入、交叉重排与模型列表查询。

```text
POST /v1/chat/completions  --> OpenAI 标准对话补全 (JSON / SSE 流式)
POST /v1/responses         --> OpenAI 新一代 Agentic Responses API (Items / Modal)
POST /v1/completions       --> 传统文本补全 (Legacy Completion)
POST /v1/embeddings        --> 文本向量嵌入 (Batch Text Embeddings)
POST /v1/rerank            --> 交叉编码器重排序 (Cross-Encoder Document Rerank)
GET  /v1/models            --> 网关逻辑模型聚合列表
```

---

### 3.1 对话补全：`POST /v1/chat/completions`

OpenAI 标准对话补全端点，广泛用于 ChatGPT、Claude 兼容层、DeepSeek、Qwen、GLM 等主流模型调用。支持非流式 JSON 与 SSE 流式输出。

#### 1. 核心请求体参数定义 (Request Body)

| 字段名 | 类型 | 必填 | 默认值 | 说明与约束 |
| :--- | :---: | :---: | :---: | :--- |
| `model` | String | **是** | - | **逻辑模型名称**（如 `qwen-2.5-72b`、`deepseek-r1`）。网关据此匹配路由规则并分发至对应后端集群。 |
| `messages` | Array[Object] | **是** | - | 会话消息列表。每个 Object 包含 `role` (`system` \| `user` \| `assistant` \| `tool`) 与 `content` (String 或多模态 Array)。 |
| `stream` | Boolean | 否 | `false` | 是否开启 SSE 流式输出。为 `true` 时响应头为 `text/event-stream`，实时逐 chunk 透传。 |
| `temperature` | Float | 否 | `1.0` | 采样温度，通常介于 `[0.0, 2.0]`。 |
| `top_p` | Float | 否 | `1.0` | 核采样概率，介于 `[0.0, 1.0]`。 |
| `max_tokens` | Integer | 否 | null | 生成的最大 Token 数量限制。 |
| `stop` | String \| Array | 否 | null | 停止词序列。 |
| `stream_options` | Object | 否 | null | 流式输出配置。推荐设置 `{"include_usage": true}`，网关据此可在流尾部提取精确 Token 消耗。 |
| `tools` | Array[Object] | 否 | null | 模型可调用的 Function Calling 工具定义清单。 |
| `tool_choice` | String \| Object | 否 | null | 控制模型调用工具的行为模式。 |

#### 2. 非流式调用请求与响应示例 (`stream = false`)

**请求示例**：
```http
POST /v1/chat/completions HTTP/1.1
Host: gate.internal.corp:8080
Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01
Content-Type: application/json
x-request-id: 6a2c3d4e-5f6a-7b8c-9d0e-1f2a3b4c5d6e

{
  "model": "qwen-2.5-72b-instruct",
  "messages": [
    {"role": "system", "content": "You are a helpful coding assistant."},
    {"role": "user", "content": "Write a Rust hello world function."}
  ],
  "temperature": 0.7,
  "stream": false
}
```

**响应示例**：
```http
HTTP/1.1 200 OK
Content-Type: application/json; charset=utf-8
x-request-id: 6a2c3d4e-5f6a-7b8c-9d0e-1f2a3b4c5d6e
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 999880
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 4999880
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 1

{
  "id": "chatcmpl-87a3b4c5d6e7f8",
  "object": "chat.completion",
  "created": 1729296000,
  "model": "qwen-2.5-72b-instruct",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 24,
    "completion_tokens": 16,
    "total_tokens": 40
  }
}
```

#### 3. 流式调用请求与响应示例 (`stream = true`)

**请求示例**：
```http
POST /v1/chat/completions HTTP/1.1
Host: gate.internal.corp:8080
Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01
Content-Type: application/json
Accept: text/event-stream

{
  "model": "qwen-2.5-72b-instruct",
  "messages": [
    {"role": "user", "content": "Rust is fast"}
  ],
  "stream": true,
  "stream_options": {
    "include_usage": true
  }
}
```

**响应报文流式数据传输序列 (SSE Chunks)**：
```http
HTTP/1.1 200 OK
Content-Type: text/event-stream; charset=utf-8
Cache-Control: no-cache
Connection: keep-alive
x-request-id: 7b3d4e5f-6a7b-8c9d-0e1f-2a3b4c5d6e7f
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 999840
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 4999840
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 1

data: {"id":"chatcmpl-7b3d4e","object":"chat.completion.chunk","created":1729296001,"model":"qwen-2.5-72b-instruct","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-7b3d4e","object":"chat.completion.chunk","created":1729296001,"model":"qwen-2.5-72b-instruct","choices":[{"index":0,"delta":{"content":"Indeed"},"finish_reason":null}]}

data: {"id":"chatcmpl-7b3d4e","object":"chat.completion.chunk","created":1729296001,"model":"qwen-2.5-72b-instruct","choices":[{"index":0,"delta":{"content":", Rust offers memory safety without garbage collection."},"finish_reason":null}]}

data: {"id":"chatcmpl-7b3d4e","object":"chat.completion.chunk","created":1729296001,"model":"qwen-2.5-72b-instruct","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: {"id":"chatcmpl-7b3d4e","object":"chat.completion.chunk","created":1729296001,"model":"qwen-2.5-72b-instruct","choices":[],"usage":{"prompt_tokens":11,"completion_tokens":12,"total_tokens":23}}

data: [DONE]

```

---

### 3.2 智能体响应：`POST /v1/responses`

OpenAI 新一代面向 Agentic 复杂任务的 Responses API。采用结构化 items 输入与多模态流式交互协议。

#### 1. 核心请求体参数定义 (Request Body)

| 字段名 | 类型 | 必填 | 默认值 | 说明与约束 |
| :--- | :---: | :---: | :---: | :--- |
| `model` | String | **是** | - | **逻辑模型名称**（如 `gpt-4o`、`deepseek-v3`、`qwen-agent`）。 |
| `input` | String \| Array[Object] | **是** | - | 输入负载。支持单字符串提示词或结构化输入项列表（Message Items, Function Results 等）。 |
| `modalities` | Array[String] | 否 | `["text"]` | 生成模态配置，支持 `["text"]`、`["text", "audio"]`。 |
| `instructions` | String | 否 | null | 针对 Agent 行为的高优先级系统级引导指令。 |
| `tools` | Array[Object] | 否 | null | 挂载的 Agent 工具集定义。 |
| `temperature` | Float | 否 | `1.0` | 采样温度。 |
| `stream` | Boolean | 否 | `false` | 是否开启流式传输。 |

#### 2. 请求与响应示例

**请求示例**：
```http
POST /v1/responses HTTP/1.1
Host: gate.internal.corp:8080
Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01
Content-Type: application/json
x-request-id: 8c4e5f6a-7b8c-9d0e-1f2a-3b4c5d6e7f8a

{
  "model": "deepseek-v3",
  "instructions": "You are a code review agent. Identify potential bugs.",
  "input": "Review this function: pub fn divide(a: i32, b: i32) -> i32 { a / b }",
  "modalities": ["text"],
  "stream": false
}
```

**响应示例**：
```http
HTTP/1.1 200 OK
Content-Type: application/json; charset=utf-8
x-request-id: 8c4e5f6a-7b8c-9d0e-1f2a-3b4c5d6e7f8a
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 999760
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 4999760
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 1

{
  "id": "resp-9a8b7c6d5e4f3a",
  "object": "response",
  "created_at": 1729296002,
  "status": "completed",
  "model": "deepseek-v3",
  "output": [
    {
      "id": "item-1a2b3c4d",
      "type": "message",
      "status": "completed",
      "role": "assistant",
      "content": [
        {
          "type": "text",
          "text": "The function `divide` lacks a check for division by zero (`b == 0`), which will cause a panic at runtime. Consider returning `Option<i32>` or `Result<i32, DivisionByZeroError>`."
        }
      ]
    }
  ],
  "usage": {
    "total_tokens": 62,
    "input_tokens": 28,
    "output_tokens": 34
  }
}
```

---

### 3.3 传统文本补全：`POST /v1/completions`

标准 Legacy 补全端点，常用于特定代码补全（FIM / Fill-in-the-Middle）以及低开销直接文本生成。

#### 1. 核心请求体参数定义 (Request Body)

| 字段名 | 类型 | 必填 | 默认值 | 说明与约束 |
| :--- | :---: | :---: | :---: | :--- |
| `model` | String | **是** | - | **逻辑模型名称**（如 `qwen-2.5-coder-7b`）。 |
| `prompt` | String \| Array[String] | **是** | - | 待补全的提示词文本或批量文本列表。 |
| `max_tokens` | Integer | 否 | `16` | 补全的最大 Token 数。 |
| `temperature` | Float | 否 | `1.0` | 采样温度。 |
| `stream` | Boolean | 否 | `false` | 是否开启流式。 |
| `suffix` | String | 否 | null | 代码中缝插入补全时所用的代码后缀（FIM 场景）。 |

#### 2. 请求与响应示例

**请求示例**：
```http
POST /v1/completions HTTP/1.1
Host: gate.internal.corp:8080
Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01
Content-Type: application/json

{
  "model": "qwen-2.5-coder-7b",
  "prompt": "def calculate_fibonacci(n):\n    if n <= 1:\n        return n\n    return ",
  "max_tokens": 30,
  "temperature": 0.2
}
```

**响应示例**：
```http
HTTP/1.1 200 OK
Content-Type: application/json; charset=utf-8
x-request-id: 9d5f6a7b-8c9d-0e1f-2a3b-4c5d6e7f8a9b
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 999710
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 4999710
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 1

{
  "id": "cmpl-0b1c2d3e4f5a",
  "object": "text_completion",
  "created": 1729296003,
  "model": "qwen-2.5-coder-7b",
  "choices": [
    {
      "text": "calculate_fibonacci(n - 1) + calculate_fibonacci(n - 2)",
      "index": 0,
      "logprobs": null,
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 21,
    "completion_tokens": 19,
    "total_tokens": 40
  }
}
```

---

### 3.4 向量嵌入：`POST /v1/embeddings`

用于将文本转换为高维稠密浮点向量，支撑企业知识库 RAG、语义搜索与文档分类。通常直通至后端 TEI (Text Embeddings Inference) 或 vLLM Embedding 模型。

#### 1. 核心请求体参数定义 (Request Body)

| 字段名 | 类型 | 必填 | 默认值 | 说明与约束 |
| :--- | :---: | :---: | :---: | :--- |
| `model` | String | **是** | - | **逻辑模型名称**（如 `bge-m3`、`text-embedding-3-large`）。 |
| `input` | String \| Array[String] | **是** | - | 输入待向量化的单条字符串，或批量字符串数组。 |
| `dimensions` | Integer | 否 | null | 降维维度设置（仅支持 Matryoshka 特性的模型有效）。 |
| `encoding_format` | String | 否 | `float` | 向量编码格式。`float`（标准浮点数组）或 `base64`（二进制字节流）。 |

#### 2. 请求与响应示例

**请求示例**：
```http
POST /v1/embeddings HTTP/1.1
Host: gate.internal.corp:8080
Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01
Content-Type: application/json

{
  "model": "bge-m3",
  "input": [
    "Rust 语言具有极高的运行性能与内存安全性",
    "rs-gate 是轻量级单二进制企业大模型网关"
  ]
}
```

**响应示例**：
```http
HTTP/1.1 200 OK
Content-Type: application/json; charset=utf-8
x-request-id: ae6a7b8c-9d0e-1f2a-3b4c-5d6e7f8a9b0c
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 999675
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 4999675
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 1

{
  "object": "list",
  "data": [
    {
      "object": "embedding",
      "index": 0,
      "embedding": [0.01238, -0.04561, 0.08912, 0.00341, "..."]
    },
    {
      "object": "embedding",
      "index": 1,
      "embedding": [0.05612, 0.01189, -0.02341, 0.07823, "..."]
    }
  ],
  "model": "bge-m3",
  "usage": {
    "prompt_tokens": 28,
    "total_tokens": 28
  }
}
```

---

### 3.5 文档重排序：`POST /v1/rerank`

用于高级 RAG 检索管线中的精准二次重排序（Cross-Encoder Reranker），通过深度语义交互为 Query 与候选 Documents 之间计算相关性评分。

#### 1. 核心请求体参数定义 (Request Body)

| 字段名 | 类型 | 必填 | 默认值 | 说明与约束 |
| :--- | :---: | :---: | :---: | :--- |
| `model` | String | **是** | - | **逻辑模型名称**（如 `bge-reranker-large`）。 |
| `query` | String | **是** | - | 用户的原始查询或检索问题。 |
| `documents` | Array[String] | **是** | - | 候选文本块列表，通常为初步向量检索召回的前 20~50 条候选。 |
| `top_n` | Integer | 否 | 全部 | 仅返回重排后得分最高的前 N 个结果。若不传则返回全量重排结果。 |
| `return_documents`| Boolean | 否 | `false` | 是否在响应结果中原样回传对应的 document 内容文本。 |

#### 2. 请求与响应示例

**请求示例**：
```http
POST /v1/rerank HTTP/1.1
Host: gate.internal.corp:8080
Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01
Content-Type: application/json

{
  "model": "bge-reranker-large",
  "query": "rs-gate 怎么解决大模型并发倾斜问题？",
  "documents": [
    "文档A：rs-gate 采用单二进制交付，内置 SQLite 存储与 Web 控制台。",
    "文档B：rs-gate 采用基于加权最小活跃长连接（WLAC）无除法交叉乘法算法，防止长思考请求积压单节点。",
    "文档C：Python 具有丰富的 AI 生态库，但在高并发场景需要多进程协同。"
  ],
  "top_n": 2,
  "return_documents": true
}
```

**响应示例**：
```http
HTTP/1.1 200 OK
Content-Type: application/json; charset=utf-8
x-request-id: bf7b8c9d-0e1f-2a3b-4c5d-6e7f8a9b0c1d
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 999610
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 4999610
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 1

{
  "id": "rerank-1f2a3b4c5d",
  "model": "bge-reranker-large",
  "results": [
    {
      "index": 1,
      "relevance_score": 0.9682,
      "document": {
        "text": "文档B：rs-gate 采用基于加权最小活跃长连接（WLAC）无除法交叉乘法算法，防止长思考请求积压单节点。"
      }
    },
    {
      "index": 0,
      "relevance_score": 0.3415,
      "document": {
        "text": "文档A：rs-gate 采用单二进制交付，内置 SQLite 存储与 Web 控制台。"
      }
    }
  ],
  "usage": {
    "total_tokens": 65
  }
}
```

---

### 3.6 逻辑模型聚合列表：`GET /v1/models`

查询当前网关中所有处于启用状态（`is_active = 1`）且当前 API Key 具备访问权限的逻辑模型列表。标准 OpenAI 格式，供 WebUI、Chatbox 或自动化客户端下拉选择。

#### 1. 请求规范 (Request)
- **Method**：`GET`
- **Path**：`/v1/models`
- **Header**：`Authorization: Bearer sk-gate-...` (必填)

#### 2. 请求与响应示例

**请求示例**：
```http
GET /v1/models HTTP/1.1
Host: gate.internal.corp:8080
Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01
```

**响应示例**：
```http
HTTP/1.1 200 OK
Content-Type: application/json; charset=utf-8
x-request-id: d0e1f2a3-b4c5-6e7f-8a9b-0c1d2e3f4a5b
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 999610
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 4999610
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 1

{
  "object": "list",
  "data": [
    {
      "id": "qwen-2.5-72b-instruct",
      "object": "model",
      "created": 1729296000,
      "owned_by": "rs-gate",
      "permission": [],
      "root": "qwen-2.5-72b-instruct",
      "parent": null
    },
    {
      "id": "deepseek-r1",
      "object": "model",
      "created": 1729296000,
      "owned_by": "rs-gate",
      "permission": [],
      "root": "deepseek-r1",
      "parent": null
    },
    {
      "id": "bge-m3",
      "object": "model",
      "created": 1729296000,
      "owned_by": "rs-gate",
      "permission": [],
      "root": "bge-m3",
      "parent": null
    },
    {
      "id": "bge-reranker-large",
      "object": "model",
      "created": 1729296000,
      "owned_by": "rs-gate",
      "permission": [],
      "root": "bge-reranker-large",
      "parent": null
    }
  ]
}
```

---

## 4. SSE 零缓冲流式中继与 Usage 增量计量规范 (SSE Streaming & Usage Protocol)

流式传输是网关承载用户体验的关键核心。为兼顾**极低首字透传延迟 (TTFT Overhead $\le 1\text{ms}$)** 与**精准 Token 审计与配额扣减**，rs-gate 制定以下严格流式中继与解析规范。

### 4.1 SSE 帧传输格式规范

流式响应严格遵循 W3C Server-Sent Events 标准与 OpenAI 协议扩展：
1. **帧格式规范**：每个有效数据帧必须以 `data: ` 开头，以连续两个换行符 `\n\n` 结尾：
   ```text
   data: <JSON_PAYLOAD>\n\n
   ```
2. **增量内容帧 (Delta Content)**：
   ```json
   data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1729296000,"model":"qwen-2.5-72b","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}
   ```
3. **完成原因帧 (Finish Reason)**：
   ```json
   data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1729296000,"model":"qwen-2.5-72b","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}
   ```
4. **尾部统计帧 (Usage Chunk)**：
   在 `stream_options.include_usage: true` 下，完成原因帧后紧跟一个携带 `usage` 对象的帧（此时 `choices` 为空数组）：
   ```json
   data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1729296000,"model":"qwen-2.5-72b","choices":[],"usage":{"prompt_tokens":25,"completion_tokens":48,"total_tokens":73}}
   ```
5. **传输终止帧 (DONE Indicator)**：
   流式传输的最后一个物理帧必须为固定的结束标记：
   ```text
   data: [DONE]\n\n
   ```

### 4.2 零缓冲管道与增量 Usage 快速提取流程

```mermaid
sequenceDiagram
    autonumber
    actor Client as 客户端
    participant Gate as rs-gate 零缓冲流式管道
    participant Extractor as 靶向增量 Usage 提取器
    participant Upstream as 后端推理引擎 (vLLM)
    participant Worker as 异步审计 Worker

    Client ->> Gate: POST /v1/chat/completions (stream=true)
    Gate ->> Upstream: 原生直通转发 Body
    Upstream -->> Gate: 建立 HTTP/1.1 长连接，持续推送 Bytes Chunks
    
    loop 逐 Chunk 透传循环
        Gate ->> Client: 立即透传 Chunk (零额外堆分配，TTFT Overhead <= 1ms)
        Gate ->> Extractor: 纳秒级子串预检 (b"\"usage\":")
        alt 未匹配到 usage 子串
            Extractor -->> Gate: 纳秒级跳过 (零 JSON 反序列化开销)
        else 匹配到 usage 关键字
            Extractor ->> Extractor: 靶向解析 prompt_tokens / completion_tokens
            Extractor -->> Gate: 缓存解析所得 Usage 元数据
        end
    end

    Upstream -->> Gate: data: [DONE]\n\n (上游流结束)
    Gate ->> Client: 透传 data: [DONE]\n\n 并关闭下游响应
    Gate ->> Worker: 组装并投递 AuditEvent (携带精确 Usage 与 TTFT)
```

### 4.3 客户端 Abort 级联中断时序规范

当客户端用户在前端点击“停止回答”或直接关闭浏览器标签页时，下游 TCP 连接断开：
1. **连接断开捕获**：Axum / Hyper 底层立即感知客户端连接重置（`ConnectionReset` / `BrokenPipe`）；
2. **生成器终止 (Stream Drop)**：Axum 流生成器（`async_stream`）被即刻 Drop，直接释放绑定的 `ActiveConnectionGuard`；
3. **上游长连接中止**：与后端 vLLM 建立的 Reqwest Response Body 流被同步丢弃（Drop），底层触发向后端发送 `TCP RST`；
4. **GPU 显存安全回收**：后端推理引擎捕获客户端连接断开，立即终止下一个 Token 的自回归生成迭代，并释放显存中的 KV Cache 资源。

### 4.4 Usage 缺失兜底机制 (Fallback Usage Estimation)

若客户端未传递 `stream_options.include_usage: true`，或旧版本后端未返回尾部 Usage 帧，网关执行两级安全策略：
1. **审计元数据标记**：网关在生成 `AuditEvent` 时记录 `is_usage_missing = 1`；
2. **轻量本地估算**：网关内置轻量级分词器（基于 `tiktoken-rs`），在后台任务中针对请求 `prompt` 与客户端实际接收的所有增量内容文本进行快速本地分词统计，补偿计算 `prompt_tokens` 与 `completion_tokens`，误差控制在 $\pm 5\%$ 以内，确保日/周配额扣减准确性。

---

## 5. 统一错误协议契约与状态码映射矩阵 (Unified Error Contract)

### 5.1 统一错误响应数据结构 (Standard Schema)

为保证上层 SDK（如官方 OpenAI Python/Node 库）能够正常解析异常并抛出对应的高级 Exception（如 `openai.RateLimitError`、`openai.AuthenticationError`），rs-gate 拦截的所有非 2xx 响应严格遵循以下 JSON 契约：

```json
{
  "error": {
    "message": "Human-readable explanation of why the request failed.",
    "type": "gateway_error",
    "param": null,
    "code": "specific_error_code"
  }
}
```

| 字段名 | 类型 | 说明与规范 |
| :--- | :---: | :--- |
| `message` | String | 人类可读的错误明细说明，清晰阐明拒绝原因、超限类型或排查建议。 |
| `type` | String | 错误分类类别。统一归类为：`invalid_request_error`、`authentication_error`、`permission_error`、`rate_limit_error`、`insufficient_quota`、`gateway_error`、`upstream_error`。 |
| `param` | String \| null | 触发错误的具体入参字段名称（如 `model`、`stream` 等）。若为全局系统错误则固定为 `null`。 |
| `code` | String \| Integer | 机器可读的精确错误码（如 `daily_quota_exceeded`、`no_healthy_backend`）。 |

### 5.2 状态码与错误映射矩阵表 (Status Code Matrix)

| HTTP 状态码 | 错误分类 (`type`) | 错误编码 (`code`) | 触发场景说明 | 处理对策与建议 |
| :---: | :--- | :--- | :--- | :--- |
| **`400`** | `invalid_request_error` | `invalid_json_body`<br>`missing_required_model` | 请求体非有效 JSON 格式；或未指定必填的 `model` 字段。 | 检查客户端请求体格式与字段。 |
| **`401`** | `authentication_error` | `invalid_api_key`<br>`api_key_expired`<br>`api_key_disabled` | 缺少 `Authorization` 请求头；虚拟 Key 在网关数据库中不存在；Key 已过期；或已被管理员禁用。 | 在自服务门户重新申领有效 API Key。 |
| **`403`** | `permission_error` | `ip_forbidden`<br>`model_access_denied` | 客户端来源 IP 未落在配置的 CIDR 白名单内；或当前 Key 的模型白名单中未授权访问该逻辑模型。 | 联系管理员开通网络白名单或授予该模型访问权限。 |
| **`404`** | `invalid_request_error` | `model_not_found` | 请求的逻辑模型在网关中不存在，或未配置任何后端映射。 | 调用 `GET /v1/models` 查看可用模型列表。 |
| **`429`** | `rate_limit_error` | `rpm_limit_exceeded` | 触发单 Key 每分钟请求数（RPM）无锁令牌桶限流上限。响应头包含 `Retry-After`。 | 客户端按 `Retry-After` 等待后重试，或联系管理员调高 RPM。 |
| **`429`** | `rate_limit_error` | `concurrency_limit_exceeded` | 当前 Key 正在执行的在途长连接并发数达到 `max_concurrency` 门限。 | 降低客户端并发调用数，排队消费。 |
| **`429`** | `insufficient_quota` | `daily_quota_exceeded` | 今日已消耗 Token 达到配置的 `daily_quota` 上限。 | 等待明日 00:00:00 自动重置，或联系管理员手动重置。 |
| **`429`** | `insufficient_quota` | `weekly_quota_exceeded` | 本周已消耗 Token 达到配置的 `weekly_quota` 上限。 | 等待下周一 00:00:00 自动重置，或申请扩容周预算。 |
| **`429`** | `insufficient_quota` | `insufficient_quota_reserve` | 流式请求准入时，当前剩余配额不足以满足最低预留门限（默认 500 Token）。 | 防止巨型生成透支，请及时充值或重置配额。 |
| **`502`** | `upstream_error` | `bad_gateway` | 网关尝试连接目标物理后端失败（TCP Connection Refused / DNS 解析失败）。 | 检查底层后端实例存活状态与防火墙配置。 |
| **`503`** | `gateway_error` | `no_healthy_backend` | 该逻辑模型下挂载的所有物理后端实例均处于异常或熔断状态（Circuit Broken）。 | 查看控制台后端健康看板，等待后端恢复或介入运维。 |
| **`504`** | `upstream_error` | `gateway_timeout` | 后端首字响应超时（TTFT 超时），或流式中继传输过程中单 Chunk 读超时。 | 优化底层推理引擎 batching 策略或扩大超时阈值。 |

### 5.3 典型错误场景响应报文范例

#### 范例 1：401 Unauthorized (API Key 无效)
```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json; charset=utf-8
x-request-id: e1f2a3b4-c5d6-7e8f-9a0b-1c2d3e4f5a6b

{
  "error": {
    "message": "Incorrect API key provided: sk-gate-invalid***. You can manage your API keys in the rs-gate self-service portal.",
    "type": "authentication_error",
    "param": null,
    "code": "invalid_api_key"
  }
}
```

#### 范例 2：403 Forbidden (CIDR 网段拦截)
```http
HTTP/1.1 403 Forbidden
Content-Type: application/json; charset=utf-8
x-request-id: f2a3b4c5-d6e7-8f9a-0b1c-2d3e4f5a6b7c

{
  "error": {
    "message": "Access denied: Client IP '198.51.100.24' is not authorized by the gateway CIDR whitelist policy.",
    "type": "permission_error",
    "param": null,
    "code": "ip_forbidden"
  }
}
```

#### 范例 3：429 Too Many Requests (日配额耗尽硬阻断)
```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json; charset=utf-8
x-request-id: 0a1b2c3d-4e5f-6a7b-8c9d-0e1f2a3b4c5d
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 0
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 2150000
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 0

{
  "error": {
    "message": "Daily token quota exceeded (1,000,000 / 1,000,000 tokens used). Quota resets at 2026-10-19T23:59:59+08:00 (Unix: 1729353599).",
    "type": "insufficient_quota",
    "param": null,
    "code": "daily_quota_exceeded"
  }
}
```

#### 范例 4：429 Too Many Requests (RPM 令牌桶限流)
```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json; charset=utf-8
Retry-After: 3
x-request-id: 1b2c3d4e-5f6a-7b8c-9d0e-1f2a3b4c5d6e
X-Quota-Daily-Limit: 1000000
X-Quota-Daily-Remaining: 852100
X-Quota-Daily-Reset: 1729353599
X-Quota-Weekly-Limit: 5000000
X-Quota-Weekly-Remaining: 3852100
X-Quota-Weekly-Reset: 1729439999
X-Concurrency-Current: 4

{
  "error": {
    "message": "Rate limit exceeded (Requests Per Minute limit reached for this API key). Please retry after 3 seconds.",
    "type": "rate_limit_error",
    "param": null,
    "code": "rpm_limit_exceeded"
  }
}
```

#### 范例 5：503 Service Unavailable (无可用健康后端)
```http
HTTP/1.1 503 Service Unavailable
Content-Type: application/json; charset=utf-8
x-request-id: 2c3d4e5f-6a7b-8c9d-0e1f-2a3b4c5d6e7f

{
  "error": {
    "message": "No healthy upstream backend servers available for model 'deepseek-r1'. All candidate instances are currently degraded or circuit-broken.",
    "type": "gateway_error",
    "param": null,
    "code": "no_healthy_backend"
  }
}
```

---

## 6. 客户端 SDK 接入与生态兼容性指南 (SDK Compatibility & Usage)

### 6.1 Python 接入示例 (官方 `openai` v1.x SDK)

```python
import os
from openai import OpenAI

# 接入 rs-gate 网关：配置 base_url 与虚拟 key
client = OpenAI(
    base_url="http://gate.internal.corp:8080/v1",
    api_key="sk-gate-9f8e7d6c5b4a312012345678abcdef01",
)

# 1. 非流式调用
response = client.chat.completions.create(
    model="qwen-2.5-72b-instruct",
    messages=[
        {"role": "system", "content": "You are a professional assistant."},
        {"role": "user", "content": "Explain Rust ownership in one sentence."}
    ],
    temperature=0.7,
)
print("Assistant:", response.choices[0].message.content)
print("Tokens used:", response.usage.total_tokens)

# 2. SSE 流式调用 (推荐携带 include_usage)
stream = client.chat.completions.create(
    model="qwen-2.5-72b-instruct",
    messages=[{"role": "user", "content": "Write a quick Rust example."}],
    stream=True,
    stream_options={"include_usage": True},
)

for chunk in stream:
    if chunk.choices and chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)
    if chunk.usage:
        print(f"\n[Stream Finished] Total tokens: {chunk.usage.total_tokens}")
```

### 6.2 Node.js / TypeScript 接入示例 (`openai` npm package)

```typescript
import OpenAI from "openai";

const openai = new OpenAI({
  baseURL: "http://gate.internal.corp:8080/v1",
  apiKey: "sk-gate-9f8e7d6c5b4a312012345678abcdef01",
});

async function main() {
  const stream = await openai.chat.completions.create({
    model: "deepseek-v3",
    messages: [{ role: "user", content: "Hello from TypeScript!" }],
    stream: true,
    stream_options: { include_usage: true },
  });

  for await (const chunk of stream) {
    process.stdout.write(chunk.choices[0]?.delta?.content || "");
    if (chunk.usage) {
      console.log(`\nUsage: ${chunk.usage.total_tokens} tokens`);
    }
  }
}

main().catch(console.error);
```

### 6.3 cURL 命令行测试与流式调试 (CLI)

```bash
# 1. 验证模型列表
curl -s http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01"

# 2. 验证流式 SSE 逐 chunk 打印并查看完整响应头
curl -N -i http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-gate-9f8e7d6c5b4a312012345678abcdef01" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen-2.5-72b-instruct",
    "messages": [{"role": "user", "content": "Hi"}],
    "stream": true,
    "stream_options": {"include_usage": true}
  }'
```

### 6.4 LangChain 接入指引 (Python)

```python
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    base_url="http://gate.internal.corp:8080/v1",
    api_key="sk-gate-9f8e7d6c5b4a312012345678abcdef01",
    model="qwen-2.5-72b-instruct",
    temperature=0.7,
)

response = llm.invoke("What is the core feature of rs-gate?")
print(response.content)
```

---

## 7. 契约验收标准与后续规划 (Verification Checklist & Next Steps)

### 7.1 代理协议契约验收标准 (AC Checklist)

- [x] **端点全覆盖**：精准定义 `/v1/chat/completions`、`/v1/responses`、`/v1/completions`、`/v1/embeddings`、`/v1/rerank` 与 `/v1/models` 共 6 个端点的请求字段与响应契约。
- [x] **协议头完整规范**：定义入站请求头（`Authorization`、`x-request-id` 等）与出站响应头（`X-Quota-*`、`X-Concurrency-*`、`Retry-After` 等），明确 Hop-by-hop 清洗规则。
- [x] **SSE 流式与 Usage 契约**：明确逐 Chunk 透传结构、`include_usage` 尾部帧结构、`[DONE]` 终止帧，以及客户端 Abort 级联中断与 Usage 缺失本地估算方案。
- [x] **统一错误契约**：统一 OpenAI 兼容错误结构，提供包含 400、401、403、404、429、502、503、504 的完整状态码映射矩阵与典型 JSON 报文示例。
- [x] **SDK 兼容性验证**：提供 Python、TypeScript、cURL 与 LangChain 的完整接入代码范例。

### 7.2 下一步工作计划

根据阶段三编制节奏：
👉 **执行阶段三第 2 步**：编制 [`docs/03-api/02-admin-api.md`](file:///home/fugui/rusts/rs-gate/docs/03-api/02-admin-api.md)（【控制台契约】控制台与自服务门户 REST API 契约规范，涵盖统一登录 Auth、普通用户自服务门户 API、管理员全局管控 API 及系统运维监控探针）。
