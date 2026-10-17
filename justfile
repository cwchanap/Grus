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
    [ -f godot/.godot/extension_list.cfg ] || godot --headless --path godot --editor --quit-after 120
    godot --path godot "$@"
