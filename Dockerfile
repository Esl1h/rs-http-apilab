# syntax=docker/dockerfile:1

# rs-http-apilab image.
#
# The builder compiles with the Rust toolchain pinned by
# rust-toolchain.toml (1.98.1) on Debian bookworm. The binary is
# dynamically linked against glibc (libgcc_s, libc and friends), so the
# runtime stage must ship a compatible glibc: both stages use Debian
# bookworm (glibc 2.36). The `ldd` call below documents the actual
# runtime dependencies; do not switch the runtime image to
# alpine/scratch without re-checking it.

# ---- builder: cargo release build with the pinned toolchain ----
FROM rust:1.98.1-bookworm AS builder

WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
COPY src ./src

# --locked honors the committed Cargo.lock; the rustc version is baked
# into /version by build.rs at compile time. Verify the produced
# binary's dynamic dependencies before choosing the runtime image.
RUN mkdir -p /out \
	&& cargo build --release --locked \
	&& cp target/release/rs-http-apilab /out/rs-http-apilab \
	&& echo '--- ldd of the final binary:' \
	&& ldd /out/rs-http-apilab

# ---- runtime: glibc userland, non-root user, curl for HEALTHCHECK ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates curl \
	&& rm -rf /var/lib/apt/lists/* \
	&& useradd --uid 10001 --system --no-create-home rsapilab

COPY --from=builder /out/rs-http-apilab /app/rs-http-apilab

USER 10001:10001
ENV PORT=8080
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
	CMD curl -fsS "http://127.0.0.1:${PORT:-8080}/health" || exit 1

ENTRYPOINT ["/app/rs-http-apilab"]
