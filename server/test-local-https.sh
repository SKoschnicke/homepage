#!/usr/bin/env bash
set -e

echo "==> Starting local HTTPS test environment"

# Start MinIO in background with podman
echo "==> Starting MinIO (S3 mock)..."
podman stop minio-test 2>/dev/null || true
podman rm minio-test 2>/dev/null || true
podman run -d --name minio-test \
    -p 9000:9000 \
    -p 9001:9001 \
    -e MINIO_ROOT_USER=minioadmin \
    -e MINIO_ROOT_PASSWORD=minioadmin \
    minio/minio:latest \
    server /data --console-address ":9001"

# Wait for MinIO to be ready
echo "==> Waiting for MinIO to be ready..."
until curl -s http://localhost:9000/minio/health/live > /dev/null 2>&1; do
    echo "  Waiting..."
    sleep 1
done
echo "  ✓ MinIO is ready"

# Create bucket if it doesn't exist
echo "==> Creating S3 bucket..."
podman run --rm --network host \
    -e AWS_ACCESS_KEY_ID=minioadmin \
    -e AWS_SECRET_ACCESS_KEY=minioadmin \
    docker.io/amazon/aws-cli \
    --endpoint-url http://localhost:9000 \
    s3 mb s3://homepage-certs 2>/dev/null || echo "  Bucket already exists"

echo ""
echo "==> MinIO Console: http://localhost:9001 (minioadmin/minioadmin)"
echo "==> Starting Rust server with HTTPS..."
echo ""

# Run the server with HTTPS enabled using unprivileged ports
export ENABLE_HTTPS=true
export LOCAL_DEV=true
export DOMAIN=localhost
export ACME_CONTACT_EMAIL=test@localhost
export ACME_STAGING=true
export S3_ENDPOINT=http://localhost:9000
export S3_BUCKET=homepage-certs
export S3_ACCESS_KEY=minioadmin
export S3_SECRET_KEY=minioadmin
export S3_REGION=us-east-1
export RUST_BACKTRACE=full
export PORT=8080

echo "Server will listen on:"
echo "  HTTP:  http://localhost:8080"
echo "  HTTPS: https://localhost:8443"
echo ""

cargo run --release
