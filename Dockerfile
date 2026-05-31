# syntax=docker/dockerfile:1
#
# Headless Tag Editor web server image.
#
# Build it for the NAS/Pi architecture with buildx (runs the build under QEMU, so
# a plain native `cargo build` yields an ARM binary with no cross toolchain):
#
#   docker buildx build --platform linux/arm64    -t tag-editor:latest --load .   # 64-bit OS
#   docker buildx build --platform linux/arm/v7   -t tag-editor:latest --load .   # 32-bit OS
#
# ...or just `docker build -t tag-editor:latest .` directly on the Pi/OMV itself.

# ---- Build stage --------------------------------------------------------------
FROM rust:1-bookworm AS build
WORKDIR /src
# Cargo manifests first so dependency compilation is cached across source edits.
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets
# --no-default-features drops eframe/winit/GTK — only the pure-Rust server remains.
RUN cargo build --release --no-default-features \
    && cp target/release/tag_editor /tag_editor

# ---- Runtime stage ------------------------------------------------------------
# debian-slim matches OpenMediaVault's Debian base and provides glibc. The server
# has no other runtime dependencies (image decoding etc. are pure Rust).
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=build /tag_editor /app/tag_editor
# Default web port; override via the mounted /app/settings.json.
EXPOSE 47823
ENTRYPOINT ["/app/tag_editor", "--server"]
