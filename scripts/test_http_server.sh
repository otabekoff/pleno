#!/usr/bin/env bash
# Starts examples/http_server.pln and checks its routes over real HTTP.
# Usage: scripts/test_http_server.sh [path/to/pleno]
set -u

root="$(cd "$(dirname "$0")/.." && pwd)"
pleno="${1:-$root/target/debug/pleno}"
base="http://127.0.0.1:8080"
log="$(mktemp)"

"$pleno" run "$root/examples/http_server.pln" > "$log" 2>&1 &
server_pid=$!
trap 'kill "$server_pid" 2>/dev/null; rm -f "$log"' EXIT

for _ in $(seq 1 30); do
    curl -s -o /dev/null "$base/" && break
    sleep 0.5
done

failures=0
check() {
    local description="$1" expected="$2" actual="$3"
    if [ "$actual" = "$expected" ]; then
        echo "ok   $description"
    else
        echo "FAIL $description: expected '$expected', got '$actual'"
        failures=$((failures + 1))
    fi
}

check "GET / body" "Hello, Pleno!" "$(curl -s "$base/")"
check "GET /users/7 body" '{"id":"7","name":"User 7"}' "$(curl -s "$base/users/7")"
check "GET /users/7 content type" "application/json" \
    "$(curl -s -o /dev/null -w '%{content_type}' "$base/users/7")"
check "POST /users status" "201" \
    "$(curl -s -o /dev/null -w '%{http_code}' -X POST -d '{"name":"Ada"}' "$base/users")"
check "POST /users body" '{"name":"Ada"}' "$(curl -s -X POST -d '{"name":"Ada"}' "$base/users")"
check "unknown route status" "404" "$(curl -s -o /dev/null -w '%{http_code}' "$base/missing")"

if [ "$failures" -gt 0 ]; then
    echo "--- server output:"
    cat "$log"
    exit 1
fi
