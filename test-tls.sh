#!/bin/bash

# Test HTTPS SNI proxy from relay to daemon for inference
# Usage: ./test-tls.sh [daemon-node-id]

FULL_NODE_ID="${1:-3818e20a7b12092e7b07dfe7be92fb54fbc1383cd32e3d0ca3e44288ce9d34e4}"
# Use first 32 chars (16 bytes) to match daemon's certificate generation
DAEMON_NODE_ID="${FULL_NODE_ID:0:32}"
RELAY_PORT="8443"

echo "Testing HTTPS SNI proxy to daemon..."
echo "Daemon Node ID: $DAEMON_NODE_ID"
echo "Relay Port: $RELAY_PORT"
echo "SNI Hostname: $DAEMON_NODE_ID"
echo ""

DAEMON_DOMAIN="$DAEMON_NODE_ID.private.hellas.ai"

curl -k \
  --resolve "$DAEMON_DOMAIN:$RELAY_PORT:127.0.0.1" \
  -H "Content-Type: application/json" \
  -d '{"model":"llama-3.2-1b","messages":[{"role":"user","content":"Hello"}],"max_tokens":50}' \
  -v \
  "https://$DAEMON_DOMAIN:$RELAY_PORT/inference/chat/completions"
