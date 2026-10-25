extends Node3D

@onready var body: MeshInstance3D = $Body
var _styled_kind := ""

func _ready() -> void:
	add_to_group("resource_views")

func _process(_delta: float) -> void:
	if not has_meta("resource_kind"):
		return
	var kind := str(get_meta("resource_kind"))
	if kind == _styled_kind:
		return
	_styled_kind = kind
	match kind:
		"Food":
			body.material_override.albedo_color = Color(0.72, 0.32, 0.38, 1.0)
		"Scrap":
			body.material_override.albedo_color = Color(0.7, 0.4, 0.18, 1.0)
		"Coal":
			body.material_override.albedo_color = Color(0.18, 0.19, 0.24, 1.0)
