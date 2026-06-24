use std::sync::atomic::{AtomicU64, Ordering};

/// 性能统计模块
/// 
/// 使用 `AtomicU64` 实现无锁计数，避免多线程竞争。
/// 所有操作使用 `Ordering::Relaxed`，在保证正确性的同时最大化性能。
/// 
/// 遵循单一职责原则（SRP）：只负责计数和快照，不参与业务逻辑。
/// 遵循接口隔离原则（ISP）：只暴露统计操作，隐藏内部实现。
#[derive(Default)]
pub struct Stats {
    total_commands: AtomicU64,
    total_connections: AtomicU64,
    total_hits: AtomicU64,
    total_misses: AtomicU64,
    // 延迟分桶（微秒）
    latency_0_1ms: AtomicU64,
    latency_1_5ms: AtomicU64,
    latency_5_10ms: AtomicU64,
    latency_over_10ms: AtomicU64,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录一条命令执行
    pub fn record_command(&self) {
        self.total_commands.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录一个客户端连接
    pub fn record_connection(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存命中
    pub fn record_hit(&self) {
        self.total_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录缓存未命中
    pub fn record_miss(&self) {
        self.total_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录延迟（微秒）
    pub fn record_latency(&self, latency_us: u64) {
        let target = match latency_us {
            0..=1000 => &self.latency_0_1ms,
            1001..=5000 => &self.latency_1_5ms,
            5001..=10000 => &self.latency_5_10ms,
            _ => &self.latency_over_10ms,
        };
        target.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_commands(&self) -> u64 {
        self.total_commands.load(Ordering::Relaxed)
    }

    pub fn total_connections(&self) -> u64 {
        self.total_connections.load(Ordering::Relaxed)
    }

    pub fn total_hits(&self) -> u64 {
        self.total_hits.load(Ordering::Relaxed)
    }

    pub fn total_misses(&self) -> u64 {
        self.total_misses.load(Ordering::Relaxed)
    }

    /// 返回延迟分布直方图：[<1ms, 1-5ms, 5-10ms, >10ms]
    #[allow(dead_code)]
    pub fn latency_histogram(&self) -> [u64; 4] {
        [
            self.latency_0_1ms.load(Ordering::Relaxed),
            self.latency_1_5ms.load(Ordering::Relaxed),
            self.latency_5_10ms.load(Ordering::Relaxed),
            self.latency_over_10ms.load(Ordering::Relaxed),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_counters() {
        let stats = Stats::new();
        stats.record_command();
        stats.record_command();
        stats.record_connection();
        stats.record_hit();
        stats.record_miss();

        assert_eq!(stats.total_commands(), 2);
        assert_eq!(stats.total_connections(), 1);
        assert_eq!(stats.total_hits(), 1);
        assert_eq!(stats.total_misses(), 1);
    }

    #[test]
    fn test_latency_histogram() {
        let stats = Stats::new();
        stats.record_latency(500);   // 0-1ms
        stats.record_latency(2000);  // 1-5ms
        stats.record_latency(7000);  // 5-10ms
        stats.record_latency(15000); // >10ms

        let hist = stats.latency_histogram();
        assert_eq!(hist, [1, 1, 1, 1]);
    }
}
