# auth-stack — patch overlay over Quozul/PicoLimbo.
#
# This Dockerfile clones a configurable ref of upstream PicoLimbo, applies
# the patch series in patches/, and builds the resulting pico_limbo binary
# into a distroless runtime image. The repo holds no upstream code; the
# build is reproducibly defined by (PICOLIMBO_REF, patches/).
#
# Build (local default = the last-known-good fallback below):
#   docker build -t picolimbo:local -f docker/Dockerfile .
#
# Build against a specific upstream ref:
#   docker build --build-arg PICOLIMBO_REF=v1.13.1+mc26.2 \
#                -t picolimbo:test -f docker/Dockerfile .
#
# In CI, .github/workflows/upstream-track.yml overrides PICOLIMBO_REF
# with the latest upstream release tag (queried from the GitHub API on
# every schedule/push). The ARG below is only used as the fallback when
# that API call fails AND when you build locally without --build-arg.
# Keep it pointing at a ref that's known to apply cleanly.

# syntax=docker/dockerfile:1.7-labs

ARG PICOLIMBO_REF=v1.12.2+mc26.1.2

# Stage 1: fetch upstream and apply patches.
# Done in a tiny image so patch-application failure is fast and obvious.
FROM alpine/git:latest AS source
ARG PICOLIMBO_REF
WORKDIR /src
RUN git clone https://github.com/Quozul/PicoLimbo.git . \
 && git checkout "${PICOLIMBO_REF}"
COPY patches/ /patches/
RUN set -e; \
    if ls /patches/*.patch >/dev/null 2>&1; then \
        echo "Applying patches against ${PICOLIMBO_REF}:"; \
        ls /patches/*.patch; \
        git apply --check /patches/*.patch; \
        git apply /patches/*.patch; \
    else \
        echo "No patches found in /patches/"; \
    fi

# Stage 2: build the patched source.
# rust:1-alpine floats with stable 1.x; the cloned repo's rust-toolchain.toml
# takes precedence via rustup so we match whatever upstream pins without
# editing the Dockerfile.
FROM rust:1-alpine AS builder
ARG TARGETPLATFORM
ARG BINARY_NAME=pico_limbo
WORKDIR /usr/src/app
COPY --from=source /src .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/app/target \
    apk add --no-cache musl-dev && \
    case "${TARGETPLATFORM:-linux/amd64}" in \
        linux/amd64) TARGET="x86_64-unknown-linux-musl";; \
        linux/arm64) TARGET="aarch64-unknown-linux-musl";; \
        *) echo "Unsupported platform: ${TARGETPLATFORM}"; exit 1;; \
    esac && \
    rustup target add "$TARGET" && \
    cargo build --release --target "$TARGET" --bin "$BINARY_NAME" && \
    cp "target/$TARGET/release/$BINARY_NAME" /usr/local/bin/pico_limbo

# Stage 3: build a tiny scratch dir owned by nonroot so the distroless
# final image has a writable WORKDIR. Distroless has no shell, so we
# can't mkdir/chown there; we have to stage the dir and COPY it in.
FROM alpine AS workdir-stage
WORKDIR /usr/src/app
RUN touch /usr/src/app/.keep

FROM gcr.io/distroless/static:latest
COPY --from=workdir-stage --chown=nonroot:nonroot /usr/src/app /usr/src/app
COPY --from=builder       --chown=nonroot:nonroot /usr/local/bin/pico_limbo /usr/local/bin/pico_limbo
USER nonroot
WORKDIR /usr/src/app
CMD ["pico_limbo"]
