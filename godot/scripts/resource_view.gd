extends Node3D

const MODELS := {
	"Food": preload("res://assets/winter/ration_cache.glb"),
	"Scrap": preload("res://assets/winter/scrap_ruin.glb"),
	"Coal": preload("res://assets/winter/coal_seam.glb"),
}

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
	# The bridge stamps the kind once at spawn and it never changes: style
	# once, then stop polling every frame.
	set_process(false)
	var previous := get_node_or_null("Model")
	if previous != null:
		remove_child(previous)
		previous.queue_free()
	if MODELS.has(kind):
		var model: Node3D = MODELS[kind].instantiate()
		model.name = "Model"
		add_child(model)
		return
	# An unmapped kind must still render and be debuggable — an invisible
	# resource would stay silently gatherable on an empty cell.
	push_warning("resource_view: no model for resource_kind '%s', using placeholder" % kind)
	var fallback := MeshInstance3D.new()
	var mesh := CylinderMesh.new()
	mesh.top_radius = 0.35
	mesh.bottom_radius = 0.4
	mesh.height = 0.55
	fallback.mesh = mesh
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.5, 0.5, 0.5, 1.0)
	material.roughness = 0.9
	fallback.material_override = material
	fallback.name = "Model"
	fallback.position.y = 0.28
	add_child(fallback)
