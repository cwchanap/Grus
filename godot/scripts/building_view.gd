extends Node3D

@onready var body: MeshInstance3D = $Body
var _styled_team := -1

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
