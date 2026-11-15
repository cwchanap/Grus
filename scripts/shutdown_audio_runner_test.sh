#!/usr/bin/env bash
# Contract test for scripts/shutdown_audio_test.sh. A stub Godot proves the
# runner (a) rejects invalid/zero run counts before launching any process,
# (b) keeps a unique log directory per invocation so retries never overwrite
# earlier failures, and (c) still gates on exit code, success marker and
# shutdown leak diagnostics. One real focused-Godot shutdown run follows when
# a real Godot binary is available, so the shell contract is backed by an
# actual engine process.
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner="$repo_root/scripts/shutdown_audio_test.sh"
real_godot="$(command -v godot || true)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/bin" "$work/logs"
cat > "$work/bin/godot" <<'EOF'
#!/usr/bin/env bash
printf 'call\n' >> "${GRUS_STUB_CALLS:?}"
case "${GRUS_STUB_MODE:-clean}" in
    clean)   echo "GRUS_GODOT_SMOKE_OK stub run"; exit 0 ;;
    leak)    echo "GRUS_GODOT_SMOKE_OK stub run"
             echo "WARNING: 4 ObjectDB instances were leaked at exit"
             echo "ERROR: 2 resources still in use at exit"
             echo "Leaked instance: AudioStreamOggVorbis:1 - Reference count: 1"
             exit 0 ;;
    fail)    echo "GRUS_GODOT_SMOKE_OK stub run"; exit 3 ;;
    nomarker) echo "no marker here"; exit 0 ;;
    *)       echo "unknown stub mode" >&2; exit 9 ;;
esac
EOF
chmod +x "$work/bin/godot"

export GODOT_BIN="$work/bin/godot"
export GRUS_SHUTDOWN_LOG_DIR="$work/logs"
export GRUS_STUB_CALLS="$work/stub-calls.txt"
: > "$GRUS_STUB_CALLS"

failures=0
fail() {
    echo "FAIL: $*" >&2
    failures=$((failures + 1))
}
godot_calls() {
    wc -l < "$GRUS_STUB_CALLS" | tr -d ' '
}
run_stub() {
    local runs="$1" mode="$2"
    GRUS_RUNS="$runs" GRUS_STUB_MODE="$mode" bash "$runner" >"$work/last.out" 2>&1
    return $?
}

# (a) Invalid and zero counts fail before any Godot process starts.
for bad in 0 -1 abc 1x '' ' 2'; do
    before="$(godot_calls)"
    if run_stub "$bad" clean; then
        fail "GRUS_RUNS='$bad' was accepted"
    fi
    [ "$(godot_calls)" -eq "$before" ] || fail "GRUS_RUNS='$bad' launched Godot"
    grep -q "GRUS_RUNS must be a positive integer" "$work/last.out" \
        || fail "GRUS_RUNS='$bad' did not explain the rejection"
done

# (b) Each valid invocation keeps its own directory; a failing invocation is
# preserved and the next clean retry cannot overwrite it.
: > "$GRUS_STUB_CALLS"
if ! run_stub 2 clean; then fail "a clean two-run invocation failed"; fi
grep -q "logs: " "$work/last.out" || fail "the runner did not print its log directory"
first_dir="$(sed -n 's/^logs: //p' "$work/last.out")"
if run_stub 1 leak; then fail "a leaking run was accepted"; fi
leak_out="$(cat "$work/last.out")"
second_dir="$(sed -n 's/^logs: //p' <<<"$leak_out")"
[ -n "$first_dir" ] && [ -n "$second_dir" ] || fail "a run did not report its log directory"
[ "$first_dir" != "$second_dir" ] || fail "two invocations reused the same log directory"
[ -f "$first_dir/run-2.log" ] || fail "the first invocation lost run-2.log"
grep -q "were leaked at exit" "$second_dir/run-1.log" \
    || fail "the failing invocation did not keep its raw leak output"
[ "$(godot_calls)" -eq 3 ] || fail "expected three stub processes, saw $(godot_calls)"

# (c) Exit code, marker and leak gates still decide the result.
if run_stub 1 fail; then fail "a nonzero Godot exit was accepted"; fi
grep -q "exited 3" "$work/last.out" || fail "the exit-code failure was not reported"
if run_stub 1 nomarker; then fail "a missing success marker was accepted"; fi
grep -q "never printed GRUS_GODOT_SMOKE_OK" "$work/last.out" \
    || fail "the marker failure was not reported"
if run_stub 1 leak; then fail "a leak diagnostic was accepted"; fi
grep -q "Leaked instance:" "$work/last.out" || fail "the leak detail was not printed"

# (d) One actual Godot shutdown regression, when the engine is available.
if [ -n "$real_godot" ]; then
    if ! GODOT_BIN="$real_godot" GRUS_RUNS=1 \
            GRUS_SHUTDOWN_SCENE=res://scenes/shutdown_quit_test.tscn \
            GRUS_SHUTDOWN_MARKER=GRUS_SHUTDOWN_QUIT_OK \
            GRUS_SHUTDOWN_LOG_DIR="$work/real" bash "$runner" >"$work/real.out" 2>&1; then
        fail "the real focused-Godot shutdown regression failed"
        cat "$work/real.out" >&2
    fi
else
    echo "skipping real Godot shutdown leg: no godot on PATH" >&2
fi

if [ "$failures" -ne 0 ]; then
    echo "shutdown runner contract: $failures failure(s)" >&2
    exit 1
fi
echo "shutdown runner contract: OK"
