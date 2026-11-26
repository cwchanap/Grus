# Grus task runner. Run `just` (no args) to list recipes.

set positional-arguments

lib := if os() == "macos" { "libgrus_godot.dylib" } else { "libgrus_godot.so" }

# List available recipes (default)
default:
    @just --list

# Build the GDExtension, stage it in godot/bin, launch. Extra args go to Godot, e.g. `just run --headless --quit-after 5`
run *args:
    cargo build -p grus-godot
    mkdir -p godot/bin
    cp target/debug/{{ lib }} godot/bin/{{ lib }}
    if [ "{{ os() }}" = macos ]; then codesign --force --sign - godot/bin/{{ lib }}; fi
    # Always import: a registered extension does not mean assets were imported (missing fonts/textures otherwise).
    godot --headless --path godot --import
    godot --path godot "$@"
