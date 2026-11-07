extends Node3D

const MODELS := {
	"Worker": preload("res://assets/units/worker.glb"),
	"Guard": preload("res://assets/units/guard.glb"),
	"Rifleman": preload("res://assets/units/rifleman.glb"),
	"Swarmer": preload("res://assets/units/swarmer.glb"),
	"Spitter": preload("res://assets/units/spitter.glb"),
	"Brute": preload("res://assets/units/brute.glb"),
	"Behemoth": preload("res://assets/units/behemoth.glb"),
}
const HEIGHTS := {
	"Worker": 1.4, "Guard": 1.88, "Rifleman": 1.4,
	"Swarmer": 0.88, "Spitter": 1.24, "Brute": 1.93, "Behemoth": 3.93,
}
const RING_SCALES := {"Swarmer": 0.9, "Spitter": 1.25, "Brute": 1.8, "Behemoth": 3.0}

@onready var body: MeshInstance3D = $Body
@onready var selection_ring: MeshInstance3D = $SelectionRing
@onready var health_bar: MeshInstance3D = $HealthBar
var _health_ratio := 1.0

func _ready() -> void:
	add_to_group("unit_views")

func _process(_delta: float) -> void:
	if not has_meta("unit_kind"):
		return
	var kind := str(get_meta("unit_kind"))
	set_process(false)
	if not MODELS.has(kind):
		push_warning("unit_view: no model for unit_kind '%s'" % kind)
		return
	var model: Node3D = MODELS[kind].instantiate()
	model.name = "Model"
	add_child(model)
	body.visible = false
	health_bar.position.y = float(HEIGHTS[kind]) + 0.25
	selection_ring.scale = Vector3.ONE * float(RING_SCALES.get(kind, 1.0))

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
