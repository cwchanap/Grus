extends Node3D

const TEAM_WARM_BLUE := Color(0.2, 0.55, 0.95, 1.0)
const TEAM_RED := Color(0.9, 0.3, 0.25, 1.0)
## Icy cold tint: visibly desaturated/whitened, never the saturated warm blue.
const TEAM_COLD_ICE := Color(0.72, 0.82, 0.95, 1.0)

@onready var body: MeshInstance3D = $Body
@onready var health_bar: MeshInstance3D = $HealthBar
var _styled_team := -1
var _styled_kind := ""
var _health_ratio := 1.0
var _cold := false

func _ready() -> void:
	add_to_group("building_views")

func _process(_delta: float) -> void:
	if not has_meta("team_id") or not has_meta("building_kind"):
		return
	var kind := str(get_meta("building_kind"))
	if kind != _styled_kind:
		_style_kind(kind)
	var team := int(get_meta("team_id"))
	if team != _styled_team:
		_styled_team = team
		_apply_team_color()

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

## Called from the Rust side every Update with the live cold value. The
## cached flag skips redundant material writes; an early call before team
## metadata exists just caches, and the team styling below reapplies the
## cold material once metadata lands (late-attached views included).
func set_cold(cold: bool) -> void:
	if cold == _cold:
		return
	_cold = cold
	_apply_team_color()

## Team color plus the cached cold state in one write, so re-styling a
## view's team can never leave a stale warm/cold material behind.
func _apply_team_color() -> void:
	if not has_meta("team_id"):
		return
	if int(get_meta("team_id")) == 1:
		body.material_override.albedo_color = TEAM_COLD_ICE if _cold else TEAM_WARM_BLUE
	else:
		body.material_override.albedo_color = TEAM_RED

## Called from the Rust side on Changed<Health>; the bar appears and shrinks
## once a building takes damage. Full health stays hidden.
func set_health_ratio(ratio: float) -> void:
	_health_ratio = clampf(ratio, 0.0, 1.0)
	health_bar.scale.x = maxf(_health_ratio, 0.02)
	health_bar.visible = _health_ratio < 0.999

func health_ratio() -> float:
	return _health_ratio
