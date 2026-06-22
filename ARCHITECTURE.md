# Mini-Cache 架构设计文档（Week 1 初稿）

> **版本**：v0.1.0 — Week 1 骨架阶段  
> **目标**：记录架构设计思路，为后续迭代提供扩展蓝图。

---

## 1. 系统分层架构

```text
┌─────────────────────────────────────────┐
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
│  - STATS: 返回键数量                      │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  存储层 (In-Memory Store)                │
│  - HashMap<String, CacheEntry>           │
│  - TTL 过期（惰性删除）                   │
│  - Store trait 抽象接口                   │
└─────────────────────────────────────────┘
```

## 2. 模块职责与 SOLID 原则

| 模块 | 职责 | 对应 SOLID 原则 |
|------|------|----------------|
| `protocol.rs` | 类 Redis 文本协议解析 | **SRP**：只负责解析，不涉及存储/网络 |
| `store.rs` | 内存数据存储 + TTL 管理 | **OCP**：`Store` trait 预留替换实现（DashMap/RocksDB） |
| `server.rs` | TCP 连接管理 + 命令分发 | **DIP**：依赖 `Store` 抽象，不依赖具体实现 |
| `main.rs` | 程序入口，组装各模块 | — |

### 2.1 依赖倒置（DIP）

`server.rs` 通过 `Store` trait 与存储层交互：

```rust
// server 只关心 Store 接口，不关心底层是 HashMap 还是 DashMap
pub async fn run_server(addr: &str) -> std::io::Result<()> {
    let store = Arc::new(Mutex::new(MemoryStore::new()));
    // ...
}
```

Week 1 使用 `MemoryStore`（单线程 HashMap），Week 2 可直接替换为 `DashMapStore` 等实现，**server 代码无需改动**。

### 2.2 开闭原则（OCP）

协议解析通过 `Command` 枚举扩展新命令，无需修改 `parse()` 的核心匹配逻辑：

```rust
pub enum Command {
    Set { key: String, value: String, ttl: Option<u64> },
    Get { key: String },
    Del { key: String },
    Stats,
    // 未来可扩展：MGET, EXISTS, INCR 等
}
```

### 2.3 里氏替换原则（LSP）

任何实现了 `Store` trait 的类型（`MemoryStore`、`DashMapStore`、`RocksDbStore`）都可以无缝替换使用。

---

## 3. 缓存 TTL 策略（Week 1 实现）

Week 1 采用 **惰性删除（Lazy Deletion）**：
- **读取时检查**：`GET` 命中键时，检查 `expires_at`；若已过期，立即删除并返回 `None`
- **优点**：实现简单，不引入额外线程
- **缺点**：过期键不访问时可能长期驻留内存

### 演进路线

| 阶段 | 策略 | 说明 |
|------|------|------|
| Week 1 | 惰性删除 | 读取时检查过期时间 |
| Week 2 | 惰性 + 定期扫描 | 后台 Task 每 100ms 扫描 TTL 堆顶，批量清理 |
| 未来 | 惰性 + 定期 + 分片 | 一致性哈希分片，每个分片独立管理 TTL |

---

## 4. 协程模型与并发设计

### 当前实现（Week 1）

- 使用 **Tokio** 异步运行时，每连接 `tokio::spawn` 一个 Task
- 存储层使用 `tokio::sync::Mutex<MemoryStore>` 保护单线程 HashMap
- 并发模型：**多协程 + 全局锁**

### 演进路线

| 阶段 | 存储层 | 并发模型 | 适用场景 |
|------|--------|----------|----------|
| Week 1 | `Mutex<HashMap>` | 多协程 + 全局锁 | 千级并发 |
| Week 2 | `DashMap` | 无锁分片 | 万级并发 |
| 未来 | 一致性哈希分片 | 多实例无锁 | 集群水平扩展 |

---

## 5. 压力测试与性能基线（预留）

Week 1 产出可运行的骨架，性能基线待 Week 4 通过 `redis-benchmark` 或自定义 Rust 压测客户端获取。

目标指标：
- QPS > 50k（单机）
- P50 延迟 < 1ms
- P99 延迟 < 5ms

---

## 6. 水平扩展思路（预留）

未来可通过 **一致性哈希** 将键空间分片到多个实例：
- 每个实例拥有独立的 `DashMap` 存储
- 客户端或代理层根据 key 哈希路由到对应实例
- 实现无锁水平扩展，突破单实例锁竞争瓶颈

---

## 7. 前端监控层（预留）

Week 3 接入 Next.js + ECharts 监控面板：
- 实时 QPS 曲线（WebSocket 推送）
- 延迟分布柱状图
- 命令日志表格
- 命令行模拟器（Redis CLI 体验）

前端静态资源与后端动态 API 分离部署，对应动态/静态资源分离架构思想。
