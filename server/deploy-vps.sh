#!/usr/bin/env bash
set -e

# Deploy to a VPS via SSH
#
# DO NOT CALL DIRECTLY - use: mise run deploy
#
# This script expects the binary to be pre-built by mise.
# The target host is "palanthas" (configured in ~/.ssh/config or /etc/hosts).

if [ "$CALLED_FROM_MISE" != "1" ]; then
    echo "ERROR: Don't call this script directly."
    echo ""
    echo "Use:  mise run deploy"
    echo ""
    echo "(This ensures the build runs first)"
    exit 1
fi

DOMAIN="sven.guru"
BINARY="target/release/static-server"
VPS_HOST="${VPS_HOST:-palanthas}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo_info()  { echo -e "${GREEN}==>${NC} $1"; }
echo_warn()  { echo -e "${YELLOW}WARNING:${NC} $1"; }
echo_error() { echo -e "${RED}ERROR:${NC} $1"; exit 1; }

# Verify the binary exists
verify_binary() {
    if [ ! -f "$BINARY" ]; then
        echo_error "Binary not found at $BINARY. Run 'mise run build' first."
    fi
    local size
    size=$(du -h "$BINARY" | cut -f1)
    echo_info "Binary: $BINARY ($size)"
}

# Upload binary and restart service
deploy() {
    echo_info "Uploading binary to $VPS_HOST..."
    scp "$BINARY" "root@${VPS_HOST}:/tmp/static-server.new"

    echo_info "Installing binary and restarting service..."
    ssh "root@${VPS_HOST}" bash -s <<'REMOTE'
set -e

systemctl stop homepage || true
mv /tmp/static-server.new /opt/homepage/static-server
chown homepage:homepage /opt/homepage/static-server
chmod +x /opt/homepage/static-server
systemctl start homepage

# Wait for it to come up
echo "Waiting for server to start..."
for i in $(seq 1 30); do
    if curl -sf --connect-timeout 2 http://localhost:80/ > /dev/null 2>&1; then
        echo "Server is up!"
        exit 0
    fi
    sleep 1
done

echo "WARNING: Server did not respond within 30s"
journalctl -u homepage --no-pager -n 20
exit 1
REMOTE
}

# Verify the site is reachable externally
verify() {
    echo_info "Verifying https://$DOMAIN ..."
    local status
    status=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "https://$DOMAIN/" 2>/dev/null || echo "000")

    if [ "$status" = "200" ]; then
        echo_info "Site is live (HTTP $status)"
    else
        echo_warn "Got HTTP $status from https://$DOMAIN — may need a moment for cert acquisition"
    fi
}

main() {
    echo_info "Deploying $DOMAIN to $VPS_HOST"
    echo ""

    verify_binary
    deploy
    verify

    echo ""
    echo_info "Deployment complete!"
    echo_info "  Host: $VPS_HOST"
    echo_info "  URL:  https://$DOMAIN"
}

main
