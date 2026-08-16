# The Rust API (Axum). Migrations are embedded; the SDE seed data is bundled so a fresh
# database seeds itself on first boot.
FROM rust:1.94-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/vector /usr/local/bin/vector
COPY data ./data
ENV LISTEN_ADDR=0.0.0.0:3000
EXPOSE 3000
CMD ["vector"]
