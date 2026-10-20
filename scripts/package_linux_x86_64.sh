#!/usr/bin/env bash
# Package a reproducible Linux x86_64 release build into dist/.
# Requires a Linux x86_64 host with Godot and the Linux export templates
# already installed; the script never downloads or installs toolchains.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Optional: package with a different main scene (e.g. the benchmark). The
# value must be a plain res:// path — it is substituted into project.godot
# by sed, so anything more exotic is rejected rather than escaped. The
# original file is restored afterwards — on success, error or interrupt —
# with its own mode (cp -p; a plain cp into the 0600 mktemp file would
# restore project.godot owner-only).
main_scene_backup=""
suffix=""
if [[ -n "${GRUS_MAIN_SCENE:-}" ]]; then
    [[ "$GRUS_MAIN_SCENE" =~ ^res://[A-Za-z0-9_./-]+$ ]] \
        || { echo "error: GRUS_MAIN_SCENE must be a res:// scene path, got '$GRUS_MAIN_SCENE'" >&2; exit 1; }
    scene_stem="$(basename "$GRUS_MAIN_SCENE")"
    suffix="-${scene_stem%.*}"
    main_scene_backup="$(mktemp)"
    cp -p godot/project.godot "$main_scene_backup"
    trap 'mv -f "$main_scene_backup" godot/project.godot' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    grep -q '^run/main_scene=' godot/project.godot \
        || { echo "error: no run/main_scene entry in godot/project.godot" >&2; exit 1; }
    sed "s|^run/main_scene=.*|run/main_scene=\"$GRUS_MAIN_SCENE\"|" \
        godot/project.godot > "$main_scene_backup.tmp"
    mv "$main_scene_backup.tmp" godot/project.godot
fi

# Clear previous package outputs before the first fallible build/import
# step so a failed run can never leave a stale archive behind. An
# alternate main scene gets its own output name — a package that boots
# the benchmark must never sit at the release path.
dist="dist/grus-linux-x86_64$suffix"
archive="dist/grus-linux-x86_64$suffix.tar.gz"
rm -rf "$dist"
rm -f "$archive" "$archive.tmp"

cargo build --release -p grus-godot --locked
mkdir -p godot/bin
cp target/release/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --import

mkdir -p "$dist"
godot --headless --path godot --export-release "Linux x86_64" "../$dist/Grus.x86_64"
test -x "$dist/Grus.x86_64"
tar -czf "$archive.tmp" -C dist "${dist##*/}"
mv "$archive.tmp" "$archive"
echo "packaged: $archive"
