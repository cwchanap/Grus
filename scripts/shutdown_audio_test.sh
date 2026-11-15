#!/usr/bin/env bash
# Real-process shutdown contract: run a scene through its real quit path with
# --verbose and fail if Godot reports leaked ObjectDB instances or resources
# still in use at exit. Godot reclaims a stopped AudioStreamPlayback on its
# mixer thread, so the game drains best-effort before quitting; this runner is
# the process-output gate that notices when that mitigation stops working.
#
# Every invocation writes into its own log directory under GRUS_SHUTDOWN_LOG_DIR
# (default test_output/shutdown-audio), so a retry never overwrites evidence.
#   GRUS_RUNS            positive integer, default 3
#   GRUS_SHUTDOWN_SCENE  default res://scenes/smoke_test.tscn
#   GRUS_SHUTDOWN_MARKER default GRUS_GODOT_SMOKE_OK
#   GODOT_BIN            default godot
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
godot_bin="${GODOT_BIN:-godot}"
scene="${GRUS_SHUTDOWN_SCENE:-res://scenes/smoke_test.tscn}"
marker="${GRUS_SHUTDOWN_MARKER:-GRUS_GODOT_SMOKE_OK}"
runs="${GRUS_RUNS-3}"
base_dir="${GRUS_SHUTDOWN_LOG_DIR:-$repo_root/test_output/shutdown-audio}"
# A run is a full scene process (seconds to minutes); 100 is far beyond any
# practical shutdown audit and keeps the loop counter inside Bash's signed
# 64-bit arithmetic, so an oversized decimal cannot wrap around.
runs_max=100

if ! [[ "$runs" =~ ^[1-9][0-9]*$ ]]; then
    echo "FAIL: GRUS_RUNS must be a positive integer no greater than $runs_max, got '$runs'" >&2
    exit 2
fi
if [ "${#runs}" -gt 3 ] || [ "$runs" -gt "$runs_max" ]; then
    echo "FAIL: GRUS_RUNS must be a positive integer no greater than $runs_max, got '$runs'" >&2
    exit 2
fi

log_dir="$base_dir/$(date -u +%Y%m%dT%H%M%SZ)-$$"
if ! mkdir -p "$log_dir"; then
    echo "FAIL: cannot create log directory $log_dir" >&2
    exit 2
fi
echo "logs: $log_dir"

status=0
for ((run = 1; run <= runs; run++)); do
    log="$log_dir/run-$run.log"
    "$godot_bin" --headless --path "$repo_root/godot" "$scene" --verbose >"$log" 2>&1
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "FAIL: $scene run $run exited $rc (log $log)" >&2
        status=1
    fi
    if ! grep -q "$marker" "$log"; then
        echo "FAIL: $scene run $run never printed $marker (log $log)" >&2
        status=1
    fi
    if grep -Eq "were leaked at exit|still in use at exit" "$log"; then
        echo "FAIL: $scene run $run leaked at exit (log $log):" >&2
        grep -E "were leaked at exit|still in use at exit|Leaked instance:|Resource still in use:" "$log" | sed 's/^/  /' >&2
        status=1
    fi
    echo "run $run: exit $rc, log $log"
done

exit "$status"
