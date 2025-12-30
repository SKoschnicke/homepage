# Multi-stage build for minimal image size
FROM rust:1.82-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev gcc

WORKDIR /build

# Copy the entire project
COPY .. /build

# Build the Hugo site first
RUN apk add --no-cache hugo && \
    hugo --minify

# Build the Rust server
WORKDIR /build/server
RUN cargo build --release

# Runtime stage - minimal scratch image
FROM scratch

# Copy the binary
COPY --from=builder /build/server/target/release/static-server /static-server

# Server runs on port 3000
EXPOSE 3000

# Run the server
ENTRYPOINT ["/static-server"]
