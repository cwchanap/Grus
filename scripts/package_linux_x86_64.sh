#!/usr/bin/env bash
# Package a reproducible Linux x86_64 release build into dist/.
# Requires a Linux x86_64 host with Godot and the Linux export templates
# already installed; the script never downloads or installs toolchains.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Optional: package with a different main scene (e.g. the benchmark), always
# restoring godot/project.godot afterwards — on success, error or interrupt.
main_scene_backup=""
if [[ -n "${GRUS_MAIN_SCENE:-}" ]]; then
    main_scene_backup="$(mktemp)"
    cp godot/project.godot "$main_scene_backup"
    trap 'mv -f "$main_scene_backup" godot/project.godot' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    grep -q '^run/main_scene=' godot/project.godot \
        || { echo "error: no run/main_scene entry in godot/project.godot" >&2; exit 1; }
    sed "s|^run/main_scene=.*|run/main_scene=\"$GRUS_MAIN_SCENE\"|" \
        godot/project.godot > "$main_scene_backup.tmp"
    mv "$main_scene_backup.tmp" godot/project.godot
fi

# Clear previous package outputs before the first fallible build/import step
# so a failed run can never leave a stale archive at the release path.
dist="dist/grus-linux-x86_64"
archive="dist/grus-linux-x86_64.tar.gz"
rm -rf "$dist"
rm -f "$archive" "$archive.tmp"

cargo build --release -p grus-godot --locked
mkdir -p godot/bin
cp target/release/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --import

mkdir -p "$dist"
godot --headless --path godot --export-release "Linux x86_64" "../$dist/Grus.x86_64"
test -x "$dist/Grus.x86_64"
tar -czf "$archive.tmp" -C dist grus-linux-x86_64
mv "$archive.tmp" "$archive"
echo "packaged: $archive"
