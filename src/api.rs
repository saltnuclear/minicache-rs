use crate::protocol::{parse, Command};
use crate::stats::Stats;
use crate::store::Store;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// HTTP API 模块
///
/// 提供 REST 接口给前端监控面板：
/// - GET  /api/stats    → 实时统计（JSON）
/// - POST /api/execute  → 代理执行缓存命令
///
/// 遵循单一职责原则（SRP）：api 只负责 HTTP 协议层转换，
/// 命令执行逻辑委托给 store/stats，避免在 handler 中写业务逻辑。
/// 统计响应结构
#[derive(Serialize)]
pub struct StatsResponse {
    pub commands: u64,
    pub connections: u64,
    pub hits: u64,
    pub misses: u64,
    pub keys: usize,
    pub latency_histogram: [u64; 4],
}

/// 命令执行请求
#[derive(Deserialize)]
pub struct ExecuteRequest {
    pub command: String,
}

/// 命令执行响应
#[derive(Serialize)]
pub struct ExecuteResponse {
    pub result: String,
}

/// 构建 Axum 路由
///
/// 使用 State 注入共享的 Store 和 Stats 实例。
/// CORS 配置允许前端 localhost:3000 访问（开发环境）。
pub fn create_router(store: Arc<dyn Store>, stats: Arc<Stats>) -> Router {
    let app_state = AppState { store, stats };

    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/execute", post(post_execute))
        .with_state(app_state)
        .layer(tower_http::cors::CorsLayer::permissive())
}

#[derive(Clone)]
struct AppState {
    store: Arc<dyn Store>,
    stats: Arc<Stats>,
}

/// GET /api/stats
///
/// 返回实时统计信息，供前端 Dashboard 展示。
async fn get_stats(State(state): State<AppState>) -> Json<StatsResponse> {
    Json(StatsResponse {
        commands: state.stats.total_commands(),
        connections: state.stats.total_connections(),
        hits: state.stats.total_hits(),
        misses: state.stats.total_misses(),
        keys: state.store.len(),
        latency_histogram: state.stats.latency_histogram(),
    })
}

/// POST /api/execute
///
/// 代理执行缓存命令，返回类 Redis 协议响应。
/// 前端命令行模拟器通过此接口与后端交互。
async fn post_execute(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    let response = process_api_command(&req.command, &*state.store, &state.stats);
    Ok(Json(ExecuteResponse { result: response }))
}

/// 执行命令并返回响应字符串
///
/// 与 server.rs 的 process_command 保持一致，但使用同步 API（无 async）。
/// 遵循 DRY 原则：核心逻辑共享，只是调用上下文不同。
fn process_api_command(line: &str, store: &dyn Store, stats: &Arc<Stats>) -> String {
    stats.record_command();

    match parse(line) {
        Ok(Command::Set { key, value, ttl }) => {
            store.set(key, value, ttl);
            "+OK".to_string()
        }
        Ok(Command::Get { key }) => match store.get(&key) {
            Some(value) => {
                stats.record_hit();
                value.to_string()
            }
            None => {
                stats.record_miss();
                "(nil)".to_string()
            }
        },
        Ok(Command::Del { key }) => {
            let removed = store.del(&key);
            format!("{}", if removed { 1 } else { 0 })
        }
        Ok(Command::Stats) => {
            format!(
                "commands={} connections={} hits={} misses={} keys={}",
                stats.total_commands(),
                stats.total_connections(),
                stats.total_hits(),
                stats.total_misses(),
                store.len()
            )
        }
        Ok(Command::Unknown(cmd)) => {
            format!("-ERR unknown command '{}'", cmd)
        }
        Err(e) => {
            format!("-ERR {}", e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::RwLockStore;

    #[tokio::test]
    async fn test_api_stats() {
        let store: Arc<dyn Store> = Arc::new(RwLockStore::new());
        let stats = Arc::new(Stats::new());

        store.set("k".to_string(), "v".to_string(), None);
        stats.record_command();
        stats.record_connection();

        let resp = get_stats(State(AppState {
            store: Arc::clone(&store),
            stats: Arc::clone(&stats),
        }))
        .await;

        assert_eq!(resp.commands, 1);
        assert_eq!(resp.connections, 1);
        assert_eq!(resp.keys, 1);
    }

    #[tokio::test]
    async fn test_api_execute_set_get() {
        let store: Arc<dyn Store> = Arc::new(RwLockStore::new());
        let stats = Arc::new(Stats::new());

        let req = ExecuteRequest {
            command: "SET foo bar".to_string(),
        };
        let resp = post_execute(
            State(AppState {
                store: Arc::clone(&store),
                stats: Arc::clone(&stats),
            }),
            Json(req),
        )
        .await
        .unwrap();
        assert_eq!(resp.result, "+OK");

        let req = ExecuteRequest {
            command: "GET foo".to_string(),
        };
        let resp = post_execute(
            State(AppState {
                store: Arc::clone(&store),
                stats,
            }),
            Json(req),
        )
        .await
        .unwrap();
        assert_eq!(resp.result, "bar");
    }
}
