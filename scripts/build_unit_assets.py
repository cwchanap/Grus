"""Build the static HPA-82 unit GLBs with Blender's built-in glTF exporter."""
import math
from pathlib import Path

import bpy
from mathutils import Vector

OUT = Path(__file__).resolve().parents[1] / 'godot/assets/units'
OUT.mkdir(parents=True, exist_ok=True)
COLORS = [
    '263c4b', '466c85', '48545b', '492f37', '56839b', '658c9c', '6e3036',
    '7998a3', '8d3f3c', '92744e', '947654', 'a48052', 'a94d42', 'ad8657',
    'ae6850', 'b7c9c8', 'c07759', 'd1a183', 'd2ad8e', 'edb65b',
]
# A palette texture gives every joined mesh one shared material and surface.
image = bpy.data.images.new('Unit palette', width=8, height=8, alpha=True)
pixels = []
for i in range(64):
    color = COLORS[min(i, len(COLORS) - 1)]
    rgb = [int(color[j:j + 2], 16) / 255 for j in (0, 2, 4)]
    pixels.extend(rgb + [1])
image.pixels = pixels
image.file_format = 'PNG'
image.pack()
mat = bpy.data.materials.new('Unit palette')
mat.use_nodes = True
shader = mat.node_tree.nodes['Principled BSDF']
shader.inputs['Roughness'].default_value = 0.9
texture = mat.node_tree.nodes.new('ShaderNodeTexImage')
texture.image = image
texture.interpolation = 'Closest'
mat.node_tree.links.new(texture.outputs['Color'], shader.inputs['Base Color'])


def tint(obj, color):
    index = COLORS.index(color)
    uv = obj.data.uv_layers.active or obj.data.uv_layers.new(name='UVMap')
    for loop in uv.data:
        loop.uv = ((index % 8 + 0.5) / 8, (index // 8 + 0.5) / 8)
    obj.data.materials.append(mat)
    return obj


def box(pos, size, color, angle=0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=pos)
    obj = bpy.context.object
    obj.dimensions = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    obj.rotation_euler[1] = angle
    return tint(obj, color)


def orb(pos, size, color):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=pos)
    obj = bpy.context.object
    obj.scale = size
    return tint(obj, color)


def limb(start, end, radius, color, tip=None):
    a, b = Vector(start), Vector(end)
    delta = b - a
    bpy.ops.mesh.primitive_cone_add(vertices=6, radius1=radius, radius2=radius if tip is None else tip,
                                    depth=delta.length, location=(a + b) / 2)
    obj = bpy.context.object
    obj.rotation_euler = delta.to_track_quat('Z', 'Y').to_euler()
    return tint(obj, color)


def human(kind):
    coat = {'worker': '56839b', 'guard': '466c85', 'rifleman': '658c9c'}[kind]
    dark, trim, skin, metal = '263c4b', 'b7c9c8', 'd2ad8e', '48545b'
    for x in (-0.17, 0.17):
        box((x, 0, 0.22), (0.24, 0.30, 0.42), dark)
        box((x, -0.07, 0.07), (0.25, 0.43, 0.14), metal)
    box((0, 0, 0.69), (0.64, 0.40, 0.62), coat)
    box((0, 0.02, 0.48), (0.66, 0.42, 0.08), dark)
    orb((0, 0, 1.13), (0.25, 0.23, 0.27), trim)
    box((0, -0.205, 1.13), (0.25, 0.06, 0.13), skin)
    box((0, -0.235, 1.15), (0.20, 0.025, 0.055), dark)
    for x in (-0.38, 0.38):
        limb((x * 0.83, 0, 0.92), (x, -0.05, 0.53), 0.12, coat)
        orb((x, -0.06, 0.48), (0.11, 0.10, 0.10), dark)
    if kind == 'worker':
        box((0, 0.27, 0.72), (0.48, 0.20, 0.48), '92744e')
        box((0.38, -0.12, 0.76), (0.06, 0.07, 0.75), 'ad8657', -0.2)
        box((0.48, -0.12, 1.10), (0.34, 0.08, 0.10), metal, -0.2)
    elif kind == 'guard':
        box((-0.39, -0.18, 0.68), (0.32, 0.10, 0.48), '7998a3')
        box((-0.39, -0.24, 0.68), (0.08, 0.03, 0.35), trim)
        limb((0.42, -0.12, 0.14), (0.42, -0.12, 1.65), 0.045, 'a48052')
        limb((0.42, -0.12, 1.56), (0.42, -0.12, 1.88), 0.12, trim, 0)
    else:
        box((0.43, -0.30, 0.82), (0.85, 0.14, 0.14), metal)
        box((1.00, -0.30, 0.82), (0.30, 0.07, 0.07), dark)
        box((0.08, -0.30, 0.71), (0.20, 0.14, 0.17), '947654')
        box((0, 0.25, 0.72), (0.43, 0.15, 0.38), dark)


def alien(kind):
    # All dimensions are authored in Blender Z-up and exported Y-up.
    scale = {'swarmer': 0.68, 'spitter': 0.95, 'brute': 1.35, 'behemoth': 2.25}[kind]
    shell = {'swarmer': 'a94d42', 'spitter': 'ae6850', 'brute': '8d3f3c', 'behemoth': '6e3036'}[kind]
    dark, bone, glow = '492f37', 'd1a183', 'edb65b'
    def p(v): return tuple(n * scale for n in v)
    def o(pos, size, color): orb(p(pos), p(size), color)
    def l(a, b, r, color, tip=None): limb(p(a), p(b), r * scale, color, None if tip is None else tip * scale)
    o((0, 0, 0.64), (0.55, 0.68, 0.43), shell)
    o((0, -0.48, 0.65), (0.39, 0.35, 0.30), dark)
    for x in (-1, 1):
        o((x * 0.22, -0.73, 0.73), (0.09, 0.06, 0.07), glow)
        for y in (-0.37, 0.10, 0.43):
            l((x * 0.36, y, 0.58), (x * 0.69, y - 0.06, 0.38), 0.13, shell, 0.09)
            l((x * 0.69, y - 0.06, 0.38), (x * 0.78, y - 0.16, 0.04), 0.09, dark, 0.035)
        l((x * 0.22, -0.64, 0.48), (x * 0.32, -0.91, 0.32), 0.10, bone, 0)
    for y in (-0.22, 0.12, 0.43):
        l((0, y, 0.86), (0, y + 0.08, 1.27), 0.14, bone, 0)
    if kind == 'spitter':
        o((0, 0.40, 0.91), (0.48, 0.46, 0.39), glow)
        l((0, -0.68, 0.55), (0, -1.08, 0.57), 0.16, shell, 0.10)
    elif kind in ('brute', 'behemoth'):
        for x in (-1, 1):
            o((x * 0.48, -0.22, 0.72), (0.36, 0.39, 0.38), shell)
            l((x * 0.5, -0.35, 0.68), (x * 0.78, -0.69, 0.16), 0.23, dark, 0.13)
            l((x * 0.37, -0.40, 0.90), (x * 0.72, -0.48, 1.32), 0.17, bone, 0)
        if kind == 'behemoth':
            for x in (-1, 1):
                l((x * 0.27, 0.10, 0.95), (x * 0.55, 0.28, 1.65), 0.22, bone, 0)
            o((0, 0.27, 0.95), (0.38, 0.42, 0.24), 'c07759')


def export(name):
    bpy.context.view_layer.update()
    meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH']
    points = [o.matrix_world @ Vector(c) for o in meshes for c in o.bound_box]
    assert min(p.z for p in points) >= -0.001, name
    assert max(p.z for p in points) < 4.5, name
    bpy.ops.object.select_all(action='SELECT')
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.join()
    obj = bpy.context.object
    obj.name = name
    bpy.context.scene.cursor.location = (0, 0, 0)
    bpy.ops.object.origin_set(type='ORIGIN_CURSOR')
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    triangles = sum(len(face.vertices) - 2 for face in obj.data.polygons)
    assert triangles < 2000, (name, triangles)
    bpy.ops.export_scene.gltf(filepath=str(OUT / f'{name}.glb'), export_format='GLB',
        use_selection=True, export_yup=True, export_apply=True, export_cameras=False,
        export_lights=False)
    print(f'UNIT_ASSET_OK {name}: {triangles} triangles, height {max(p.z for p in points):.2f}')
    bpy.ops.object.delete(use_global=False)


bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
for kind in ('worker', 'guard', 'rifleman'):
    human(kind)
    export(kind)
for kind in ('swarmer', 'spitter', 'brute', 'behemoth'):
    alien(kind)
    export(kind)
