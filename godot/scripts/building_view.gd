extends Node3D

## The bridge pushes building state on first sight and transitions: `complete` flips the
## generic construction site for the kind's final GLB exactly once, `cold`
## toggles one shared pale overlay on the active model's meshes. Neither
## flag is polled — Rust is authoritative — and pushes arriving before the
## stamped building_kind metadata just cache until _process first sees it.
const ICE_OVERLAY := preload("res://assets/structures/ice_overlay.tres")
const SITE_MODEL := preload("res://assets/structures/construction_site.glb")
const MODELS := {
	"ResearchCenter": preload("res://assets/structures/research_center.glb"),
	"Bunkhouse": preload("res://assets/structures/bunkhouse.glb"),
	"Depot": preload("res://assets/structures/depot.glb"),
	"Barracks": preload("res://assets/structures/barracks.glb"),
	"Wall": preload("res://assets/structures/wall.glb"),
	"Gate": preload("res://assets/structures/gate.glb"),
	"GunTurret": preload("res://assets/structures/gun_turret.glb"),
	"CoalFurnace": preload("res://assets/structures/coal_furnace.glb"),
	"FlameTower": preload("res://assets/structures/flame_tower.glb"),
	"CrystalLance": preload("res://assets/structures/crystal_lance.glb"),
}

@onready var health_bar: MeshInstance3D = $HealthBar
var _styled_kind := ""
var _complete := false
var _cold := false
var _health_ratio := 1.0

func _ready() -> void:
	add_to_group("building_views")

func _process(_delta: float) -> void:
	if not has_meta("building_kind"):
		return
	_styled_kind = str(get_meta("building_kind"))
	# The kind meta is stamped once at spawn and never changes, and every
	# later state change arrives through set_state — style once, then stop
	# polling every frame.
	if _attach_model():
		set_process(false)

## Rust value-gates authoritative state pushes. These flags also preserve
## early state and make repeated completion or cold pushes idempotent.
func set_state(complete: bool, cold: bool) -> void:
	var finishing := complete and not _complete
	_complete = complete
	if cold != _cold:
		_cold = cold
		_apply_cold()
	if _styled_kind == "":
		# Pushed before building_kind metadata was stamped (or before
		# _process first saw it): the cached flags drive the first
		# _attach_model, so a seeded-complete building renders its final
		# model without ever flashing the site.
		return
	if finishing:
		set_process(not _attach_model())

func _attach_model() -> bool:
	var instance: Node
	if not _complete:
		instance = SITE_MODEL.instantiate()
	else:
		if MODELS.has(_styled_kind):
			instance = (MODELS[_styled_kind] as PackedScene).instantiate()
		else:
			# A kind whose GLB has not landed yet must still render and be
			# debuggable — an invisible building would silently stand.
			push_warning("building_view: no model for building_kind '%s', using placeholder" % _styled_kind)
			instance = _placeholder()
	var model := instance as Node3D
	if model == null:
		if instance != null:
			instance.free()
		push_error("building_view: failed to instantiate model for '%s'" % _styled_kind)
		return false
	var previous := get_node_or_null("Model")
	if previous != null:
		remove_child(previous)
		previous.queue_free()
	model.name = "Model"
	add_child(model)
	_apply_cold()
	_seat_health_bar()
	return true

func _placeholder() -> Node3D:
	var holder := Node3D.new()
	var mesh_instance := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(0.8, 1.0, 0.8)
	mesh_instance.mesh = box
	mesh_instance.position.y = 0.5
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.62, 0.67, 0.72, 1.0)
	material.roughness = 0.85
	mesh_instance.material_override = material
	holder.add_child(mesh_instance)
	return holder

## The one shared translucent ice material sits on the active model's
## meshes while cold — multiplying vertex-colored albedo would only
## darken, so a pale overlay whitens instead. HealthBar never takes it.
func _apply_cold() -> void:
	var model := get_node_or_null("Model") as Node3D
	if model == null:
		return
	var overlay: Material = ICE_OVERLAY if _cold else null
	for node in model.find_children("*", "MeshInstance3D", true, false):
		(node as MeshInstance3D).material_overlay = overlay

## The bar seats just above the active model's real AABB each time the
## model swaps, so a flat site and a tall final GLB both read correctly.
func _seat_health_bar() -> void:
	var model := get_node_or_null("Model") as Node3D
	if model == null:
		return
	var bounds := AABB()
	var first := true
	for node in model.find_children("*", "MeshInstance3D", true, false):
		var mesh := node as MeshInstance3D
		var local: AABB = global_transform.affine_inverse() * mesh.global_transform * mesh.get_aabb()
		bounds = local if first else bounds.merge(local)
		first = false
	if first:
		return
	health_bar.position = Vector3(bounds.get_center().x, bounds.end.y + 0.15, bounds.get_center().z)

## Called from the Rust side on Changed<Health>; the bar appears and shrinks
## once a building takes damage. Full health stays hidden.
func set_health_ratio(ratio: float) -> void:
	_health_ratio = clampf(ratio, 0.0, 1.0)
	health_bar.scale.x = maxf(_health_ratio, 0.02)
	health_bar.visible = _health_ratio < 0.999

func health_ratio() -> float:
	return _health_ratio
