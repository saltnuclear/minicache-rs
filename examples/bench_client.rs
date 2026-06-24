use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// 自定义压测客户端
///
/// 使用 Tokio 并发多个 Task，模拟真实客户端连接。
/// 每个 Task 独立建立 TCP 连接，发送命令并读取响应。
///
/// 用法：
/// cargo run --bin bench-client -- --host 127.0.0.1 --port 6379 --clients 1000 --requests 100000 --cmd set
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = parse_args();

    println!("🚀 Mini-Cache Bench Client");
    println!("Target: {}:{}", args.host, args.port);
    println!("Clients: {} | Requests: {} | Command: {}", args.clients, args.requests, args.cmd);
    println!("----------------------------------------");

    let total_requests = Arc::new(AtomicU64::new(0));
    let total_latency = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(std::sync::Mutex::new(Vec::<u64>::new()));

    let start = Instant::now();
    let mut handles = vec![];

    // 每个客户端 Task 分配均匀的请求数
    let reqs_per_client = args.requests / args.clients;

    for i in 0..args.clients {
        let host = args.host.clone();
        let port = args.port;
        let cmd = args.cmd.clone();
        let total_req = Arc::clone(&total_requests);
        let total_lat = Arc::clone(&total_latency);
        let lats = Arc::clone(&latencies);

        let handle = tokio::spawn(async move {
            match TcpStream::connect(format!("{}:{}", host, port)).await {
                Ok(stream) => {
                    // 使用 tokio::io::split 分离读写两端
                    let (reader, mut writer) = tokio::io::split(stream);
                    let mut reader = BufReader::new(reader);
                    let mut line = String::new();

                    for j in 0..reqs_per_client {
                        let key = format!("key{}_{}", i, j);
                        let command = build_command(&cmd, &key, j);

                        let req_start = Instant::now();
                        if let Err(e) = writer.write_all(command.as_bytes()).await {
                            eprintln!("Client {} send error: {}", i, e);
                            continue;
                        }
                        line.clear();
                        if let Err(e) = reader.read_line(&mut line).await {
                            eprintln!("Client {} read error: {}", i, e);
                            continue;
                        }
                        let latency_us = req_start.elapsed().as_micros() as u64;

                        total_req.fetch_add(1, Ordering::Relaxed);
                        total_lat.fetch_add(latency_us, Ordering::Relaxed);
                        lats.lock().unwrap().push(latency_us);
                    }
                }
                Err(e) => {
                    eprintln!("Client {} connection failed: {}", i, e);
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.await?;
    }

    let elapsed = start.elapsed().as_secs_f64();
    let total = total_requests.load(Ordering::Relaxed);
    let qps = total as f64 / elapsed;

    let lats = latencies.lock().unwrap();
    let mut sorted = lats.clone();
    drop(lats);
    sorted.sort();

    let p50 = percentile(&sorted, 0.50);
    let p99 = percentile(&sorted, 0.99);

    println!("\n📊 压测结果");
    println!("----------------------------------------");
    println!("总请求数: {}", total);
    println!("总耗时:   {:.2}s", elapsed);
    println!("QPS:      {:.0}", qps);
    println!("P50 延迟: {:.2}ms", p50 / 1000.0);
    println!("P99 延迟: {:.2}ms", p99 / 1000.0);
    println!("----------------------------------------");

    Ok(())
}

struct Args {
    host: String,
    port: u16,
    clients: u64,
    requests: u64,
    cmd: String, // set, get, mixed
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut host = "127.0.0.1".to_string();
    let mut port = 6379u16;
    let mut clients = 100u64;
    let mut requests = 10000u64;
    let mut cmd = "set".to_string();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().unwrap_or_default(),
            "--port" => port = args.next().unwrap_or_default().parse().unwrap_or(6379),
            "--clients" => clients = args.next().unwrap_or_default().parse().unwrap_or(100),
            "--requests" => requests = args.next().unwrap_or_default().parse().unwrap_or(10000),
            "--cmd" => cmd = args.next().unwrap_or_default(),
            _ => {}
        }
    }

    Args {
        host,
        port,
        clients,
        requests,
        cmd,
    }
}

fn build_command(cmd: &str, key: &str, _idx: u64) -> String {
    match cmd {
        "set" => format!("SET {} value{}\r\n", key, _idx),
        "get" => format!("GET {}\r\n", key),
        "mixed" => {
            if _idx % 5 == 0 {
                format!("SET {} value{}\r\n", key, _idx)
            } else {
                format!("GET {}\r\n", key)
            }
        }
        _ => format!("SET {} value{}\r\n", key, _idx),
    }
}

fn percentile(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p) as usize;
    sorted[idx.clamp(0, sorted.len() - 1)] as f64
}
