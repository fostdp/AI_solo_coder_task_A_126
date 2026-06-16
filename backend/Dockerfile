# ============================================================
# 古代铸钟工艺仿真系统 - Rust后端 Dockerfile
# 多阶段构建：最终镜像为 scratch + 静态二进制
# ============================================================

# ---------- 阶段1: 构建环境 ----------
FROM rust:1.77-alpine3.19 AS builder
WORKDIR /app

RUN apk add --no-cache musl-dev openssl-dev perl make

ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV OPENSSL_STATIC=1

# 创建空项目以缓存依赖
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl 2>/dev/null || true

# 复制源码构建
COPY src src
COPY config ../config
RUN touch src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl

# 验证二进制为静态链接
RUN ldd target/x86_64-unknown-linux-musl/release/bell-casting-backend 2>&1 | grep -q "not a dynamic executable" || \
    (echo "ERROR: Binary is not statically linked" && exit 1)

# ---------- 阶段2: 运行时 (scratch + CA证书) ----------
FROM scratch
WORKDIR /app

# 从构建阶段复制静态二进制
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/bell-casting-backend .

# 复制CA证书（用于HTTPS）
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# 创建 /tmp 目录
COPY --from=builder /tmp /tmp

EXPOSE 8080
EXPOSE 9090

CMD ["./bell-casting-backend"]
