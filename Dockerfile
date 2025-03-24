FROM rust:1.85

WORKDIR /hitokoto


COPY . .
# 预下载依赖（假设项目使用 cargo 工作区）
RUN cargo build --release --locked


CMD ["target/release/hitokoto-rust"]