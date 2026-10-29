# syntax=docker/dockerfile:1
# One image per commit: the server binary plus, under /release, everything
# the instance needs to run the stack (Compose file, Caddyfile, Prometheus
# config, Grafana's provisioning and dashboard, built client). Build stages
# run on the build machine's own architecture and cross-compile for arm64,
# so a laptop or an x86 runner produces the same image without emulation.
#
#   docker build --platform linux/arm64 --build-arg COMMIT=$(git rev-parse HEAD) -t formula-none:dev .

ARG RUST_VERSION=1.97
ARG NODE_VERSION=22

FROM node:${NODE_VERSION}-bookworm-slim AS node

# --- server: cross-compiled release binary -----------------------------------
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-bookworm AS server-build
ARG BUILDARCH
ARG COMMIT
RUN test -n "$COMMIT" || { echo "build-arg COMMIT is required" >&2; exit 1; }

# A cross linker only when the build machine is not already arm64.
RUN if [ "$BUILDARCH" != "arm64" ]; then \
      apt-get update && apt-get install -y --no-install-recommends gcc-aarch64-linux-gnu \
      && rm -rf /var/lib/apt/lists/*; \
    fi
RUN rustup target add aarch64-unknown-linux-gnu
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# The version endpoint reports this commit (crates/server/src/main.rs).
ENV FORMULA_NONE_COMMIT=$COMMIT
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target-server,target=/src/target \
    if [ "$BUILDARCH" = "arm64" ]; then unset CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER; fi \
    && cargo build --release -p server --target aarch64-unknown-linux-gnu \
    && cp target/aarch64-unknown-linux-gnu/release/server /server

# --- client: the bridge to wasm, then the Vite build --------------------------
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-bookworm AS web-build
RUN rustup target add wasm32-unknown-unknown \
    && curl -fsSL https://rustwasm.github.io/wasm-pack/installer/init.sh | sh
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && npm install -g pnpm@11.23.0

WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target-web,target=/src/target \
    --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile \
    && pnpm build:bridge \
    && pnpm --filter web build

# --- runtime -------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=server-build /server /usr/local/bin/server
COPY deploy/compose.yaml deploy/Caddyfile deploy/prometheus.yml /release/
COPY deploy/grafana /release/grafana
COPY --from=web-build /src/apps/web/dist /release/www
EXPOSE 8080
# QUIC for WebTransport, when TLS_CERT and TLS_KEY are set (deploy/compose.yaml).
EXPOSE 4433/udp
ENTRYPOINT ["/usr/local/bin/server"]
