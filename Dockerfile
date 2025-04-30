# 第一阶段：构建阶段（使用 musl 静态编译）
FROM rust:1.73-alpine as builder

# 安装构建依赖
RUN apk add --no-cache \
    musl-dev \
    ca-certificates

# 安装 musl 目标
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app

# 缓存依赖
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl && \
    rm -rf src

# 复制源码
COPY src ./src

# 构建正式版本（完全静态链接）
RUN cargo build --release \
    --target x86_64-unknown-linux-musl \
    -j $(nproc) && \
    strip target/x86_64-unknown-linux-musl/release/hitokoto-rust

# 第二阶段：运行时镜像（最小化）
FROM scratch

# 复制时区数据
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# 复制可执行文件
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/hitokoto-rust /

# 设置环境变量
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

EXPOSE 8080

# 启动命令
CMD ["/hitokoto-rust"]