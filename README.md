# rs-gate

> 高性能、轻量级、单二进制交付的企业级内部大模型网关 (Enterprise LLM Gateway in Rust)

`rs-gate` 是一个面向企业内部大模型调用场景的统一接入网关，具备 OpenAI 兼容代理、多租户 API Key 鉴权、基于令牌桶的并发与频次流控、SSE 低延迟流式中继与异步审计计量能力，并内嵌管理控制台前端，实现无外部依赖的单二进制交付。

本项目严格遵循 **规范驱动开发 (Spec-Driven Development, SDD)** 模式：
**「文档规范先行 -> 架构设计定稿 -> 任务拆解与验收清单 -> 分阶段实施与验证」**

---

## 目录结构 (Repository Layout)

```text
rs-gate/
├── docs/            # 规范文档库 (SDD 规范、特性矩阵、架构设计、接口契约与 ADR)
├── src/             # 后端核心源码 (Rust 网关核心，阶段二设计定稿后逐步实施)
├── web/             # 前端控制台源码 (管理后台界面，产物内嵌至单二进制)
├── migrations/      # 数据库版本迁移 (声明式 SQLite DDL)
└── tests/           # 集成与回归测试 (端到端验证、流式代理测试与压测脚本)
```

完整文档规范与分层索引请见 [docs/README.md](docs/README.md)。

---

## SDD 开发生命周期 (Phases)

- [x] **阶段一：特性清单与边界明确** ([`docs/01-features/01-feature-matrix.md`](file:///home/fugui/rusts/rs-gate/docs/01-features/01-feature-matrix.md))
- [ ] **阶段二：系统架构设计文档**
  - [x] [01-system-architecture.md](file:///home/fugui/rusts/rs-gate/docs/02-design/01-system-architecture.md) (【总览篇】系统总体架构设计)
  - [x] [02-storage-and-models.md](file:///home/fugui/rusts/rs-gate/docs/02-design/02-storage-and-models.md) (【存储篇】存储模型与迁移)
  - [x] [03-routing-and-proxy.md](file:///home/fugui/rusts/rs-gate/docs/02-design/03-routing-and-proxy.md) (【代理篇】调度与流式中继)
  - [ ] 04-limiting-and-audit.md (【流控与审计篇】安全与审计)
- [ ] **阶段三：接口契约与任务分解** (`docs/03-api/01-gateway-api.md`, `docs/04-tasks/01-tasks.md`)
- [ ] **阶段四：工程实施与交付规范** (代码实现、静态检查、单元/集成测试与发布)

---

## 开源协议 (License)

Apache-2.0
