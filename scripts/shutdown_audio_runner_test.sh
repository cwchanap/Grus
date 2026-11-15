#!/usr/bin/env bash
# Contract test for scripts/shutdown_audio_test.sh. A stub Godot proves the
# runner (a) rejects invalid, zero and oversized run counts before launching
# any process, (b) keeps a unique log directory per invocation so a later
# clean retry never overwrites an earlier failure, and (c) still gates on exit
# code, success marker and shutdown leak diagnostics. Real focused-Godot
# shutdown modes follow when a real Godot binary is available, so the shell
# contract is backed by actual engine processes; scratch logs are kept when
# anything fails.
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner="$repo_root/scripts/shutdown_audio_test.sh"
real_godot="$(command -v godot || true)"
work="$(mktemp -d)"
failures=0

cleanup() {
    if [ "$failures" -eq 0 ]; then
        rm -rf "$work"
    else
        echo "kept runner-test artifacts: $work" >&2
    fi
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    failures=$((failures + 1))
}

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

godot_calls() {
    wc -l < "$GRUS_STUB_CALLS" | tr -d ' '
}
run_stub() {
    local runs="$1" mode="$2"
    GRUS_RUNS="$runs" GRUS_STUB_MODE="$mode" bash "$runner" >"$work/last.out" 2>&1
    return $?
}

# (a) Invalid, zero and oversized counts fail before any Godot process starts.
# The oversized decimals pass the numeric regex but would wrap Bash arithmetic.
bad_values=(0 -1 abc 1x '' ' 2' 101 1000 9223372036854775808 18446744073709551616 999999999999999999999999999999)
for bad in "${bad_values[@]}"; do
    before="$(godot_calls)"
    if run_stub "$bad" clean; then
        fail "GRUS_RUNS='$bad' was accepted"
    fi
    [ "$(godot_calls)" -eq "$before" ] || fail "GRUS_RUNS='$bad' launched Godot"
    grep -q "positive integer no greater than 100" "$work/last.out" \
        || fail "GRUS_RUNS='$bad' did not explain the rejection"
done

# (b) A failing invocation is preserved, and later clean retries keep their own
# directories without overwriting it.
: > "$GRUS_STUB_CALLS"
if run_stub 1 leak; then fail "a leaking run was accepted"; fi
leak_dir="$(sed -n 's/^logs: //p' "$work/last.out")"
[ -n "$leak_dir" ] || fail "the failing invocation did not report a log directory"
[ -f "$leak_dir/run-1.log" ] || fail "the failing invocation did not keep its raw log"
grep -q "were leaked at exit" "$leak_dir/run-1.log" 2>/dev/null \
    || fail "the failing raw log lost its leak output"

if ! run_stub 1 clean; then fail "a clean retry after the failure failed"; fi
clean_dir="$(sed -n 's/^logs: //p' "$work/last.out")"
[ -n "$clean_dir" ] || fail "the clean retry did not report a log directory"
[ "$leak_dir" != "$clean_dir" ] || fail "the clean retry reused the failure's log directory"
[ -f "$leak_dir/run-1.log" ] || fail "the clean retry removed the earlier failure log"
grep -q "were leaked at exit" "$leak_dir/run-1.log" 2>/dev/null \
    || fail "the earlier failure log lost its leak output after a clean retry"

if ! run_stub 2 clean; then fail "a second clean invocation failed"; fi
clean_dir_2="$(sed -n 's/^logs: //p' "$work/last.out")"
[ "$clean_dir_2" != "$clean_dir" ] || fail "two clean invocations reused a log directory"
[ "$(godot_calls)" -eq 4 ] || fail "expected four stub processes, saw $(godot_calls)"

# (c) Exit code, marker and leak gates still decide the result.
if run_stub 1 fail; then fail "a nonzero Godot exit was accepted"; fi
grep -q "exited 3" "$work/last.out" || fail "the exit-code failure was not reported"
if run_stub 1 nomarker; then fail "a missing success marker was accepted"; fi
grep -q "never printed GRUS_GODOT_SMOKE_OK" "$work/last.out" \
    || fail "the marker failure was not reported"
if run_stub 1 leak; then fail "a leak diagnostic was accepted"; fi
grep -q "Leaked instance:" "$work/last.out" || fail "the leak detail was not printed"

# (d) Actual Godot shutdown regressions, when the engine is available: the
# focused default journey plus the stale-cache and Start/Paused close modes.
if [ -n "$real_godot" ]; then
    if ! GODOT_BIN="$real_godot" GRUS_RUNS=1 \
            GRUS_SHUTDOWN_SCENE=res://scenes/shutdown_quit_test.tscn \
            GRUS_SHUTDOWN_MARKER=GRUS_SHUTDOWN_QUIT_OK \
            GRUS_SHUTDOWN_LOG_DIR="$work/real" bash "$runner" >"$work/real-runner.out" 2>&1; then
        fail "the real focused-Godot shutdown regression failed"
        cat "$work/real-runner.out" >&2
    fi
    for mode in stale_start stale_resume close_start close_paused; do
        mode_log="$work/mode-$mode.log"
        if ! "$real_godot" --headless --path "$repo_root/godot" \
                res://scenes/shutdown_quit_test.tscn --verbose -- "$mode" >"$mode_log" 2>&1; then
            fail "focused mode $mode exited nonzero (log $mode_log)"
        fi
        grep -q "GRUS_SHUTDOWN_QUIT_OK" "$mode_log" \
            || fail "focused mode $mode never printed its marker (log $mode_log)"
        if grep -Eq "were leaked at exit|still in use at exit" "$mode_log"; then
            fail "focused mode $mode leaked at exit (log $mode_log)"
        fi
    done
else
    echo "skipping real Godot shutdown legs: no godot on PATH" >&2
fi

if [ "$failures" -ne 0 ]; then
    echo "shutdown runner contract: $failures failure(s)" >&2
    exit 1
fi
echo "shutdown runner contract: OK"
