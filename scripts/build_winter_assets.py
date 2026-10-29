"""Blender source for HPA-290. Run: blender --background --factory-startup --python-exit-code 1 --python scripts/build_winter_assets.py."""

import math
from pathlib import Path
import random

import bpy
from mathutils import Vector


OUT = Path(__file__).resolve().parents[1] / "godot/assets/winter"
RNG = random.Random(290)


def prop_material():
    """One shared material per prop: per-piece colors ride vertex colors so
    every exported model renders as a single surface/draw call."""
    mat = bpy.data.materials.new("Winter prop")
    mat.diffuse_color = (1.0, 1.0, 1.0, 1.0)
    mat.use_nodes = True
    shader = mat.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    shader.inputs["Roughness"].default_value = 0.92
    return mat


PROP = prop_material()


def tint(obj, hex_color):
    srgb = [int(hex_color[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in srgb]
    attribute = obj.data.color_attributes.new(
        name="Color", domain="CORNER", type="FLOAT_COLOR")
    for datum in attribute.data:
        datum.color = (*linear, 1.0)


def box(name, location, size, hex_color, rotation=0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=location)
    obj = bpy.context.object
    obj.name = name
    obj.dimensions = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    obj.rotation_euler.z = rotation
    obj.data.materials.append(PROP)
    tint(obj, hex_color)
    return obj


def rock(name, location, size, hex_color):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=location)
    obj = bpy.context.object
    obj.name = name
    obj.scale = size
    obj.rotation_euler.z = RNG.uniform(-math.pi, math.pi)
    obj.data.materials.append(PROP)
    tint(obj, hex_color)
    return obj


def export(name, resource=False):
    bpy.context.view_layer.update()
    meshes = [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
    points = [obj.matrix_world @ Vector(corner) for obj in meshes for corner in obj.bound_box]
    # The runtime scales obstacle X/Z to the authoritative blocked rectangle.
    assert points and all(abs(p.x) <= 0.5 and abs(p.y) <= 0.5 for p in points), name
    if not resource:
        # Scaling assumes the footprint fills the unit square edge-to-edge;
        # under-covering a blocked rect renders walkable cells as open ground.
        assert min(p.x for p in points) <= -0.499 and max(p.x for p in points) >= 0.499, name
        assert min(p.y for p in points) <= -0.499 and max(p.y for p in points) >= 0.499, name
    assert min(p.z for p in points) >= -0.00001, name
    assert max(p.z for p in points) <= (1 if resource else 2), name
    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.join()
    obj = bpy.context.object
    obj.name = name
    bpy.context.scene.cursor.location = (0, 0, 0)
    bpy.ops.object.origin_set(type="ORIGIN_CURSOR")
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    triangles = sum(len(face.vertices) - 2 for face in obj.data.polygons)
    assert triangles < 4000, (name, triangles)
    bpy.ops.export_scene.gltf(
        filepath=str(OUT / f"{name}.glb"), export_format="GLB",
        use_selection=True, export_yup=True, export_apply=True,
        export_cameras=False, export_lights=False,
        export_vertex_color="ACTIVE",
    )
    print(f"WINTER_ASSET_OK {name}: {triangles} triangles")
    bpy.ops.object.delete(use_global=False)


bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete(use_global=False)
OUT.mkdir(parents=True, exist_ok=True)
SNOW = "e6eef1"
ICE = "8daebd"
CONCRETE = "727e88"
BROKEN = "a4afb5"
STEEL = "63594f"
RUST = "aa613e"
DARK = "25313c"
ORANGE = "d18a39"
IVORY = "eee3c7"
COAL = "22252d"
COAL_EDGE = "465260"
VEHICLE = "527c7c"

# Broken city shell: exposed stories, snapped walls and a collapsed roof.
box("Foundation", (0, 0, 0.05), (1, 1, 0.1), CONCRETE)
box("Snow apron", (0, -0.1, 0.11), (0.98, 0.77, 0.04), SNOW)
for x, height in [(-0.36, 1.65), (-0.12, 1.35), (0.12, 1.8), (0.36, 1.1)]:
    box("Broken facade", (x, 0.36, height / 2 + 0.1), (0.23, 0.19, height), CONCRETE)
    box("Snow coping", (x, 0.36, height + 0.12), (0.24, 0.21, 0.045), SNOW)
    for z in (0.56, 1.13):
        if z + 0.18 < height:
            box("Empty window", (x, 0.257, z), (0.13, 0.015, 0.24), DARK)
            box("Window snow sill", (x, 0.24, z - 0.13), (0.15, 0.045, 0.035), SNOW)
box("Return wall", (-0.39, 0.08, 0.59), (0.17, 0.4, 0.98), CONCRETE)
box("Broken floor", (0.04, 0.08, 0.55), (0.67, 0.29, 0.14), BROKEN, -0.18)
box("Collapsed roof snow", (0.04, 0.08, 0.63), (0.63, 0.27, 0.025), SNOW, -0.18)
for x, y in [(-0.26, -0.28), (0.2, -0.28), (0.36, -0.1)]:
    rock("Masonry rubble", (x, y, 0.24), (0.12, 0.12, 0.12), BROKEN)
box("Exposed rebar", (0.12, 0.36, 1.94), (0.018, 0.018, 0.12), RUST)
export("collapsed_building")

# Wrecked transport: low body, broken cab, dark wheel and window silhouettes.
box("Ice plinth", (0, 0, 0.04), (1, 1, 0.08), ICE)
box("Snow drift", (0, 0.1, 0.09), (0.98, 0.72, 0.035), SNOW)
box("Chassis", (0, -0.02, 0.27), (0.88, 0.43, 0.18), DARK)
box("Body", (-0.07, -0.02, 0.46), (0.7, 0.45, 0.28), VEHICLE)
box("Cab", (-0.15, -0.02, 0.72), (0.48, 0.43, 0.28), VEHICLE)
for x in (-0.3, -0.14, 0.02):
    box("Shattered window", (x, -0.243, 0.73), (0.12, 0.018, 0.16), DARK)
box("Snow roof", (-0.15, -0.02, 0.88), (0.5, 0.45, 0.045), SNOW)
box("Rusted open hood", (0.31, -0.02, 0.62), (0.14, 0.42, 0.09), RUST, 0.12)
for x in (-0.27, 0.25):
    for y in (-0.26, 0.22):
        bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.135, depth=0.08,
                                          location=(x, y, 0.235), rotation=(math.pi / 2, 0, 0))
        bpy.context.object.data.materials.append(PROP)
        tint(bpy.context.object, DARK)
box("Detached bumper", (0.21, -0.38, 0.16), (0.4, 0.05, 0.08), STEEL, 0.13)
export("wrecked_vehicle")

# Frozen debris: a low readable rubble field with bent girders.
box("Frozen foundation", (0, 0, 0.05), (1, 1, 0.1), ICE)
for x, y, z in [(-0.25, -0.23, 0.3), (0.23, 0.22, 0.37), (-0.2, 0.2, 0.46), (0.22, -0.2, 0.24)]:
    rock("Broken slab", (x, y, z), (0.21, 0.19, z - 0.1), BROKEN)
    rock("Snow cap", (x, y, z + 0.13), (0.19, 0.17, 0.075), SNOW)
box("Collapsed girder", (0, 0, 0.67), (0.77, 0.1, 0.1), RUST, -0.6)
box("Girder flange", (0, 0, 0.73), (0.79, 0.18, 0.045), STEEL, -0.6)
box("Girder snow", (-0.06, 0.03, 0.76), (0.58, 0.14, 0.025), SNOW, -0.6)
export("frozen_debris")

# Resource art stays in one gather cell, with deliberately different silhouettes.
box("Pallet", (0, 0, 0.065), (0.9, 0.79, 0.13), STEEL)
for x, y, z in [(-0.21, -0.1, 0.28), (0.21, -0.1, 0.28), (0, 0.15, 0.61)]:
    box("Ration case", (x, y, z), (0.38, 0.42, 0.3), ORANGE)
    box("Ivory strap", (x, y - 0.217, z), (0.075, 0.018, 0.29), IVORY)
    box("Snow on lid", (x, y + 0.06, z + 0.16), (0.37, 0.28, 0.03), SNOW)
export("ration_cache", resource=True)

box("Salvage slab", (0, 0, 0.06), (0.9, 0.86, 0.12), CONCRETE)
box("Metal panel", (-0.12, 0.08, 0.3), (0.52, 0.4, 0.18), RUST, 0.3)
box("Snow on panel", (-0.12, 0.08, 0.4), (0.49, 0.37, 0.03), SNOW, 0.3)
for x in (-0.28, 0.28):
    box("Salvage upright", (x, 0.24, 0.49), (0.09, 0.1, 0.74), RUST)
box("Bent lintel", (0, 0.24, 0.85), (0.73, 0.12, 0.12), STEEL, -0.08)
box("Loose girder", (0.07, -0.22, 0.22), (0.66, 0.11, 0.13), RUST, -0.35)
export("scrap_ruin", resource=True)

box("Icy seam", (0, 0, 0.06), (0.94, 0.88, 0.12), ICE)
for x, y, z, size in [(-0.22, 0.04, 0.32, 0.23), (0.08, 0.12, 0.43, 0.25),
                       (0.25, -0.18, 0.27, 0.19), (-0.12, -0.24, 0.22, 0.16)]:
    rock("Exposed coal", (x, y, z), (size, size * 0.8, size), COAL if x < 0.2 else COAL_EDGE)
rock("Frozen snow crest", (0.1, 0.16, 0.68), (0.22, 0.16, 0.07), SNOW)
export("coal_seam", resource=True)
