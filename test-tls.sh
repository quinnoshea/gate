#!/bin/bash

# Test HTTPS SNI proxy from relay to daemon for inference
# Usage: ./test-tls.sh [daemon-node-id]

# Function to read daemon node ID from logs
read_daemon_node_id() {
    local daemon_log="${1:-.state/daemon.log}"
    
    if [[ ! -f "$daemon_log" ]]; then
        echo "Error: Daemon log not found at $daemon_log" >&2
        return 1
    fi
    
    # Extract the short domain prefix from the daemon logs
    grep "Short domain:" "$daemon_log" | tail -1 | sed 's/.*Short domain: \([a-f0-9]*\)\.private\.hellas\.ai.*/\1/'
}

DAEMON_NODE_ID="${1:-$(read_daemon_node_id)}"
if [[ -z "$DAEMON_NODE_ID" ]]; then
    # Fallback to hardcoded current node ID (first 16 chars)
    DAEMON_NODE_ID="3818e20a7b12092e"
fi
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
  -d '{"model":"gemma-2-2b-it","messages":[{"role":"user","content":"Hello! Can you tell me a short joke?"}],"max_tokens":100,"temperature":0.7}' \
  -v \
  "https://$DAEMON_DOMAIN:$RELAY_PORT/v1/chat/completions"
