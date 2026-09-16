extends Node3D

@onready var body: MeshInstance3D = $Body
@onready var health_bar: MeshInstance3D = $HealthBar
var _styled_team := -1
var _health_ratio := 1.0

func _ready() -> void:
	add_to_group("building_views")

func _process(_delta: float) -> void:
	if not has_meta("team_id"):
		return
	var team := int(get_meta("team_id"))
	if team == _styled_team:
		return
	_styled_team = team
	if team == 1:
		body.material_override.albedo_color = Color(0.2, 0.55, 0.95, 1.0)
	else:
		body.material_override.albedo_color = Color(0.9, 0.3, 0.25, 1.0)

## Called from the Rust side on Changed<Health>; the bar appears and shrinks
## once a building takes damage. Full health stays hidden.
func set_health_ratio(ratio: float) -> void:
	_health_ratio = clampf(ratio, 0.0, 1.0)
	health_bar.scale.x = maxf(_health_ratio, 0.02)
	health_bar.visible = _health_ratio < 0.999

func health_ratio() -> float:
	return _health_ratio
