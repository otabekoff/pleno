#!/usr/bin/env bash
# Runs every .pln script under tests/ and examples/ and checks it against the
# expectations declared in comments at the top of the file:
#
#   // expect-error: <text>   must exit non-zero with <text> in its output
#   // args: <flags>          extra flags for `pleno run`
#   // stdin: <text>          fed to the script; `\n` separates lines
#   // skip: <reason>         not run (for example, a module other tests import)
#   // requires: network      run only when PLENO_NETWORK_TESTS=1
#
# Scripts run from their own directory, so relative paths behave as in normal use.
# Usage: scripts/run_pln_tests.sh [path/to/pleno]
set -u

root="$(cd "$(dirname "$0")/.." && pwd)"
pleno="${1:-$root/target/debug/pleno}"
case "$pleno" in /*) ;; *) pleno="$(pwd)/$pleno" ;; esac
timeout_secs="${PLENO_TEST_TIMEOUT:-30}"

passed=0
failed=0
skipped=0
failures=()

# Prints the value of `// <name>: <value>` from the first lines of a script.
directive() {
    head -n 15 "$1" | sed -n "s#^// $2: ##p" | head -n 1 | tr -d '\r'
}

while IFS= read -r file; do
    rel="${file#"$root"/}"
    skip_reason="$(directive "$file" skip)"
    if [ -n "$skip_reason" ]; then
        skipped=$((skipped + 1))
        continue
    fi
    if [ "$(directive "$file" requires)" = "network" ] && [ "${PLENO_NETWORK_TESTS:-0}" != "1" ]; then
        skipped=$((skipped + 1))
        continue
    fi

    expect_error="$(directive "$file" expect-error)"
    args="$(directive "$file" args)"
    stdin_text="$(directive "$file" stdin)"

    # shellcheck disable=SC2086 # `args` holds several flags
    output="$(cd "$(dirname "$file")" &&
        printf '%b' "$stdin_text" |
        timeout "$timeout_secs" "$pleno" run $args "$(basename "$file")" 2>&1)"
    status=$?
    output="$(printf '%s' "$output" | sed 's/\x1b\[[0-9;]*m//g')"

    if [ -n "$expect_error" ]; then
        if [ "$status" -ne 0 ] && [ "$status" -ne 124 ] && printf '%s' "$output" | grep -qF -- "$expect_error"; then
            passed=$((passed + 1))
        else
            failed=$((failed + 1))
            failures+=("$rel: expected error '$expect_error' (exit $status)")
        fi
    elif [ "$status" -eq 0 ]; then
        passed=$((passed + 1))
    else
        failed=$((failed + 1))
        reason="$(printf '%s\n' "$output" | grep -m 1 -E 'Error|panicked' || true)"
        [ "$status" -eq 124 ] && reason="timed out after ${timeout_secs}s"
        failures+=("$rel: exit $status: $reason")
    fi
done < <(find "$root/tests" "$root/examples" -name '*.pln' | sort)

echo "pln scripts: $passed passed, $failed failed, $skipped skipped"
if [ "$failed" -gt 0 ]; then
    printf '  FAIL %s\n' "${failures[@]}"
    exit 1
fi
