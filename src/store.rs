use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 缓存条目
/// 
/// 封装了值和可选的过期时间，隐藏内部实现细节。
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub value: String,
    pub expires_at: Option<Instant>,
}

/// 存储引擎抽象接口
/// 
/// 遵循依赖倒置原则（DIP）：上层模块（server）依赖此抽象接口，
/// 而非具体的存储实现。也遵循接口隔离原则（ISP）：只暴露必要的操作方法。
/// 
/// 遵循里氏替换原则（LSP）：任何实现了 Store 的类型都可以在 server 中互换使用。
pub trait Store: Send + Sync {
    /// 设置键值对，可选 TTL（秒）
    fn set(&mut self, key: String, value: String, ttl: Option<u64>);
    /// 获取键对应的值，惰性检查过期时间
    fn get(&mut self, key: &str) -> Option<String>;
    /// 删除键，返回是否成功删除
    fn del(&mut self, key: &str) -> bool;
    /// 返回当前键数量
    fn len(&self) -> usize;
    /// 检查是否为空
    #[allow(dead_code)]
    fn is_empty(&self) -> bool;
}

/// 基于标准库 HashMap 的单线程内存存储实现
/// 
/// Week 1 实现：单线程 HashMap 存储。
/// 通过惰性删除（Lazy Deletion）在 GET 时检查 TTL，保证不返回脏数据。
/// 
/// 遵循单一职责原则（SRP）：只负责内存数据的增删查和 TTL 管理。
/// 遵循开闭原则（OCP）：未来可替换为 DashMap / RocksDB 等实现，无需修改 server 代码。
pub struct MemoryStore {
    data: HashMap<String, CacheEntry>,
}

impl MemoryStore {
    /// 创建新的空存储实例
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// 内部辅助方法：检查条目是否已过期
    fn is_expired(&self, entry: &CacheEntry) -> bool {
        matches!(
            entry.expires_at,
            Some(expires_at) if Instant::now() > expires_at
        )
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for MemoryStore {
    fn set(&mut self, key: String, value: String, ttl: Option<u64>) {
        let expires_at = ttl.map(|secs| Instant::now() + Duration::from_secs(secs));
        self.data.insert(key, CacheEntry { value, expires_at });
    }

    fn get(&mut self, key: &str) -> Option<String> {
        match self.data.get(key) {
            Some(entry) if self.is_expired(entry) => {
                // 惰性删除：读取时发现过期，立即清理
                self.data.remove(key);
                None
            }
            Some(entry) => Some(entry.value.clone()),
            None => None,
        }
    }

    fn del(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let mut store = MemoryStore::new();
        store.set("key1".to_string(), "value1".to_string(), None);
        assert_eq!(store.get("key1"), Some("value1".to_string()));
    }

    #[test]
    fn test_get_nonexistent() {
        let mut store = MemoryStore::new();
        assert_eq!(store.get("nokey"), None);
    }

    #[test]
    fn test_del() {
        let mut store = MemoryStore::new();
        store.set("k".to_string(), "v".to_string(), None);
        assert!(store.del("k"));
        assert!(!store.del("k"));
    }

    #[test]
    fn test_ttl_lazy_expiration() {
        let mut store = MemoryStore::new();
        // 设置 0 秒 TTL，立即过期
        store.set("k".to_string(), "v".to_string(), Some(0));
        // 惰性删除：GET 时应该返回 None
        assert_eq!(store.get("k"), None);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_stats_len() {
        let mut store = MemoryStore::new();
        assert_eq!(store.len(), 0);
        store.set("a".to_string(), "1".to_string(), None);
        store.set("b".to_string(), "2".to_string(), None);
        assert_eq!(store.len(), 2);
    }
}
