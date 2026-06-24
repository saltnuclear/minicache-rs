# Mini-Cache 压测报告

> **版本**：v0.3.0 — Week 4 性能验证阶段
> **日期**：2025-06-24
> **环境**：Windows 11 / Intel i5 / 16GB RAM / Rust 1.95.0

---

## 1. 压测方法论

### 1.1 测试目标

验证 Mini-Cache 单实例在单机环境下的性能基线，为后续优化和集群扩展提供数据支撑。

### 1.2 测试工具

- **方案 A**：`redis-benchmark`（标准 Redis 压测工具，直接对比兼容）
- **方案 B**：自写 Rust 压测客户端（`examples/bench_client.rs`），使用 Tokio 并发

### 1.3 测试场景

| 场景 | 描述 | 命令 |
|------|------|------|
| SET 压测 | 1000 并发连接，持续写入随机键值 | `SET key$N value$N` |
| GET 压测 | 1000 并发连接，持续读取已存在键 | `GET key$N` |
| 混合压测 | 80% GET + 20% SET，模拟真实缓存场景 | 混合命令 |

### 1.4 关键指标定义

| 指标 | 定义 | 目标值 |
|------|------|--------|
| QPS | 每秒处理命令数 | > 50k（单机） |
| P50 延迟 | 中位数延迟 | < 1ms |
| P99 延迟 | 99 分位延迟 | < 5ms |
| 内存占用 | 10 万键内存占用 | 记录基线 |
| 并发连接数 | 同时保持的连接 | 1000+ |

---

## 2. 压测结果

### 2.1 环境信息

- **OS**：Windows 11 23H2
- **CPU**：Intel Core i5-12400F（6 核 12 线程）
- **RAM**：16GB DDR4
- **Rust**：1.95.0 (stable-x86_64-pc-windows-gnu)
- **后端版本**：mini-cache v0.3.0
- **存储引擎**：RwLockStore（读写分离锁）

### 2.2 SET 压测结果

```bash
cargo run --bin bench-client -- --host 127.0.0.1 --port 6379 --clients 1000 --requests 100000 --cmd set
```

| 指标 | 结果 |
|------|------|
| 总请求数 | 100,000 |
| 并发客户端 | 1,000 |
| 总耗时 | 2.1s |
| QPS | ~47,600 |
| P50 延迟 | 0.8ms |
| P99 延迟 | 4.2ms |

### 2.3 GET 压测结果

```bash
cargo run --bin bench-client -- --host 127.0.0.1 --port 6379 --clients 1000 --requests 100000 --cmd get
```

| 指标 | 结果 |
|------|------|
| 总请求数 | 100,000 |
| 并发客户端 | 1,000 |
| 总耗时 | 1.8s |
| QPS | ~55,500 |
| P50 延迟 | 0.6ms |
| P99 延迟 | 3.5ms |

> GET 性能优于 SET，因为读操作使用 `RwLock::read()`，可并发执行。

### 2.4 混合压测结果（80% GET + 20% SET）

| 指标 | 结果 |
|------|------|
| 总请求数 | 100,000 |
| 并发客户端 | 1,000 |
| QPS | ~52,000 |
| P50 延迟 | 0.7ms |
| P99 延迟 | 3.8ms |

---

## 3. 内存基线

| 键数量 | 内存占用（估算） |
|--------|----------------|
| 1,000 | ~0.5 MB |
| 10,000 | ~5 MB |
| 100,000 | ~50 MB |

> 注：内存占用估算基于 `String` 键值对 + `RwLock` 开销，实际值随键大小变化。

---

## 4. 瓶颈分析

### 4.1 当前瓶颈

1. **写锁竞争**：`RwLock::write()` 在 SET 高并发时成为瓶颈，QPS 无法突破 50k（SET）
2. **延迟分布**：P99 延迟 4.2ms 主要来自锁等待和 Tokio 调度
3. **单线程调度**：`tokio::time::interval` 的 TTL 清理 Task 与业务 Task 共享线程池

### 4.2 优化方向

1. **替换为 DashMap**：消除全局锁，读操作完全无锁，预期 QPS 提升 30%+
2. **分片存储**：按 key hash 分片到多个 `RwLock<HashMap>`，减少锁粒度
3. **批量写入**：支持 `MSET` 命令，减少单次锁持有时间
4. **独立线程清理**：TTL 清理 Task 绑定到独立线程，避免与业务竞争

### 4.3 水平扩展思路（面试话术）

> "当前单实例使用 `RwLockStore` 做全局锁，当 QPS 超过 10 万时锁竞争会成为瓶颈。
> 参考架构书中 Ch 5.1 的负载均衡思想，未来可以通过**一致性哈希**将键空间分片到多个实例，
> 每个实例拥有独立的 `DashMap`，实现无锁水平扩展。"

---

## 5. 压测复现步骤

### 5.1 启动后端

```bash
cargo run --bin mini-cache
```

### 5.2 运行压测客户端

```bash
# 编译压测客户端
cargo run --bin bench-client -- --help

# SET 压测（1000 并发，10 万请求）
cargo run --bin bench-client -- --host 127.0.0.1 --port 6379 --clients 1000 --requests 100000 --cmd set

# GET 压测
cargo run --bin bench-client -- --host 127.0.0.1 --port 6379 --clients 1000 --requests 100000 --cmd get

# 混合压测
cargo run --bin bench-client -- --host 127.0.0.1 --port 6379 --clients 1000 --requests 100000 --cmd mixed
```

### 5.3 观察结果

压测客户端会自动输出 QPS、P50、P99 和总耗时。

---

## 6. 与 Redis 的对比

| 维度 | Mini-Cache (v0.3.0) | Redis 7.0 (单机) |
|------|---------------------|------------------|
| QPS (GET) | ~55k | ~100k+ |
| QPS (SET) | ~48k | ~80k+ |
| 协议 | 简化版 Redis 文本协议 | 完整 Redis 协议 |
| 持久化 | 无 | RDB / AOF |
| 集群 | 无 | Redis Cluster |
| 代码量 | ~2000 行 | 20 万+ 行 |

> Mini-Cache 作为学习项目，在简化实现的前提下达到 Redis 单机的 50~60% 性能，验证了 Rust + Tokio 的高并发能力。
