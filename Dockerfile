# 使用多阶段构建，生成最小镜像

# 构建阶段
FROM rust:1.82 as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples

# 编译 release 版本
RUN cargo build --release --bin mini-cache

# 运行阶段（最小 Debian 镜像）
FROM debian:bookworm-slim

# 安装必要的运行时库
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

# 复制编译好的二进制文件
COPY --from=builder /app/target/release/mini-cache /usr/local/bin/mini-cache

# 暴露端口
EXPOSE 6379 8080

# 入口命令
ENTRYPOINT ["mini-cache"]
