# --- Builder stage ---
FROM rust:1.82-bookworm AS builder

WORKDIR /app

# Cache dependencies by copying manifests first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Build the actual application
COPY . .
RUN touch src/main.rs && cargo build --release

# --- Runtime stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/finopsmind /app/finopsmind
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/config.yaml /app/config.yaml

EXPOSE 8080

CMD ["/app/finopsmind"]
