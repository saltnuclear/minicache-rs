mod protocol;
mod server;
mod stats;
mod store;

use std::sync::Arc;
use std::time::Duration;
use server::run_server;
use stats::Stats;
use store::RwLockStore;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = "127.0.0.1:6379";
    println!("Mini-Cache v0.2.0 starting...");
    println!("Week 2: RwLockStore + TTL cleanup + Stats");

    let store = Arc::new(RwLockStore::new());

    // 启动 TTL 定期清理后台任务
    // 每 100ms 扫描最小堆顶，批量清理过期键
    let cleanup_store = Arc::clone(&store);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            cleanup_store.cleanup_expired();
        }
    });

    let stats = Arc::new(Stats::new());
    println!("Listening on {}", addr);
    run_server(addr, store, stats).await
}
