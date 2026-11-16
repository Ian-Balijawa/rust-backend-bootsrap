FROM rust:1.99-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system api \
    && useradd --system --gid api --home-dir /app api \
    && mkdir -p /app/uploads \
    && chown -R api:api /app
WORKDIR /app
COPY --from=build /app/target/release/axum_template /usr/local/bin/axum_template
COPY config.ini ./config.ini
USER api
EXPOSE 8000
ENTRYPOINT ["/usr/local/bin/axum_template"]
