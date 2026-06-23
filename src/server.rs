use crate::protocol::{parse, Command};
use crate::stats::Stats;
use crate::store::{RwLockStore, Store};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

/// 启动 TCP 服务器
/// 
/// Week 2 演进：
/// - 存储层从 `Mutex<MemoryStore>` 替换为 `RwLockStore`（读写分离锁）
/// - 新增 `Stats` 统计模块，每个命令实时记录延迟和命中
/// - 每连接独立 spawn Task，利用 Tokio 协程模型实现轻量级并发
/// 
/// 遵循单一职责原则（SRP）：server 只负责网络连接管理和协议分发，
/// 存储逻辑委托给 `RwLockStore`，统计委托给 `Stats`。
pub async fn run_server(
    addr: &str,
    store: Arc<RwLockStore>,
    stats: Arc<Stats>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("Mini-Cache v0.2.0 listening on {}", addr);
    println!("Store: RwLock<HashMap> (read-write split lock)");

    loop {
        let (socket, peer_addr) = listener.accept().await?;
        stats.record_connection();
        println!("Client connected: {}", peer_addr);

        let store = Arc::clone(&store);
        let stats = Arc::clone(&stats);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, store, stats).await {
                eprintln!("Connection error from {}: {:?}", peer_addr, e);
            }
            println!("Client disconnected: {}", peer_addr);
        });
    }
}

/// 处理单个 TCP 连接
async fn handle_connection(
    mut socket: TcpStream,
    store: Arc<RwLockStore>,
    stats: Arc<Stats>,
) -> std::io::Result<()> {
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let response = process_command(&line, &store, &stats).await;
        writer.write_all(response.as_bytes()).await?;
        writer.flush().await?;
    }

    Ok(())
}

/// 解析并执行命令，返回响应字符串
/// 
/// 遵循 DRY 原则：所有命令响应格式统一在这里组装。
/// 同时记录命令延迟分布（微秒级精度）。
async fn process_command(
    line: &str,
    store: &Arc<RwLockStore>,
    stats: &Arc<Stats>,
) -> String {
    let start = Instant::now();
    stats.record_command();

    let response = match parse(line) {
        Ok(Command::Set { key, value, ttl }) => {
            store.set(key, value, ttl);
            "+OK\r\n".to_string()
        }
        Ok(Command::Get { key }) => {
            match store.get(&key) {
                Some(value) => {
                    stats.record_hit();
                    format!("${}\r\n{}\r\n", value.len(), value)
                }
                None => {
                    stats.record_miss();
                    "$-1\r\n".to_string()
                }
            }
        }
        Ok(Command::Del { key }) => {
            let removed = store.del(&key);
            format!(":{}\r\n", if removed { 1 } else { 0 })
        }
        Ok(Command::Stats) => {
            format!(
                "+STATS commands={} connections={} hits={} misses={} keys={}\r\n",
                stats.total_commands(),
                stats.total_connections(),
                stats.total_hits(),
                stats.total_misses(),
                store.len()
            )
        }
        Ok(Command::Unknown(cmd)) => {
            format!("-ERR unknown command '{}'\r\n", cmd)
        }
        Err(e) => {
            format!("-ERR {}\r\n", e)
        }
    };

    let latency = start.elapsed().as_micros() as u64;
    stats.record_latency(latency);

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_command_set_get() {
        let store = Arc::new(RwLockStore::new());
        let stats = Arc::new(Stats::new());

        let resp = process_command("SET foo bar\r\n", &store, &stats).await;
        assert_eq!(resp, "+OK\r\n");

        let resp = process_command("GET foo\r\n", &store, &stats).await;
        assert_eq!(resp, "$3\r\nbar\r\n");

        let resp = process_command("GET noexist\r\n", &store, &stats).await;
        assert_eq!(resp, "$-1\r\n");
    }

    #[tokio::test]
    async fn test_process_command_del() {
        let store = Arc::new(RwLockStore::new());
        let stats = Arc::new(Stats::new());
        process_command("SET k v\r\n", &store, &stats).await;

        let resp = process_command("DEL k\r\n", &store, &stats).await;
        assert_eq!(resp, ":1\r\n");

        let resp = process_command("DEL k\r\n", &store, &stats).await;
        assert_eq!(resp, ":0\r\n");
    }

    #[tokio::test]
    async fn test_process_command_stats() {
        let store = Arc::new(RwLockStore::new());
        let stats = Arc::new(Stats::new());
        process_command("SET a 1\r\n", &store, &stats).await;
        process_command("GET a\r\n", &store, &stats).await;

        let resp = process_command("STATS\r\n", &store, &stats).await;
        assert!(resp.contains("commands=2"));
        assert!(resp.contains("hits=1"));
        assert!(resp.contains("keys=1"));
    }

    #[tokio::test]
    async fn test_concurrent_commands() {
        let store = Arc::new(RwLockStore::new());
        let stats = Arc::new(Stats::new());

        let mut handles = vec![];
        for i in 0..100 {
            let store = Arc::clone(&store);
            let stats = Arc::clone(&stats);
            handles.push(tokio::spawn(async move {
                let resp = process_command(
                    &format!("SET key{} value{}\r\n", i, i),
                    &store,
                    &stats,
                )
                .await;
                assert_eq!(resp, "+OK\r\n");
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(store.len(), 100);
        assert_eq!(stats.total_commands(), 100);
    }
}
