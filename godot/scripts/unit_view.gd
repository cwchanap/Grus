extends Node3D

## Primitive role presentation per unit kind — no art assets. Every kind
## gets a distinct (body scale, marker primitive) pair so units are
## readable before final art lands.
const ROLE_SPECS := {
	"Worker": {"body_scale": 0.8, "marker": ""},
	"Guard": {"body_scale": 1.0, "marker": "spear"},
	"Rifleman": {"body_scale": 0.9, "marker": "bow"},
	"Swarmer": {"body_scale": 0.62, "marker": "spines"},
	"Spitter": {"body_scale": 0.84, "marker": "sac"},
	"Brute": {"body_scale": 1.3, "marker": "shoulders"},
	"Behemoth": {"body_scale": 1.8, "marker": "crest"},
}

## Class-level shared marker resources: 150+ alien views must not each own
## unique meshes/materials — every instance pulls from these caches.
static var _shared_meshes: Dictionary = {}
static var _shared_marker_material: StandardMaterial3D = null

@onready var body: MeshInstance3D = $Body
@onready var selection_ring: MeshInstance3D = $SelectionRing
@onready var role_marker: Node3D = $RoleMarker
@onready var health_bar: MeshInstance3D = $HealthBar
var _styled_team := -1
var _styled_kind := ""
var _health_ratio := 1.0

func _ready() -> void:
	add_to_group("unit_views")

func _process(_delta: float) -> void:
	if not has_meta("team_id"):
		return
	var team := int(get_meta("team_id"))
	if team != _styled_team:
		_styled_team = team
		if team == 1:
			body.material_override.albedo_color = Color(0.2, 0.55, 0.95, 1.0)
		else:
			body.material_override.albedo_color = Color(0.9, 0.3, 0.25, 1.0)
	if not has_meta("unit_kind"):
		return
	var kind := str(get_meta("unit_kind"))
	if kind != _styled_kind:
		_styled_kind = kind
		_apply_role(kind)

func unit_id() -> int:
	return int(get_meta("unit_id", -1))

func team_id() -> int:
	return int(get_meta("team_id", -1))

func unit_kind() -> String:
	return str(get_meta("unit_kind", ""))

func set_selected(selected: bool) -> void:
	selection_ring.visible = selected

## Called from the Rust side on Changed<Health>; the bar appears and shrinks
## once a unit takes damage. Full health stays hidden.
func set_health_ratio(ratio: float) -> void:
	_health_ratio = clampf(ratio, 0.0, 1.0)
	health_bar.scale.x = maxf(_health_ratio, 0.02)
	health_bar.visible = _health_ratio < 0.999

func health_ratio() -> float:
	return _health_ratio

func _apply_role(kind: String) -> void:
	var spec: Dictionary = ROLE_SPECS.get(kind, {"body_scale": 1.0, "marker": ""})
	var body_scale := float(spec.get("body_scale", 1.0))
	body.scale = Vector3.ONE * body_scale
	# Keep the scaled body grounded (the cylinder mesh centers on the node).
	body.position.y = 0.5 * body_scale
	health_bar.position.y = body_scale + 0.4
	for old in role_marker.get_children():
		old.queue_free()
	match str(spec.get("marker", "")):
		"spear":
			_add_marker(_shared_mesh("spear"))
		"bow":
			_add_marker(_shared_mesh("bow"))
		"spines":
			for offset in [-0.22, 0.0, 0.22]:
				_add_marker(_shared_mesh("spine"), Vector3(offset, body_scale + 0.08, 0.0))
		"sac":
			_add_marker(_shared_mesh("sac"), Vector3(0.18, body_scale * 0.8, 0.0))
		"shoulders":
			_add_marker(_shared_mesh("shoulders"), Vector3(0.0, body_scale * 0.7, 0.0))
		"crest":
			_add_marker(_shared_mesh("crest"), Vector3(0.0, body_scale + 0.18, 0.0))

## Builds each marker primitive exactly once and hands every unit view the
## same instance — kind silhouettes stay distinct via the shared mesh class.
func _shared_mesh(key: String) -> Mesh:
	var mesh: Mesh = _shared_meshes.get(key)
	if mesh != null:
		return mesh
	match key:
		"spear":
			mesh = _make_box_mesh(Vector3(0.08, 1.6, 0.08))
		"bow":
			mesh = _make_box_mesh(Vector3(0.55, 0.55, 0.08))
		"spine":
			var spike := CylinderMesh.new()
			spike.top_radius = 0.0
			spike.bottom_radius = 0.12
			spike.height = 0.48
			mesh = spike
		"sac":
			var sphere := SphereMesh.new()
			sphere.radius = 0.24
			sphere.height = 0.48
			mesh = sphere
		"shoulders":
			mesh = _make_box_mesh(Vector3(1.1, 0.32, 0.78))
		"crest":
			var capsule := CapsuleMesh.new()
			capsule.radius = 0.34
			capsule.height = 0.95
			mesh = capsule
		_:
			return null
	_shared_meshes[key] = mesh
	return mesh

func _make_box_mesh(size: Vector3) -> BoxMesh:
	var mesh := BoxMesh.new()
	mesh.size = size
	return mesh

func _add_marker(mesh: Mesh, local_position := Vector3(0.0, 0.8, 0.0)) -> void:
	if mesh == null:
		return
	var view := MeshInstance3D.new()
	view.name = "MarkerMesh"
	view.mesh = mesh
	view.position = local_position
	if _shared_marker_material == null:
		_shared_marker_material = StandardMaterial3D.new()
		_shared_marker_material.albedo_color = Color(0.25, 0.22, 0.2, 1.0)
	view.material_override = _shared_marker_material
	role_marker.add_child(view)
