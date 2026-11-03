# syntax=docker/dockerfile:1
# One image per commit: the server binary plus, under /release, everything
# the instance needs to run the stack (Compose file, Caddyfile, Prometheus
# config, Grafana's provisioning and dashboard, built client). Build stages
# run on the build machine's own architecture and cross-compile for the
# target, amd64 or arm64, so a laptop or an x86 runner produces the same image
# without emulation. Production runs on arm64 (ADR 0015).
#
#   docker build --platform linux/arm64 --build-arg COMMIT=$(git rev-parse HEAD) -t formula-none:dev .

ARG RUST_VERSION=1.97
ARG NODE_VERSION=22

FROM --platform=$BUILDPLATFORM node:${NODE_VERSION}-bookworm-slim AS node

# --- server: cross-compiled release binary -----------------------------------
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-bookworm AS server-build
ARG BUILDARCH
ARG TARGETARCH
ARG COMMIT
RUN test -n "$COMMIT" || { echo "build-arg COMMIT is required" >&2; exit 1; }

# The Rust target, and the Debian names of its cross compiler and C headers,
# for the architecture asked for. The headers are only a recommended package
# of the compiler, and the ring crate compiles C. Later steps read the target
# from /rust-target, which keeps the mapping in this one place.
RUN case "$TARGETARCH" in \
      amd64) rust_target=x86_64-unknown-linux-gnu; gcc=gcc-x86-64-linux-gnu; headers=libc6-dev-amd64-cross ;; \
      arm64) rust_target=aarch64-unknown-linux-gnu; gcc=gcc-aarch64-linux-gnu; headers=libc6-dev-arm64-cross ;; \
      *) echo "unsupported target architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && echo "$rust_target" > /rust-target \
    && rustup target add "$rust_target" \
    && if [ "$BUILDARCH" != "$TARGETARCH" ]; then \
      apt-get update && apt-get install -y --no-install-recommends "$gcc" "$headers" \
      && rm -rf /var/lib/apt/lists/*; \
    fi
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# The server embeds each course's sidecar (crates/server/src); the rest of the
# course files are the client's. COPY's --parents keeps their paths.
COPY --parents assets/tracks/*/*.track.json ./
# The version endpoint reports this commit (crates/server/src/main.rs).
ENV FORMULA_NONE_COMMIT=$COMMIT
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,id=target-server,target=/src/target \
    if [ "$BUILDARCH" = "$TARGETARCH" ]; then \
      unset CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER; \
    fi \
    && rust_target=$(cat /rust-target) \
    && cargo build --release -p server --target "$rust_target" \
    && cp "target/$rust_target/release/server" /server

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
COPY deploy/compose.yaml deploy/compose.selfhost.yaml deploy/Caddyfile deploy/prometheus.yml /release/
COPY deploy/grafana /release/grafana
COPY --from=web-build /src/apps/web/dist /release/www
EXPOSE 8080
# QUIC for WebTransport, when TLS_CERT and TLS_KEY are set (deploy/compose.yaml).
EXPOSE 4433/udp
ENTRYPOINT ["/usr/local/bin/server"]
