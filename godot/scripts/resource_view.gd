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
	var previous := get_node_or_null("Model")
	if previous != null:
		remove_child(previous)
		previous.queue_free()
	if MODELS.has(kind):
		var model: Node3D = MODELS[kind].instantiate()
		model.name = "Model"
		add_child(model)
