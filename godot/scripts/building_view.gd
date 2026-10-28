extends Node3D

@onready var body: MeshInstance3D = $Body
@onready var health_bar: MeshInstance3D = $HealthBar
var _styled_team := -1
var _styled_kind := ""
var _health_ratio := 1.0

func _ready() -> void:
	add_to_group("building_views")

func _process(_delta: float) -> void:
	if not has_meta("team_id") or not has_meta("building_kind"):
		return
	var kind := str(get_meta("building_kind"))
	if kind != _styled_kind:
		_style_kind(kind)
	var team := int(get_meta("team_id"))
	if team == _styled_team:
		return
	_styled_team = team
	if team == 1:
		body.material_override.albedo_color = Color(0.2, 0.55, 0.95, 1.0)
	else:
		body.material_override.albedo_color = Color(0.9, 0.3, 0.25, 1.0)

func _style_kind(kind: String) -> void:
	_styled_kind = kind
	match kind:
		"Wall":
			var wall := BoxMesh.new()
			wall.size = Vector3(1.0, 0.9, 1.0)
			body.mesh = wall
			body.position.y = 0.45
		"Gate":
			body.visible = false
			_add_gate_part("GatePostLeft", Vector3(0.16, 1.2, 0.16), Vector3(-0.34, 0.6, 0.0))
			_add_gate_part("GatePostRight", Vector3(0.16, 1.2, 0.16), Vector3(0.34, 0.6, 0.0))
			_add_gate_part("GateLintel", Vector3(0.84, 0.16, 0.16), Vector3(0.0, 1.12, 0.0))
		"GunTurret":
			var turret := CylinderMesh.new()
			turret.top_radius = 0.36
			turret.bottom_radius = 0.55
			turret.height = 1.3
			body.mesh = turret
			body.position.y = 0.65

func _add_gate_part(part_name: String, size: Vector3, position: Vector3) -> void:
	var part := MeshInstance3D.new()
	var mesh := BoxMesh.new()
	mesh.size = size
	part.name = part_name
	part.mesh = mesh
	part.material_override = body.material_override
	part.position = position
	add_child(part)

## Called from the Rust side on Changed<Health>; the bar appears and shrinks
## once a building takes damage. Full health stays hidden.
func set_health_ratio(ratio: float) -> void:
	_health_ratio = clampf(ratio, 0.0, 1.0)
	health_bar.scale.x = maxf(_health_ratio, 0.02)
	health_bar.visible = _health_ratio < 0.999

func health_ratio() -> float:
	return _health_ratio
