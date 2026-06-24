# Mini-Cache 架构设计文档（Week 3 更新）

> **版本**：v0.3.0 — Week 3 HTTP API + 前端监控阶段
> **目标**：记录架构设计思路，为后续迭代提供扩展蓝图。

---

## 1. 系统分层架构

```text
┌─────────────────────────────────────────┐
│  前端监控层 (Next.js)                    │
│  - 实时监控面板 (Dashboard)              │
│  - 命令行模拟器 (Terminal)               │
│  - HTTP 轮询获取统计数据                 │
└──────────────┬──────────────────────────┘
               │ HTTP (CORS)
┌──────────────▼──────────────────────────┐
│  HTTP API 层 (Axum)                      │
│  - GET  /api/stats    → JSON 统计       │
│  - POST /api/execute → 代理执行命令     │
│  - CORS 跨域支持                         │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  接入层 (Tokio TCP Server)               │
│  - 监听端口 (127.0.0.1:6379)             │
│  - 每连接一个 Task (tokio::spawn)       │
│  - 协议解析器 (protocol.rs)              │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  逻辑层 (Command Handler)                │
│  - SET: 写入内存 + 注册 TTL              │
│  - GET: 读取内存 + 惰性删除检查           │
│  - DEL: 删除键                           │
│  - STATS: 聚合统计（QPS/命中/延迟）       │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  存储层 (In-Memory Store)                │
│  - RwLock<HashMap> 并发读写              │
│  - TTL 最小堆（惰性删除 + 定期扫描）     │
│  - Store trait 抽象接口                   │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  统计层 (Stats)                          │
│  - AtomicU64 无锁计数器                  │
│  - 延迟分布直方图（<1ms, 1-5ms, 5-10ms） │
└─────────────────────────────────────────┘
```

---

## 2. 模块职责与 SOLID 原则

| 模块 | 职责 | 对应 SOLID 原则 |
|------|------|----------------|
| `protocol.rs` | 类 Redis 文本协议解析 | **SRP**：只负责解析，不涉及存储/网络 |
| `store.rs` | 内存数据存储 + TTL 管理 | **OCP**：`Store` trait 预留替换实现（DashMap/RocksDB） |
| `server.rs` | TCP 连接管理 + 命令分发 | **DIP**：依赖 `Store` 抽象，不依赖具体实现 |
| `stats.rs` | 性能统计（QPS/命中/延迟） | **SRP**：只负责计数，不参与业务逻辑 |
| `api.rs` | HTTP 协议转换 + 路由分发 | **SRP**：只负责 HTTP 层，命令逻辑委托给 store/stats |
| `main.rs` | 程序入口，组装各模块 | — |
| `frontend/` | 前端监控面板（Next.js） | **DRY**：复用后端 API，不重复实现逻辑 |

### 2.1 依赖倒置（DIP）

`server.rs` 和 `api.rs` 都通过 `Store` trait 与存储层交互：

```rust
pub async fn run_server(addr: &str, store: Arc<RwLockStore>, stats: Arc<Stats>);
pub fn create_router(store: Arc<RwLockStore>, stats: Arc<Stats>) -> Router;
```

Week 2 使用 `RwLockStore`，Week 3 的 HTTP API 和 TCP Server 共享同一套存储和统计实例，**无需任何改动即可接入 API**。

### 2.2 开闭原则（OCP）

协议解析通过 `Command` 枚举扩展新命令，无需修改 `parse()` 的核心匹配逻辑。

存储通过 `Store` trait 扩展新实现，无需修改 `server.rs` 或 `api.rs` 的命令处理逻辑。

### 2.3 里氏替换原则（LSP）

任何实现了 `Store` trait 的类型（`MemoryStore`、`RwLockStore`、`DashMapStore`）都可以无缝替换使用。

---

## 3. 动态/静态资源分离

Week 3 前端（Next.js）通过 `next export` 生成静态 HTML/CSS/JS，部署到 Vercel 或 Nginx；后端 Axum 提供动态 JSON API，部署到服务器或容器。

| 资源类型 | 部署方式 | 技术 |
|----------|----------|------|
| 前端静态 | Vercel / CDN | Next.js 静态导出 |
| 后端动态 | 服务器/容器 | Axum + Tokio |
| 协议 | HTTP + CORS | JSON API |

前端通过 `fetch` 轮询 `/api/stats`（每秒一次），实时刷新 Dashboard 数据。命令执行通过 `POST /api/execute` 代理到后端存储层。

---

## 4. 缓存 TTL 策略（Week 2 实现）

Week 2 采用 **惰性删除 + 定期扫描** 的混合策略：

- **惰性删除**：`GET` 命中键时，检查 `expires_at`；若已过期，立即删除并返回 `None`
- **定期扫描**：后台 Task 每 100ms 扫描 TTL 最小堆顶，批量清理过期键
- **优点**：既保证不返回脏数据，又防止内存泄漏
- **缺点**：定期扫描引入少量锁竞争（`Mutex<BinaryHeap>`）

### 演进路线

| 阶段 | 策略 | 说明 |
|------|------|------|
| Week 1 | 惰性删除 | 读取时检查过期时间 |
| Week 2 | 惰性 + 定期扫描 | 后台 Task 每 100ms 扫描 TTL 堆顶，批量清理 |
| 未来 | 惰性 + 定期 + 分片 | 一致性哈希分片，每个分片独立管理 TTL |

---

## 5. 并发模型演进

### Week 1：单线程 + 全局锁

```rust
let store = Arc::new(Mutex::new(MemoryStore::new()));
// 所有读写都竞争同一个 Mutex
```

- 并发模型：**多协程 + 全局锁**
- 适用场景：百级并发

### Week 2：读写分离锁

```rust
let store = Arc::new(RwLockStore::new());
// GET 获取读锁（可并发），SET/DEL 获取写锁（独占）
```

- 并发模型：**多协程 + 读写分离锁**
- 适用场景：千级并发

### 未来：无锁分片

```rust
let store = Arc::new(DashMapStore::new());
// DashMap 内部使用分片锁，读操作无锁
```

- 并发模型：**多协程 + 无锁分片**
- 适用场景：万级并发

---

## 6. Rust Async vs 多线程模型对比

| 维度 | Tokio Async（本项目） | 传统多线程 |
|------|----------------------|-----------|
| 内存占用 | 每个 Task ~几 KB | 每个线程 ~1-2 MB |
| 上下文切换 | 用户态，无内核开销 | 内核态，有开销 |
| 锁粒度 | 更细，可跨 await 点释放 | 较粗，需手动管理 |
| 适用场景 | IO 密集型（网络服务） | CPU 密集型（计算） |
| 代码复杂度 | 需理解 async/await 和 Pin | 更直观 |

本项目选择 Tokio async 的原因：缓存服务器是 **IO 密集型** 应用，每连接一个 Task 可以用极低的资源成本处理上万并发。

---

## 7. 压力测试与性能基线（预留）

Week 3 产出可运行的全栈版本，性能基线待 Week 4 通过 `redis-benchmark` 或自定义 Rust 压测客户端获取。

目标指标：
- QPS > 50k（单机）
- P50 延迟 < 1ms
- P99 延迟 < 5ms

---

## 8. 水平扩展思路（预留）

未来可通过 **一致性哈希** 将键空间分片到多个实例：
- 每个实例拥有独立的 `DashMap` 存储
- 客户端或代理层根据 key 哈希路由到对应实例
- 实现无锁水平扩展，突破单实例锁竞争瓶颈

---

## 9. 前端监控层（Week 3 实现）

前端使用 Next.js 14 (App Router) + TypeScript 构建，通过 HTTP 轮询（每秒）获取后端统计数据。

### 页面1：实时监控面板 (Dashboard)
- **顶部卡片**：当前命令数、总连接数、键数量、命中率
- **中部图表**：
  - 折线图：最近 60 秒 QPS 曲线（Canvas 原生绘制）
  - 柱状图：延迟分布（<1ms, 1-5ms, 5-10ms, >10ms）
- **命令行模拟器**：输入 Redis 命令，通过 POST 到后端执行，返回结果

### 技术选型
- Next.js 14 (App Router) + TypeScript
- 原生 fetch 轮询（无需额外依赖如 SWR）
- Canvas 原生绘制图表（无需 ECharts/Recharts，减少依赖）
- Tailwind 风格内联样式（无需额外 CSS 框架）

### 构建与部署
```bash
cd frontend
npm install
npm run build   # 输出到 dist/ 目录
# 部署 dist/ 到 Vercel 或 Nginx
```
