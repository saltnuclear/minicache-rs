mod api;
mod protocol;
mod server;
mod stats;
mod store;

use std::sync::Arc;
use std::time::Duration;
use server::run_server;
use stats::Stats;
use store::{DashMapStore, Store};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let tcp_addr = "127.0.0.1:6379";
    let http_addr = "0.0.0.0:8080";
    println!("Mini-Cache v0.4.0 starting...");
    println!("Week 4: DashMap Store Optimization");

    let store: Arc<dyn Store> = Arc::new(DashMapStore::new());
    let stats = Arc::new(Stats::new());

    // 启动 TTL 定期清理后台任务
    let cleanup_store = Arc::clone(&store);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            cleanup_store.cleanup_expired();
        }
    });

    // 启动 HTTP API 服务器
    let api_store = Arc::clone(&store);
    let api_stats = Arc::clone(&stats);
    tokio::spawn(async move {
        let app = api::create_router(api_store, api_stats);
        let listener = match tokio::net::TcpListener::bind(http_addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to bind HTTP API on {}: {}", http_addr, e);
                return;
            }
        };
        println!("HTTP API listening on {}", http_addr);
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("HTTP API error: {}", e);
        }
    });

    // 启动 TCP 缓存服务器（主线程阻塞）
    println!("TCP Server listening on {}", tcp_addr);
    run_server(tcp_addr, store, stats).await
}
