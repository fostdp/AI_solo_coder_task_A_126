FROM rust:1.75-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y libssl-dev pkg-config
COPY Cargo.toml .
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
COPY src src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/bell-casting-backend /app/
EXPOSE 8080
CMD ["./bell-casting-backend", "--skip-mqtt"]
