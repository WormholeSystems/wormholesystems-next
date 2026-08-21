# The Rust API (Axum). Migrations are embedded; the SDE seed data is bundled so a fresh
# database seeds itself on first boot.
# Pinned to bookworm to match the runtime stage below: the default slim tag tracks a newer
# Debian, and a binary linked against its glibc will not start on bookworm.
FROM rust:1.94-slim-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock sqlx.toml ./
# The workspace names it, so cargo needs it present even though `default-members` keeps it
# out of this build. Its manifest and sources are a few KB.
COPY wsctl ./wsctl
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/wormholesystems /usr/local/bin/wormholesystems
COPY data ./data
ENV LISTEN_ADDR=0.0.0.0:3000
EXPOSE 3000
CMD ["wormholesystems"]
