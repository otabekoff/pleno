#!/usr/bin/env bash
# Starts examples/http_server.pln and examples/web_app/app.pln and checks them
# over real HTTP with curl.
# Usage: scripts/test_http_server.sh [path/to/pleno]
set -u

root="$(cd "$(dirname "$0")/.." && pwd)"
pleno="${1:-$root/target/debug/pleno}"
case "$pleno" in /*) ;; *) pleno="$(pwd)/$pleno" ;; esac
base="http://127.0.0.1:8080"
app="http://127.0.0.1:8081"
log="$(mktemp)"
app_log="$(mktemp)"
jar="$(mktemp)"

"$pleno" run "$root/examples/http_server.pln" > "$log" 2>&1 &
server_pid=$!
# The web app serves ./public, so it runs from its own folder.
(cd "$root/examples/web_app" && exec "$pleno" run app.pln) > "$app_log" 2>&1 &
app_pid=$!
trap 'kill "$server_pid" "$app_pid" 2>/dev/null; rm -f "$log" "$app_log" "$jar"' EXIT

for url in "$base/" "$app/"; do
    for _ in $(seq 1 30); do
        curl -s -o /dev/null "$url" && break
        sleep 0.5
    done
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
status() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
# The value of a response header, without the trailing CR.
header() {
    local name="$1"
    shift
    curl -s -D - -o /dev/null "$@" | tr -d '\r' | sed -n "s/^$name: //Ip" | head -n 1
}

# examples/http_server.pln: routes, JSON, async handlers.
check "GET / body" "Hello, Pleno!" "$(curl -s "$base/")"
check "GET /users/7 body" '{"id":"7","name":"User 7"}' "$(curl -s "$base/users/7")"
check "GET /users/7 content type" "application/json" \
    "$(curl -s -o /dev/null -w '%{content_type}' "$base/users/7")"
check "POST /users status" "201" "$(status -X POST -d '{"name":"Ada"}' "$base/users")"
check "POST /users body" '{"name":"Ada"}' "$(curl -s -X POST -d '{"name":"Ada"}' "$base/users")"
check "async handler" "done after 10ms" "$(curl -s "$base/delayed")"
check "query string" "done after 25ms" "$(curl -s "$base/delayed?ms=25")"
check "unknown route status" "404" "$(status "$base/missing")"
check "HEAD has no body" "" "$(curl -s -I "$base/" | tr -d '\r' | sed '1,/^$/d')"

# examples/web_app: middleware, static files, cookies, the error handler.
check "static index.html" "text/html; charset=utf-8" \
    "$(curl -s -o /dev/null -w '%{content_type}' "$app/")"
check "static css" "text/css; charset=utf-8" \
    "$(curl -s -o /dev/null -w '%{content_type}' "$app/css/style.css")"
etag="$(header ETag "$app/css/style.css")"
check "static 304 for a known ETag" "304" "$(status -H "If-None-Match: $etag" "$app/css/style.css")"
check "static HEAD length" "$(curl -s "$app/css/style.css" | wc -c | tr -d ' ')" \
    "$(header Content-Length -I "$app/css/style.css")"
check "traversal with .." "403" "$(status --path-as-is "$app/../secret.txt")"
check "traversal with %2e%2e" "403" "$(status --path-as-is "$app/%2e%2e/app.pln")"
check "traversal with %5c" "403" "$(status --path-as-is "$app/..%5capp.pln")"
check "middleware adds a header" "1" \
    "$(header X-Response-Time "$app/" | grep -c 'us$')"
check "/api without a cookie" "401" "$(status "$app/api/notes")"
check "login sets a cookie" "user=Ada%20L; Max-Age=3600; Path=/; HttpOnly; SameSite=Lax" \
    "$(header Set-Cookie -X POST -d 'Ada L' -c "$jar" "$app/login")"
check "/api with the cookie" '{"user":"Ada L","notes":[{"id":1,"text":"Buy milk"}]}' \
    "$(curl -s -b "$jar" "$app/api/notes")"
check "POST a note" "201" "$(status -b "$jar" -X POST -d '{"text":"Call Bob"}' "$app/api/notes")"
check "onError answers bad JSON" '{"error":"the body is not valid JSON"}' \
    "$(curl -s -b "$jar" -X POST -d '{bad' "$app/api/notes")"
check "async route" "slow but fine" "$(curl -s -b "$jar" "$app/api/slow")"
check "logout clears the cookie" "user=; Max-Age=0" \
    "$(header Set-Cookie -X POST "$app/logout" | cut -d';' -f1-2)"

if [ "$failures" -gt 0 ]; then
    echo "--- examples/http_server.pln output:"
    cat "$log"
    echo "--- examples/web_app/app.pln output:"
    cat "$app_log"
    exit 1
fi
