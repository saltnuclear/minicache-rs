use dashmap::DashMap;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Mutex;
use std::sync::RwLock;
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
///
/// Week 2 关键演进：所有方法签名改为 `&self`，支持并发读写。
pub trait Store: Send + Sync {
    /// 设置键值对，可选 TTL（秒）
    fn set(&self, key: String, value: String, ttl: Option<u64>);
    /// 获取键对应的值，惰性检查过期时间
    fn get(&self, key: &str) -> Option<String>;
    /// 删除键，返回是否成功删除
    fn del(&self, key: &str) -> bool;
    /// 返回当前键数量
    fn len(&self) -> usize;
    /// 检查是否为空
    #[allow(dead_code)]
    fn is_empty(&self) -> bool;
    /// 定期清理过期键（后台任务调用）
    fn cleanup_expired(&self);
}

/// 基于标准库 HashMap 的单线程内存存储实现
///
/// Week 1 参考实现保留，使用 `Mutex<HashMap>` 保持接口兼容。
/// 适用于测试和对比场景。
#[allow(dead_code)]
pub struct MemoryStore {
    data: Mutex<HashMap<String, CacheEntry>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

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
    fn set(&self, key: String, value: String, ttl: Option<u64>) {
        let mut data = self.data.lock().unwrap();
        let expires_at = ttl.map(|secs| Instant::now() + Duration::from_secs(secs));
        data.insert(key, CacheEntry { value, expires_at });
    }

    fn get(&self, key: &str) -> Option<String> {
        let mut data = self.data.lock().unwrap();
        match data.get(key) {
            Some(entry) if self.is_expired(entry) => {
                data.remove(key);
                None
            }
            Some(entry) => Some(entry.value.clone()),
            None => None,
        }
    }

    fn del(&self, key: &str) -> bool {
        let mut data = self.data.lock().unwrap();
        data.remove(key).is_some()
    }

    fn len(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.len()
    }

    fn is_empty(&self) -> bool {
        let data = self.data.lock().unwrap();
        data.is_empty()
    }

    fn cleanup_expired(&self) {}
}

/// 基于 `RwLock<HashMap>` 的并发内存存储实现
///
/// Week 2 核心实现：使用 `std::sync::RwLock` 实现读写分离，
/// 配合 `Mutex<BinaryHeap>` 管理 TTL 过期队列，实现惰性删除 + 定期扫描的混合策略。
///
/// 相比 `Mutex<HashMap>`：读操作可以并发，写操作独占锁。
/// 相比 `DashMap`：实现简单，不需要额外依赖，适合千级并发场景。
///
/// 遵循单一职责原则（SRP）：只负责内存数据的增删查和 TTL 管理。
/// 遵循开闭原则（OCP）：`Store` trait 接口稳定，底层实现可继续演进。
pub struct RwLockStore {
    data: RwLock<HashMap<String, CacheEntry>>,
    /// TTL 最小堆（过期时间, key）
    /// 使用 `Mutex` 保护是因为 `BinaryHeap` 不是线程安全的，
    /// 但写操作（`set`）频率远低于读操作（`get`），锁竞争极小。
    ttl_queue: Mutex<BinaryHeap<(std::cmp::Reverse<Instant>, String)>>,
}

impl RwLockStore {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            ttl_queue: Mutex::new(BinaryHeap::new()),
        }
    }

    fn is_expired(&self, entry: &CacheEntry) -> bool {
        matches!(
            entry.expires_at,
            Some(expires_at) if Instant::now() > expires_at
        )
    }
}

impl Default for RwLockStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for RwLockStore {
    fn set(&self, key: String, value: String, ttl: Option<u64>) {
        let expires_at = ttl.map(|secs| Instant::now() + Duration::from_secs(secs));
        let entry = CacheEntry { value, expires_at };
        {
            let mut data = self.data.write().unwrap();
            data.insert(key.clone(), entry);
        }

        if let Some(exp) = expires_at {
            let mut queue = self.ttl_queue.lock().unwrap();
            queue.push((std::cmp::Reverse(exp), key));
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        {
            let data = self.data.read().unwrap();
            match data.get(key) {
                Some(entry) if !self.is_expired(entry) => {
                    return Some(entry.value.clone());
                }
                Some(_) => {
                    // 过期了，释放读锁，获取写锁来删除
                    drop(data);
                    let mut data = self.data.write().unwrap();
                    data.remove(key);
                    return None;
                }
                None => return None,
            }
        }
    }

    fn del(&self, key: &str) -> bool {
        let mut data = self.data.write().unwrap();
        data.remove(key).is_some()
    }

    fn len(&self) -> usize {
        let data = self.data.read().unwrap();
        data.len()
    }

    fn is_empty(&self) -> bool {
        let data = self.data.read().unwrap();
        data.is_empty()
    }

    fn cleanup_expired(&self) {
        let mut queue = self.ttl_queue.lock().unwrap();
        let now = Instant::now();

        while let Some((std::cmp::Reverse(exp), _)) = queue.peek() {
            if *exp > now {
                break;
            }
            let (_, key) = queue.pop().unwrap();
            let mut data = self.data.write().unwrap();
            if let Some(entry) = data.get(&key) {
                if self.is_expired(entry) {
                    data.remove(&key);
                }
            }
        }
    }
}

/// 基于 `DashMap` 的并发无锁存储实现
///
/// Week 4 优化：使用 `dashmap::DashMap` 替换 `RwLock<HashMap>`，
/// 读操作完全无锁（分片级别），写操作只锁对应分片。
/// 预期 SET QPS 提升 30~50%，P99 延迟降低 50%。
///
/// 保留 `Mutex<BinaryHeap>` 管理 TTL 过期队列，因为 TTL 清理频率
/// 远低于业务操作，锁竞争极小。
pub struct DashMapStore {
    data: DashMap<String, CacheEntry>,
    ttl_queue: Mutex<BinaryHeap<(std::cmp::Reverse<Instant>, String)>>,
}

impl DashMapStore {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
            ttl_queue: Mutex::new(BinaryHeap::new()),
        }
    }

    fn is_expired(&self, entry: &CacheEntry) -> bool {
        matches!(
            entry.expires_at,
            Some(expires_at) if Instant::now() > expires_at
        )
    }
}

impl Default for DashMapStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Store for DashMapStore {
    fn set(&self, key: String, value: String, ttl: Option<u64>) {
        let expires_at = ttl.map(|secs| Instant::now() + Duration::from_secs(secs));
        let entry = CacheEntry { value, expires_at };
        self.data.insert(key.clone(), entry);

        if let Some(exp) = expires_at {
            let mut queue = self.ttl_queue.lock().unwrap();
            queue.push((std::cmp::Reverse(exp), key));
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        if let Some(entry) = self.data.get(key) {
            if !self.is_expired(&entry) {
                return Some(entry.value.clone());
            }
            // 过期了，先释放 Ref 再删除，避免死锁
            drop(entry);
            self.data.remove(key);
        }
        None
    }

    fn del(&self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn cleanup_expired(&self) {
        let mut queue = self.ttl_queue.lock().unwrap();
        let now = Instant::now();

        while let Some((std::cmp::Reverse(exp), _)) = queue.peek() {
            if *exp > now {
                break;
            }
            let (_, key) = queue.pop().unwrap();
            let should_remove = if let Some(entry) = self.data.get(&key) {
                self.is_expired(&entry)
            } else {
                false
            };
            if should_remove {
                self.data.remove(&key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // MemoryStore 测试
    #[test]
    fn test_memory_store_set_and_get() {
        let store = MemoryStore::new();
        store.set("key1".to_string(), "value1".to_string(), None);
        assert_eq!(store.get("key1"), Some("value1".to_string()));
    }

    #[test]
    fn test_memory_store_get_nonexistent() {
        let store = MemoryStore::new();
        assert_eq!(store.get("nokey"), None);
    }

    #[test]
    fn test_memory_store_del() {
        let store = MemoryStore::new();
        store.set("k".to_string(), "v".to_string(), None);
        assert!(store.del("k"));
        assert!(!store.del("k"));
    }

    #[test]
    fn test_memory_store_ttl_lazy_expiration() {
        let store = MemoryStore::new();
        store.set("k".to_string(), "v".to_string(), Some(0));
        assert_eq!(store.get("k"), None);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_memory_store_stats_len() {
        let store = MemoryStore::new();
        assert_eq!(store.len(), 0);
        store.set("a".to_string(), "1".to_string(), None);
        store.set("b".to_string(), "2".to_string(), None);
        assert_eq!(store.len(), 2);
    }

    // RwLockStore 测试
    #[test]
    fn test_rwlock_store_set_and_get() {
        let store = RwLockStore::new();
        store.set("key1".to_string(), "value1".to_string(), None);
        assert_eq!(store.get("key1"), Some("value1".to_string()));
    }

    #[test]
    fn test_rwlock_store_get_nonexistent() {
        let store = RwLockStore::new();
        assert_eq!(store.get("nokey"), None);
    }

    #[test]
    fn test_rwlock_store_del() {
        let store = RwLockStore::new();
        store.set("k".to_string(), "v".to_string(), None);
        assert!(store.del("k"));
        assert!(!store.del("k"));
    }

    #[test]
    fn test_rwlock_store_ttl_lazy_expiration() {
        let store = RwLockStore::new();
        store.set("k".to_string(), "v".to_string(), Some(0));
        assert_eq!(store.get("k"), None);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_rwlock_store_ttl_cleanup() {
        let store = RwLockStore::new();
        store.set("k".to_string(), "v".to_string(), Some(0));
        // 此时键仍在 HashMap 中（未被读取触发惰性删除）
        assert_eq!(store.len(), 1);
        // 手动触发定期扫描
        store.cleanup_expired();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_rwlock_store_concurrent_access() {
        use std::thread;

        let store = std::sync::Arc::new(RwLockStore::new());
        let mut handles = vec![];

        for i in 0..10 {
            let store = std::sync::Arc::clone(&store);
            handles.push(thread::spawn(move || {
                store.set(format!("key{}", i), format!("value{}", i), None);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(store.len(), 10);
    }
}
