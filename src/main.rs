mod protocol;
mod server;
mod store;

use server::run_server;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = "127.0.0.1:6379";
    println!("Mini-Cache v0.1.0 starting...");
    println!("Listening on {}", addr);
    run_server(addr).await
}
