# Mini-Cache 项目完全指南
> **定位**：一个月可完成的 Rust 高性能内存缓存服务器 + 前端监控面板
> **核心优势**：手写 TCP 异步服务 + 前端可视化 + 架构书概念包装

---

## 一、项目概述

实现一个基于 Tokio 的异步内存缓存服务器，支持简单的类 Redis 文本协议，并附带一个前端实时监控 Dashboard。通过这个项目，你可以展示：

- **Rust 系统编程能力**：Ownership、Lifetime、Concurrency、Async/Await
- **网络编程能力**：TCP 协议解析、Socket 编程、高并发连接处理
- **架构设计意识**：缓存淘汰策略、压力测试、水平扩展思路（引用《复杂架构设计》）
- **全栈交付能力**：前端实时监控界面（保留你的前端优势）

---

## 二、技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 后端 | Rust + Tokio | 异步 TCP 服务器，支持 1000+ 并发 |
| 后端 | DashMap / RwLock | 线程安全的内存存储（替代纯 HashMap） |
| 协议 | 类 Redis 文本协议 | `SET key value EX 60` / `GET key` / `DEL key` / `STATS` |
| 前端 | Next.js + TypeScript | 监控面板 |
| 前端 | ECharts / Ant Design | 实时 QPS 曲线、内存占用、延迟分布 |
| 压测 | redis-benchmark / 自定义 client | 验证性能指标 |

---

## 三、架构设计（对应架构书概念）

```text
┌─────────────────────────────────────────────────────────┐
│  前端监控层 (Next.js)                                      │
│  - 命令执行器 (模拟 Redis CLI)                              │
│  - 实时监控面板 (WebSocket/HTTP 轮询)                       │
│  - 压测结果可视化                                          │
└──────────────┬──────────────────────────────────────────┘
               │ HTTP / WebSocket
┌──────────────▼──────────────────────────────────────────┐
│  接入层 (Tokio TCP Server)                                 │
│  - 监听端口 (如 6379)                                      │
│  - 每连接一个 Task (Tokio::spawn)                          │
│  - 协议解析器 (Parser)                                     │
└──────────────┬──────────────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────────────┐
│  逻辑层 (Command Handler)                                  │
│  - SET: 写入内存 + 注册 TTL                                │
│  - GET: 读取内存 + 惰性删除检查                             │
│  - STATS: 聚合 QPS/内存/键数量                              │
└──────────────┬──────────────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────────────┐
│  存储层 (In-Memory Store)                                  │
│  - HashMap<String, CacheEntry>                             │
│  - TTL 过期队列 (最小堆 / 定期扫描)                         │
│  - 统计计数器 (AtomicU64)                                  │
└─────────────────────────────────────────────────────────┘
```

### 架构书概念植入点（写进 ARCHITECTURE.md）
1. **动态/静态资源分离**（架构书 Ch 2.2）：前端静态页面与后端动态 API 分离部署
2. **协程模型**（架构书 Ch 6.2.4）：Rust async/await 实现轻量级并发，对比 Go 协程
3. **缓存设计**（架构书 Ch 11）：TTL 过期策略类比 Redis 的"惰性删除 + 定期删除"
4. **压力测试**（架构书 Ch 4）：标准化压测流程，记录 QPS、P99 延迟、内存占用
5. **水平扩展思路**（架构书 Ch 5.1）：未来可通过一致性哈希扩展为集群

---

## 四、模块设计（Rust 代码结构）

```
mini-cache/
├── Cargo.toml
├── README.md
├── ARCHITECTURE.md          ← 架构书概念输出地
├── BENCHMARK.md             ← 压测报告
├── src/
│   ├── main.rs              ← 入口：启动 TCP Server 和 HTTP API
│   ├── server.rs            ← TCP 服务器核心（Tokio）
│   ├── protocol.rs          ← 协议解析（SET/GET/DEL/STATS）
│   ├── store.rs             ← 内存存储 + TTL 管理
│   ├── stats.rs             ← 性能统计（QPS、延迟 histogram）
│   └── api.rs               ← HTTP API（给前端提供 JSON 数据）
├── frontend/                ← Next.js 前端（独立目录）
│   ├── app/
│   ├── components/
│   │   ├── Dashboard.tsx    ← 监控面板
│   │   └── Terminal.tsx     ← 命令行界面
│   └── package.json
└── benches/
    └── cache_bench.rs       ← Criterion 基准测试
```

### 核心模块说明

#### 1. `protocol.rs` — 协议解析
支持简单文本协议（空格分隔）：
```
SET mykey myvalue EX 60

GET mykey

DEL mykey

STATS

```

#### 2. `store.rs` — 内存存储
```rust
struct CacheEntry {
    value: String,
    expires_at: Option<Instant>,
}

struct CacheStore {
    data: DashMap<String, CacheEntry>,  // 线程安全 HashMap
    ttl_queue: Mutex<BinaryHeap<(Instant, String)>>, // TTL 最小堆
}
```
- **惰性删除**：GET 时检查 `expires_at`，过期则删除
- **定期扫描**：后台 Task 每 100ms 扫描堆顶，清理过期键

#### 3. `server.rs` — TCP 服务器
```rust
async fn run_server(addr: &str) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(addr).await?;
    let store = Arc::new(CacheStore::new());

    loop {
        let (socket, _) = listener.accept().await?;
        let store = Arc::clone(&store);
        tokio::spawn(handle_connection(socket, store)); // 每连接一个协程
    }
}
```

#### 4. `stats.rs` — 统计
使用 `AtomicU64` 记录：
- `total_commands`: 总命令数
- `total_connections`: 总连接数
- `latency_histogram`: 延迟分布（简化版分桶）

#### 5. `api.rs` — HTTP API（Axum/Actix-web）
提供 REST 接口给前端：
- `GET /api/stats` -> JSON 格式的实时统计
- `POST /api/execute` -> 代理执行命令并返回结果

---

## 五、前端设计（保留你的优势）

### 页面1：实时监控面板（Dashboard.tsx）
- **顶部卡片**：当前 QPS、总连接数、内存占用（估算）、键数量
- **中部图表**：
  - 折线图：最近 60 秒 QPS 曲线（HTTP 轮询，1秒间隔）
  - 柱状图：延迟分布（<1ms, 1-5ms, 5-10ms, >10ms）
- **底部表格**：最近 50 条命令日志（时间、命令、耗时）

### 页面2：命令行界面（Terminal.tsx）
- 输入框输入命令（如 `SET foo bar EX 10`）
- 点击发送，通过 HTTP POST 到后端
- 显示返回结果（类似 Redis CLI 体验）

### 技术选型（Vite + React + TypeScript）
- Vite 6（构建工具，极速 HMR，无需 SWC）
- React 18 + TypeScript
- Ant Design / Tailwind CSS（UI 框架）
- ECharts 或 Recharts（图表库）
- SWR 或原生 fetch（轮询）

---

## 六、四周开发时间线

### Week 1：骨架 + 协议 + 单线程存储
- **TRPL 学习**：Ch 1-6（重点 Ch 4 Ownership）+ Ch 8 Collections
- **项目目标**：
  - 搭好 Cargo 项目结构
  - 实现 `protocol.rs`：能解析 `SET key value` / `GET key`
  - 实现 `store.rs`：单线程 HashMap 存储
  - 实现 `server.rs`：同步/简单异步 Echo Server
- **架构书阅读**：Ch 1（高并发入门）+ Ch 2（架构套路），产出 `ARCHITECTURE.md` 初稿
- **刷题**：每天 1h 力扣（HashMap、链表、栈队列）

### Week 2：异步化 + TTL + 并发安全
- **TRPL 学习**：Ch 9（Error Handling）+ Ch 10（Generics/Traits/Lifetimes）+ Ch 16（Concurrency）
- **项目目标**：
  - 接入 `tokio`，改为全异步 TCP Server
  - 存储改为 `DashMap`（或 `RwLock<HashMap>`），支持并发读写
  - 实现 TTL 过期（最小堆 + 惰性删除）
  - 实现 `STATS` 命令和基础统计
- **架构书阅读**：Ch 6（协程/线程模型）+ Ch 7（Web Server/epoll），在文档中对比 Rust async vs 多线程
- **刷题**：每天 1h 力扣（二叉树、递归、双指针）

### Week 3：HTTP API + 前端 + 压测准备
- **TRPL 学习**：Ch 17（Async Programming）查缺补漏 + Ch 13（Iterators）
- **项目目标**：
  - 用 `axum` 或 `actix-web` 实现 HTTP API
  - 前端 Dashboard 开发（Next.js + ECharts）
  - 对接后端，实现实时数据展示
  - 写 `benches/cache_bench.rs`（Criterion）
- **架构书阅读**：Ch 11（缓存设计），在文档中详细写 TTL 策略设计理由
- **刷题**：每天 1h 力扣（滑动窗口、堆、TopK）+ 0.5h 前端八股（React 原理、浏览器缓存）

### Week 4：压测 + 文档 + 包装 + 公开
- **TRPL 学习**：复习 Ch 4（Ownership 面试核心）+ Ch 10（Lifetime 面试核心）
- **项目目标**：
  - 用 `redis-benchmark` 或自写 client 压测，记录数据
  - 完善 `README.md`（放架构图、截图、演示链接）
  - 完善 `ARCHITECTURE.md`（植入架构书概念）
  - 完善 `BENCHMARK.md`（压测方法论、结果、瓶颈分析）
  - GitHub 仓库从私有转为公开
- **架构书阅读**：Ch 4（压力测试），产出标准化压测报告
- **刷题**：每天 1h 力扣（模拟面试，随机抽题）+ 1h 前端八股/简历准备

---

## 七、压测方案（BENCHMARK.md 模板）

### 工具
- 方案 A：`redis-benchmark -p 6379 -t set,get -n 100000 -c 1000`
- 方案 B：自写 Rust 压测客户端（用 `tokio` 并发 1000 个 Task）

### 指标
| 指标 | 说明 | 目标值（参考） |
|------|------|--------------|
| QPS | 每秒处理命令数 | > 50k（单机） |
| P50 延迟 | 中位数延迟 | < 1ms |
| P99 延迟 | 99 分位延迟 | < 5ms |
| 内存占用 | 10万键内存占用 | 记录基线 |
| 并发连接数 | 同时保持的连接 | 1000+ |

### 瓶颈分析（面试话术）
> "当前单实例使用 `DashMap` 做全局锁，当 QPS 超过 10万 时锁竞争会成为瓶颈。
> 参考架构书 Ch 5.1 的负载均衡思想，未来可以通过**一致性哈希**将键空间分片到多个实例，
> 每个实例拥有独立的 `DashMap`，实现无锁水平扩展。"

---

## 八、面试话术（提前背熟）

### 项目介绍（30秒版本）
> "我开发了一个 Mini-Cache，基于 Rust + Tokio 实现了一个异步 TCP 缓存服务器，支持类 Redis 协议和 TTL 过期策略。
> 为了展示全栈能力，我用 Next.js 开发了一个实时监控面板，可以实时看到 QPS、延迟分布和内存占用。
> 在设计上，我参考了《复杂架构设计》中的缓存设计思想和压力测试方法论，
> 使用惰性删除 + 定期扫描的混合策略管理 TTL，并通过压测验证了单实例 5万+ QPS 的性能。"

### 为什么用 Rust？
> "Rust 的 Ownership 和 Lifetime 机制让我在编译期就消除了内存泄漏和数据竞争的风险。
> 对于缓存这种长生命周期的服务，零成本抽象意味着我能用高级语言特性写出媲美 C++ 的性能，
> 同时保持代码的可维护性。Tokio 的协程模型（async/await）让我可以用极低的资源成本处理上万并发连接。"

### 架构设计亮点
> "我刻意将协议解析、存储、统计三层解耦。存储层目前使用 `DashMap` 保证线程安全，
> 但参考架构书中'软件定义计算'的思想，存储引擎是可替换的——未来可以接入 RocksDB 做持久化，
> 或者通过分片实现分布式存储，而上层协议完全不需要改动。"

### 缓存策略（通用）
> "TTL 管理我采用了**惰性删除 + 定期扫描**的混合策略：
> - 读取时检查过期时间，过期立即删除（保证不返回脏数据）
> - 后台 Task 每 100ms 扫描 TTL 堆顶，批量清理（防止内存泄漏）
> 这对应架构书中 Ch 11 提到的'缓存更新与淘汰策略'，在一致性和性能之间做了权衡。"

---

## 九、GitHub 公开检查清单

- [ ] 提交历史：至少 20 次 commit，分布均匀（不要一天提交完）
- [ ] Commit message：使用英文，简洁专业（如 `feat: add TTL expiration heap`）
- [ ] README：包含项目截图、架构图、演示链接、快速启动命令
- [ ] ARCHITECTURE.md：包含架构书概念引用（Ch 2, Ch 6, Ch 11, Ch 4）
- [ ] BENCHMARK.md：包含压测环境、方法、结果、瓶颈分析
- [ ] 前端：部署到 Vercel，提供在线演示链接（面试官会点）
- [ ] 后端：提供 Docker 一键启动（可选，加分项）
- [ ] 代码注释：关键函数（如 `Store::get` 的 TTL 检查）要有中文或英文注释

---

## 十、每日时间分配建议（含刷题/八股）

| 时间段 | 内容 | 时长 |
|--------|------|------|
| 上午 | TRPL 精读 + 知识点截图/笔记 | 3h |
| 下午 | 项目编码（Rust 后端） | 3h |
| 傍晚 | 力扣刷题（保持算法手感） | 1-1.5h |
| 晚上 | 架构书阅读 + 文档写作 | 0.5-1h |
| 碎片时间 | 前端八股（React/Next.js/浏览器） | 0.5h |

**总计**：8-10h/天

> **刷题重点**：力扣热题 100 / 剑指 Offer
> - 必须掌握：哈希表（对应缓存）、链表（LRU）、堆（TopK）、二叉树（递归基础）
> - 选做：多线程/并发题（如 1114 按序打印，了解并发概念）
> 
> **前端八股重点**：保留前端面试选择
> - React 渲染原理、Hooks 闭包陷阱、Next.js SSR/SSG
> - 浏览器：事件循环、缓存策略、HTTP/2
