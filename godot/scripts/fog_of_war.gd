extends Node3D

## Fog-of-war presentation: two primitive MultiMesh overlays over the
## terrain — Unexplored (opaque dark) and Explored (translucent dark);
## Visible cells get no instance. Polls the bridge's cheap visibility
## revision every frame and rebuilds the instance buffers only when the sim
## revision changes. No custom shader, no image assets, no per-cell nodes.

const FOG_Y := 0.15

var _revision := -1
var _unexplored: MultiMeshInstance3D
var _explored: MultiMeshInstance3D

func _ready() -> void:
	_unexplored = _make_layer(Color(0.02, 0.03, 0.05, 1.0))
	_explored = _make_layer(Color(0.02, 0.03, 0.05, 0.45))

func _process(_delta: float) -> void:
	var revision := int(GrusBridge.visibility_revision())
	if revision == _revision:
		return
	_revision = revision
	if revision < 0:
		# No visibility state (visibility-free benchmark): clear the fog.
		_unexplored.multimesh.instance_count = 0
		_explored.multimesh.instance_count = 0
		_unexplored.visible = false
		_explored.visible = false
		return
	var snap: Dictionary = GrusBridge.visibility_snapshot()
	if int(snap.get("revision", -1)) != revision:
		# The revision moved during the read; the next frame catches up.
		return
	_rebuild(snap)

func _make_layer(color: Color) -> MultiMeshInstance3D:
	var instance := MultiMeshInstance3D.new()
	var quad := QuadMesh.new()
	quad.size = Vector2(1.0, 1.0)
	quad.orientation = PlaneMesh.FACE_Y
	var material := StandardMaterial3D.new()
	material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = color
	quad.material = material
	var multimesh := MultiMesh.new()
	multimesh.transform_format = MultiMesh.TRANSFORM_3D
	multimesh.mesh = quad
	multimesh.instance_count = 0
	instance.multimesh = multimesh
	instance.visible = false
	add_child(instance)
	return instance

func _rebuild(snap: Dictionary) -> void:
	var width := int(snap.get("width", 0))
	var height := int(snap.get("height", 0))
	var states: PackedInt32Array = snap.get("states", PackedInt32Array())
	if width <= 0 or height <= 0 or states.size() != width * height:
		return
	_fill(_unexplored, states, width, 0)
	_fill(_explored, states, width, 1)

func _fill(layer: MultiMeshInstance3D, states: PackedInt32Array, width: int, state: int) -> void:
	var count := 0
	for value in states:
		if value == state:
			count += 1
	var multimesh := layer.multimesh
	# Resizing (re)allocates the instance buffer; fill it right after.
	multimesh.instance_count = count
	var index := 0
	var x := 0
	var y := 0
	for value in states:
		if value == state:
			multimesh.set_instance_transform(
				index, Transform3D(Basis.IDENTITY, Vector3(x + 0.5, FOG_Y, y + 0.5)))
			index += 1
		x += 1
		if x >= width:
			x = 0
			y += 1
	layer.visible = count > 0
