#!/usr/bin/env bash
# Real-process shutdown contract: run a smoke scene through its real quit path
# with --verbose and fail if Godot reports leaked ObjectDB instances or
# resources still in use at exit. Godot reclaims a stopped AudioStreamPlayback
# on its mixer thread, so a quit in the same frame as the stop used to leak the
# Ogg playback and its stream; the game therefore drains before quitting.
#
# The scene and its success marker can be overridden to audit another journey:
#   GRUS_SHUTDOWN_SCENE=res://scenes/economy_smoke_test.tscn \
#   GRUS_SHUTDOWN_MARKER=GRUS_ECONOMY_SMOKE_OK scripts/shutdown_audio_test.sh
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
godot_bin="${GODOT_BIN:-godot}"
scene="${GRUS_SHUTDOWN_SCENE:-res://scenes/smoke_test.tscn}"
marker="${GRUS_SHUTDOWN_MARKER:-GRUS_GODOT_SMOKE_OK}"
runs="${GRUS_RUNS:-3}"
log_dir="${GRUS_SHUTDOWN_LOG_DIR:-$repo_root/test_output/shutdown-audio}"
mkdir -p "$log_dir"

status=0
for run in $(seq 1 "$runs"); do
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
