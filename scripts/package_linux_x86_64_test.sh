#!/usr/bin/env bash
# Regression contract for scripts/package_linux_x86_64.sh: a failed run must
# never leave a previous archive at the release path. Runs the real script in
# a scratch repo with cargo/godot stubs; GRUS_STUB_FAIL selects which step
# fails (cargo|import|export). Exercises the release-path cleanup only — no
# real toolchain runs.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/repo/scripts" "$work/repo/godot" "$work/bin"
cp "$repo_root/scripts/package_linux_x86_64.sh" "$work/repo/scripts/"

cat > "$work/bin/cargo" <<'EOF'
#!/usr/bin/env bash
if [[ "${GRUS_STUB_FAIL:-}" == "cargo" ]]; then exit 1; fi
mkdir -p target/release
printf 'stub' > target/release/libgrus_godot.so
EOF

cat > "$work/bin/godot" <<'EOF'
#!/usr/bin/env bash
case " $* " in
    *" --import "*)
        [[ "${GRUS_STUB_FAIL:-}" == "import" ]] && exit 1
        ;;
    *" --export-release "*)
        [[ "${GRUS_STUB_FAIL:-}" == "export" ]] && exit 1
        out="${@: -1}"
        out="${out#../}"
        mkdir -p "$(dirname "$out")"
        printf '#!/bin/sh\n' > "$out"
        chmod +x "$out"
        ;;
esac
exit 0
EOF

chmod +x "$work/bin/cargo" "$work/bin/godot"

archive="$work/repo/dist/grus-linux-x86_64.tar.gz"
export_dir="$work/repo/dist/grus-linux-x86_64"

seed_stale_outputs() {
    rm -rf "$work/repo/dist" "$work/repo/target" "$work/repo/godot/bin"
    mkdir -p "$export_dir"
    printf 'stale-binary' > "$export_dir/Grus.x86_64"
    printf 'stale-archive' > "$archive"
    printf 'stale-tmp' > "$archive.tmp"
}

run_script() {
    PATH="$work/bin:$PATH" GRUS_STUB_FAIL="$1" \
        bash "$work/repo/scripts/package_linux_x86_64.sh" >"$work/out.log" 2>&1
}

fail() {
    echo "FAIL: $*" >&2
    echo "--- script output ---" >&2
    cat "$work/out.log" >&2
    exit 1
}

# Each fallible step must clear the stale release archive before failing. A
# failed export still creates a fresh (empty) export dir via mkdir -p, so the
# stale-binary check covers it; for earlier steps the whole dir must be gone.
for step in cargo import export; do
    seed_stale_outputs
    if run_script "$step"; then
        fail "expected script to fail with GRUS_STUB_FAIL=$step"
    fi
    [[ ! -e "$archive" ]] || fail "stale archive survived failed $step step"
    [[ ! -e "$archive.tmp" ]] || fail "stale temp archive survived failed $step step"
    [[ ! -e "$export_dir/Grus.x86_64" ]] \
        || fail "stale export binary survived failed $step step"
    if [[ "$step" != "export" ]]; then
        [[ ! -d "$export_dir" ]] \
            || fail "stale export dir survived failed $step step"
    fi
done

# Happy path: stubs succeed end to end and the archive is published via tmp+mv.
seed_stale_outputs
run_script none || fail "expected script to succeed with stubs"
[[ -f "$archive" ]] || fail "archive missing after successful stub run"
[[ ! -e "$archive.tmp" ]] || fail "temp archive left behind after publish"

echo "PACKAGE_SCRIPT_TEST_OK"
