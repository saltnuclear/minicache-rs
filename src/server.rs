use crate::protocol::{parse, Command};
use crate::store::MemoryStore;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// 启动 TCP 服务器
/// 
/// 遵循单一职责原则（SRP）：server 只负责网络连接管理和协议分发，
/// 存储逻辑委托给 Store 实现，协议解析委托给 protocol 模块。
/// 
/// 每个客户端连接独立 spawn 一个异步 Task，利用 Tokio 协程模型实现轻量级并发。
/// Week 1 使用 `tokio::sync::Mutex<MemoryStore>` 保护单线程 HashMap，
/// 为 Week 2 无缝替换为 DashMap 预留扩展点。
pub async fn run_server(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("Mini-Cache listening on {}", addr);

    // 使用 Arc + Mutex 在多个 Task 间共享存储
    // 依赖倒置：server 依赖具体类型 MemoryStore（Week 1），
    // 后续可通过泛型或 trait object 替换为其他 Store 实现。
    let store = Arc::new(Mutex::new(MemoryStore::new()));

    loop {
        let (socket, peer_addr) = listener.accept().await?;
        println!("Client connected: {}", peer_addr);

        let store = Arc::clone(&store);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, store).await {
                eprintln!("Connection error from {}: {:?}", peer_addr, e);
            }
            println!("Client disconnected: {}", peer_addr);
        });
    }
}

/// 处理单个 TCP 连接
/// 
/// 读取客户端发送的每行文本，解析为 Command，委托给 Store 执行，
/// 并将结果写回客户端。遵循 DRY 原则：所有命令响应格式统一在这里组装。
async fn handle_connection(
    socket: TcpStream,
    store: Arc<Mutex<MemoryStore>>,
) -> std::io::Result<()> {
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            // 客户端断开连接
            break;
        }

        let response = process_command(&line, &store).await;
        writer.write_all(response.as_bytes()).await?;
        writer.flush().await?;
    }

    Ok(())
}

/// 解析并执行命令，返回响应字符串
/// 
/// 将命令处理逻辑从 handle_connection 中抽离，保持函数短小且职责单一。
async fn process_command(line: &str, store: &Arc<Mutex<MemoryStore>>) -> String {
    match parse(line) {
        Ok(Command::Set { key, value, ttl }) => {
            let mut store = store.lock().await;
            store.set(key, value, ttl);
            "+OK\r\n".to_string()
        }
        Ok(Command::Get { key }) => {
            let mut store = store.lock().await;
            match store.get(&key) {
                Some(value) => format!("${}\r\n{}\r\n", value.len(), value),
                None => "$-1\r\n".to_string(),
            }
        }
        Ok(Command::Del { key }) => {
            let mut store = store.lock().await;
            let removed = store.del(&key);
            format!(":{}\r\n", if removed { 1 } else { 0 })
        }
        Ok(Command::Stats) => {
            let store = store.lock().await;
            format!(":{}\r\n", store.len())
        }
        Ok(Command::Unknown(cmd)) => {
            format!("-ERR unknown command '{}'\r\n", cmd)
        }
        Err(e) => {
            format!("-ERR {}\r\n", e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_command_set_get() {
        let store = Arc::new(Mutex::new(MemoryStore::new()));
        
        let resp = process_command("SET foo bar\r\n", &store).await;
        assert_eq!(resp, "+OK\r\n");

        let resp = process_command("GET foo\r\n", &store).await;
        assert_eq!(resp, "$3\r\nbar\r\n");

        let resp = process_command("GET noexist\r\n", &store).await;
        assert_eq!(resp, "$-1\r\n");
    }

    #[tokio::test]
    async fn test_process_command_del() {
        let store = Arc::new(Mutex::new(MemoryStore::new()));
        process_command("SET k v\r\n", &store).await;

        let resp = process_command("DEL k\r\n", &store).await;
        assert_eq!(resp, ":1\r\n");

        let resp = process_command("DEL k\r\n", &store).await;
        assert_eq!(resp, ":0\r\n");
    }

    #[tokio::test]
    async fn test_process_command_stats() {
        let store = Arc::new(Mutex::new(MemoryStore::new()));
        process_command("SET a 1\r\n", &store).await;
        process_command("SET b 2\r\n", &store).await;

        let resp = process_command("STATS\r\n", &store).await;
        assert_eq!(resp, ":2\r\n");
    }
}
