#!/usr/bin/env bash
set -e

# Deployment script for Hetzner Cloud with automatic DNS update
#
# Secrets are loaded from 1Password CLI.
# Required 1Password items:
#   - "Hetzner Cloud API Token" (in Private vault) - for instance and DNS management
#   - "Hetzner Object Storage S3 credentials" (in Private vault) - for cert storage
#
# Make sure you're signed in: eval $(op signin)

OPS="${HOME}/.local/bin/ops"
CONFIG="config-hetzner.json"
IMAGE_NAME="homepage-unikernel"
DOMAIN="sven.guru"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_info() {
    echo -e "${GREEN}==>${NC} $1"
}

echo_warn() {
    echo -e "${YELLOW}WARNING:${NC} $1"
}

echo_error() {
    echo -e "${RED}ERROR:${NC} $1"
    exit 1
}

# Load secrets from 1Password
load_secrets() {
    echo_info "Loading secrets from 1Password..."

    # Check if op CLI is available
    if ! command -v op &> /dev/null; then
        echo_error "1Password CLI (op) not found. Install it from: https://1password.com/downloads/command-line/"
    fi

    # Check if signed in
    if ! op account list &> /dev/null; then
        echo_error "Not signed in to 1Password. Run: eval \$(op signin)"
    fi

    # Load secrets from 1Password
    export HCLOUD_TOKEN=$(op read "op://Private/Hetzner Cloud API Token/password" 2>/dev/null || echo "")
    export OBJECT_STORAGE_DOMAIN=$(op read "op://Private/Hetzner Object Storage S3 credentials/endpoint" 2>/dev/null || echo "")
    export OBJECT_STORAGE_KEY=$(op read "op://Private/Hetzner Object Storage S3 credentials/add more/Access Key" 2>/dev/null || echo "")
    export OBJECT_STORAGE_SECRET=$(op read "op://Private/Hetzner Object Storage S3 credentials/password" 2>/dev/null || echo "")

    # Validate all secrets were loaded
    local missing=()
    [ -z "$HCLOUD_TOKEN" ] && missing+=("op://Private/Hetzner Cloud API Token/password")
    [ -z "$OBJECT_STORAGE_DOMAIN" ] && missing+=("op://Private/Hetzner Object Storage S3 credentials/endpoint")
    [ -z "$OBJECT_STORAGE_KEY" ] && missing+=("op://Private/Hetzner Object Storage S3 credentials/add more/Access Key")
    [ -z "$OBJECT_STORAGE_SECRET" ] && missing+=("op://Private/Hetzner Object Storage S3 credentials/password")

    if [ ${#missing[@]} -gt 0 ]; then
        echo_error "Missing secrets in 1Password: ${missing[*]}"
    fi

    echo_info "Secrets loaded successfully"
}

# Get DNS zone ID from Cloud API
get_dns_zone_id() {
    echo_info "Fetching DNS zone ID for $DOMAIN..."

    local zones=$(curl -s "https://api.hetzner.cloud/v1/zones" \
        -H "Authorization: Bearer $HCLOUD_TOKEN")

    DNS_ZONE_ID=$(echo "$zones" | python3 -c "import sys, json; zones = json.load(sys.stdin)['zones']; print(next((z['id'] for z in zones if z['name'] == '$DOMAIN'), ''))" 2>/dev/null)

    if [ -z "$DNS_ZONE_ID" ]; then
        echo_error "Could not find DNS zone for $DOMAIN. Please create it in Hetzner Console first."
    fi

    echo_info "Found DNS zone: $DOMAIN (ID: $DNS_ZONE_ID)"
}

# Delete existing instances
cleanup_instances() {
    echo_info "Checking for existing instances..."
    local instances=$($OPS instance list -t hetzner -c $CONFIG 2>/dev/null | grep 'homepage-unikernel-' | awk '{print $2}' || true)

    if [ -n "$instances" ]; then
        echo "$instances" | while read -r instance; do
            if [ -n "$instance" ] && [ "$instance" != "NAME" ]; then
                echo_info "Deleting instance: $instance"
                echo "yes" | $OPS instance delete "$instance" -t hetzner -c $CONFIG
            fi
        done
    else
        echo_info "No existing instances found"
    fi
}

# Delete existing image
cleanup_image() {
    echo_info "Checking for existing image..."
    if $OPS image list -t hetzner -c $CONFIG 2>/dev/null | grep -q "$IMAGE_NAME"; then
        echo_info "Deleting existing image: $IMAGE_NAME"
        echo "yes" | $OPS image delete "$IMAGE_NAME" -t hetzner -c $CONFIG
    else
        echo_info "No existing image found"
    fi
}

# Inject S3 credentials into config
inject_s3_credentials() {
    echo_info "Injecting S3 credentials into config..."

    # Create temporary config with real credentials
    TEMP_CONFIG=$(mktemp /tmp/config-hetzner-XXXXXX.json)

    # Replace placeholder values with real credentials
    cat $CONFIG | \
        sed "s|PLACEHOLDER_WILL_BE_SET_BY_DEPLOY_SCRIPT|${OBJECT_STORAGE_KEY}|" | \
        jq --arg secret "$OBJECT_STORAGE_SECRET" \
           --arg endpoint "https://$OBJECT_STORAGE_DOMAIN" \
           '.Env.S3_SECRET_KEY = $secret | .Env.S3_ENDPOINT = $endpoint' \
        > "$TEMP_CONFIG"

    CONFIG="$TEMP_CONFIG"
    echo_info "Using temporary config: $CONFIG"
}

# Create new unikernel image
create_image() {
    echo_info "Creating unikernel image..."
    $OPS image create target/release/static-server -c $CONFIG -t hetzner -i "$IMAGE_NAME" 2>&1 | grep -v "^\[" | grep -E "(created|error|Error|failed|Failed)" || true
    echo_info "Image creation complete"
}

# Create new instance
create_instance() {
    echo_info "Creating instance..."
    local output=$($OPS instance create "$IMAGE_NAME" -t hetzner -c $CONFIG 2>&1 | grep -v "^\[")
    echo "$output" | grep -E "(created|error|Error|failed|Failed)"

    # Extract instance name from output
    INSTANCE_NAME=$(echo "$output" | grep "created..." | sed "s/hetzner instance '\(.*\)' created.../\1/")

    if [ -z "$INSTANCE_NAME" ]; then
        echo_error "Failed to extract instance name"
    fi

    echo_info "Instance created: $INSTANCE_NAME"
}

# Start the instance
start_instance() {
    echo_info "Starting instance..."
    $OPS instance start "$INSTANCE_NAME" -t hetzner -c $CONFIG 2>&1 | grep -v "^\[" | grep -E "(started|error|Error|failed|Failed)" || true
    echo_info "Instance start command sent"
    sleep 5  # Give it a moment to actually start
}

# Get instance IP and verify instance is running
get_instance_ip() {
    echo_info "Getting instance IP..."

    # Wait for instance to get an IP (max 30 attempts = 60 seconds)
    local attempt=0
    local max_attempts=30

    while [ $attempt -lt $max_attempts ]; do
        local instance_info=$($OPS instance list -t hetzner -c $CONFIG | grep "$INSTANCE_NAME")
        IP=$(echo "$instance_info" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' | head -1)

        # Check if we got a valid IP
        if [ -n "$IP" ]; then
            echo_info "Instance IP: $IP"

            # Verify instance appears in list (basic health check)
            if echo "$instance_info" | grep -q "$INSTANCE_NAME"; then
                echo_info "Instance is created and listed"
                return 0
            fi
        fi

        attempt=$((attempt + 1))
        echo "  Waiting for IP assignment... (attempt $attempt/$max_attempts)"
        sleep 2
    done

    echo_error "Failed to get instance IP after ${max_attempts} attempts"
    echo_error "Instance may have failed to start. Check Hetzner Cloud Console for details."
    exit 1
}

# Update or create DNS record via Hetzner Cloud API
update_dns() {
    echo_info "Checking for existing A record for $DOMAIN..."

    # Check if A record exists
    local check_response=$(curl -s "https://api.hetzner.cloud/v1/zones/$DNS_ZONE_ID/rrsets/@/A" \
        -H "Authorization: Bearer $HCLOUD_TOKEN")

    if echo "$check_response" | grep -q '"rrset"'; then
        # Record exists, update it using set_records action
        echo_info "Updating existing DNS A record for $DOMAIN -> $IP"

        local response=$(curl -s -X POST "https://api.hetzner.cloud/v1/zones/$DNS_ZONE_ID/rrsets/@/A/actions/set_records" \
            -H "Authorization: Bearer $HCLOUD_TOKEN" \
            -H "Content-Type: application/json" \
            -d "{
                \"records\": [
                    {\"value\": \"$IP\", \"comment\": \"Deployed by deploy-hetzner.sh\"}
                ]
            }")

        # Actions endpoint returns {"action": {...}} - check if action was created successfully
        if ! echo "$response" | grep -q '"action"'; then
            echo_error "DNS update failed: $response"
        fi

        echo_info "DNS record updated successfully"
    else
        # Record doesn't exist, create it
        echo_info "Creating new DNS A record for $DOMAIN -> $IP"

        local response=$(curl -s -X POST "https://api.hetzner.cloud/v1/zones/$DNS_ZONE_ID/rrsets" \
            -H "Authorization: Bearer $HCLOUD_TOKEN" \
            -H "Content-Type: application/json" \
            -d "{
                \"name\": \"@\",
                \"type\": \"A\",
                \"ttl\": 60,
                \"records\": [
                    {\"value\": \"$IP\", \"comment\": \"Deployed by deploy-hetzner.sh\"}
                ]
            }")

        if echo "$response" | grep -q '"error"' && ! echo "$response" | grep -q '"rrset"'; then
            echo_error "DNS creation failed: $response"
        fi

        echo_info "DNS record created successfully"
    fi
}

# Wait for DNS propagation
wait_for_dns() {
    echo_info "Waiting for DNS propagation..."
    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        local resolved_ip=$(host "$DOMAIN" 2>/dev/null | grep "has address" | head -1 | awk '{print $4}')

        if [ "$resolved_ip" = "$IP" ]; then
            echo_info "DNS propagated successfully: $DOMAIN -> $IP"
            return 0
        fi

        attempt=$((attempt + 1))
        echo "  Waiting... (attempt $attempt/$max_attempts, current: $resolved_ip, expected: $IP)"
        sleep 2
    done

    echo_warn "DNS propagation timeout. Current IP: $resolved_ip, Expected: $IP"
    echo_warn "Server may take longer to acquire certificate"
}

# Wait for server to respond
wait_for_server() {
    echo_info "Waiting for server to start (this may take 30-60s for cert acquisition)..."
    local max_attempts=60
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        # Check if server responds (accept any HTTP response including 404)
        if curl -s -I --connect-timeout 2 "http://$IP" >/dev/null 2>&1; then
            echo_info "Server is responding!"
            return 0
        fi

        attempt=$((attempt + 1))
        echo "  Waiting... (attempt $attempt/$max_attempts)"
        sleep 2
    done

    echo_error "Server failed to start after 2 minutes"
    echo_error "Instance created but not responding on port 80. Check Hetzner Cloud Console for details."
    exit 1
}

# Main deployment flow
main() {
    echo_info "Starting Hetzner deployment for $DOMAIN"
    echo ""

    load_secrets
    get_dns_zone_id
    inject_s3_credentials
    cleanup_instances
    cleanup_image
    create_image
    create_instance
    start_instance
    get_instance_ip
    update_dns
    wait_for_dns
    wait_for_server

    # Cleanup temp config
    if [ -f "$TEMP_CONFIG" ]; then
        rm -f "$TEMP_CONFIG"
    fi

    echo ""
    echo_info "Deployment complete!"
    echo_info "  Instance: $INSTANCE_NAME"
    echo_info "  IP: $IP"
    echo_info "  HTTP: http://$DOMAIN"
    echo_info "  HTTPS: https://$DOMAIN"
    echo ""
    echo_info "Test with:"
    echo "  curl -I http://$IP"
    echo "  curl -I http://$DOMAIN"
}

main
