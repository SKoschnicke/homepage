#!/usr/bin/env bash
# Helper script to get Hetzner Cloud DNS zone and record IDs
#
# Usage: HCLOUD_TOKEN=xxx ./get-dns-info.sh

if [ -z "$HCLOUD_TOKEN" ]; then
    echo "ERROR: HCLOUD_TOKEN environment variable not set"
    echo ""
    echo "Get your token at: https://console.hetzner.cloud/"
    echo "Security → API Tokens → Generate API token (Read & Write)"
    echo ""
    echo "Usage: HCLOUD_TOKEN=xxx ./get-dns-info.sh"
    exit 1
fi

DOMAIN="sven.guru"

echo "Fetching DNS zones from Hetzner Cloud API..."
zones=$(curl -s "https://api.hetzner.cloud/v1/zones" \
    -H "Authorization: Bearer $HCLOUD_TOKEN")

# Extract zone ID for the domain
zone_id=$(echo "$zones" | grep -B2 "\"name\":\"$DOMAIN\"" | grep '"id"' | head -1 | grep -o '[0-9]*')

if [ -z "$zone_id" ]; then
    echo "ERROR: Could not find zone for $DOMAIN"
    echo ""
    echo "Response:"
    echo "$zones"
    exit 1
fi

echo "Found zone: $DOMAIN"
echo "  Zone ID: $zone_id"
echo ""

echo "Fetching DNS records for zone..."
records=$(curl -s "https://api.hetzner.cloud/v1/zones/$zone_id/records" \
    -H "Authorization: Bearer $HCLOUD_TOKEN")

# Find the A record for @ (root domain)
record_id=$(echo "$records" | grep -A10 '"name":"@"' | grep -A5 '"type":"A"' | grep '"id"' | head -1 | grep -o '[0-9]*')
current_value=$(echo "$records" | grep -A10 "\"id\":$record_id" | grep '"records"' | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+\.[0-9]\+' | head -1)

if [ -z "$record_id" ]; then
    echo "ERROR: Could not find A record for $DOMAIN"
    echo ""
    echo "Records:"
    echo "$records"
    exit 1
fi

echo "Found A record:"
echo "  Record ID: $record_id"
echo "  Current IP: $current_value"
echo ""
echo "Export these variables for deployment:"
echo ""
echo "export DNS_ZONE_ID=$zone_id"
echo "export DNS_RECORD_ID=$record_id"
