extends Node3D

## Painted unit presentation (HPA-1198): one camera-facing Sprite3D per unit
## with a base and an action pose, brought alive by code motion only — a
## left/right flip by heading, a walk bob and waddle, the action pose on each
## attack (and each gather/build swing for Workers), a short red hurt flash,
## and a topple-and-fade corpse handed to CombatFx on death. Presentation
## only: Rust stays authoritative and pushes health and Worker work state.
const BuildingView := preload("res://scripts/building_view.gd")

const POSES := {
	"Worker": [preload("res://assets/units/painted/worker.png"), preload("res://assets/units/painted/worker_action.png")],
	"Guard": [preload("res://assets/units/painted/guard.png"), preload("res://assets/units/painted/guard_action.png")],
	"Rifleman": [preload("res://assets/units/painted/rifleman.png"), preload("res://assets/units/painted/rifleman_action.png")],
	"Swarmer": [preload("res://assets/units/painted/swarmer.png"), preload("res://assets/units/painted/swarmer_action.png")],
	"Spitter": [preload("res://assets/units/painted/spitter.png"), preload("res://assets/units/painted/spitter_action.png")],
	"Brute": [preload("res://assets/units/painted/brute.png"), preload("res://assets/units/painted/brute_action.png")],
	"Behemoth": [preload("res://assets/units/painted/behemoth.png"), preload("res://assets/units/painted/behemoth_action.png")],
}
## On-screen size per kind, sized by the painted figure, not its canvas (the
## figures fill 55–95% of their 512 px canvas): [figure height in world units
## on the camera-facing plane, selection ring / health bar scale]. At the
## default zoom one world unit is 20 px at 1920×1080 and 13.3 px at 1280×720.
const SIZES := {
	"Worker": [1.9, 1.0], "Guard": [1.9, 1.0], "Rifleman": [1.9, 1.0],
	"Swarmer": [1.5, 1.1], "Spitter": [1.8, 1.2], "Brute": [2.5, 1.6], "Behemoth": [3.8, 2.6],
}
## Art contract (art-src/2026-11-26/process.py): both poses share one canvas
## scale, figures face right, and the feet and baked contact shadow sit at
## the bottom center, 24 px above the bottom edge of a 512 px canvas.
const FEET_FROM_BOTTOM := 24.0 / 512.0
## Ranged attackers' muzzle on the action pose, in normalized top-down
## texture coordinates; melee kinds strike from the figure's middle.
const MUZZLE_UV := {"Rifleman": Vector2(0.97, 0.34), "Spitter": Vector2(0.97, 0.53)}
const DEFAULT_SPRITE_PITCH_DEGREES := 54.0
const GROUND_LIFT := 0.01
## The sprite leans back like the painted buildings, so its upper body lies
## behind the feet in depth and a low 3D prop just behind a unit (a Wall,
## Gate beam or resource) would cut across its legs. Sliding the sprite this
## far toward the orthographic camera changes only its depth, never its
## screen position: units then stand in front of props they are in front
## of, while painted buildings (whose planes start at their front edge, at
## least 2.5 cells ahead of any unit behind them) still hide units behind.
const DEPTH_BIAS := 1.0
const ACTION_SECONDS := 0.22
const SWING_SECONDS := 0.75
const HURT_SECONDS := 0.18
const HURT_TINT := Color(1.0, 0.35, 0.3, 1.0)
## Facing flips only after this much horizontal travel the other way, so
## separation nudges never make a unit flicker.
const FLIP_TRAVEL := 0.05
const WALK_CYCLES_PER_SECOND := 2.6
const BOB_FRACTION := 0.035
const WADDLE_DEGREES := 4.0

## Monotonic count of action poses shown by any unit view (smoke telemetry,
## like CombatFx.cue_counts).
static var actions_played := 0

@onready var selection_ring: MeshInstance3D = $SelectionRing
@onready var health_bar: MeshInstance3D = $HealthBar
var _health_ratio := 1.0
var _kind := ""
var _model: Node3D = null
var _sprite: Sprite3D = null
var _figure_height := 1.0
var _bar_scale := 1.0
var _facing_left := false
var _travel_x := 0.0
var _last_position := Vector3.INF
var _walk_time := 0.0
var _action_left := 0.0
var _hurt_left := 0.0
var _working := false
var _swing_clock := 0.0

func _ready() -> void:
	add_to_group("unit_views")

func _process(delta: float) -> void:
	if _sprite == null:
		_try_attach()
		return
	if not visible:
		# Fog-hidden enemies keep their state but skip the per-frame motion.
		_last_position = global_position
		return
	_update_heading_and_walk(delta)
	if _working:
		_swing_clock += delta
		if _swing_clock >= SWING_SECONDS:
			_swing_clock -= SWING_SECONDS
			play_action()
	if _action_left > 0.0:
		_action_left -= delta
		if _action_left <= 0.0:
			_sprite.texture = POSES[_kind][0]
	if _hurt_left > 0.0:
		_hurt_left = maxf(_hurt_left - delta, 0.0)
		_sprite.modulate = Color.WHITE.lerp(HURT_TINT, _hurt_left / HURT_SECONDS)

## The bridge stamps `unit_kind` a frame after instancing; attach once it
## lands. Views without a known kind (synthetic test probes) stay bare.
func _try_attach() -> void:
	if not has_meta("unit_kind"):
		return
	_kind = str(get_meta("unit_kind"))
	if not POSES.has(_kind):
		set_process(false)
		push_warning("unit_view: no painted poses for unit_kind '%s'" % _kind)
		return
	var base: Texture2D = POSES[_kind][0]
	_figure_height = float(SIZES[_kind][0])
	_bar_scale = float(SIZES[_kind][1])
	_model = Node3D.new()
	_model.name = "Model"
	_sprite = Sprite3D.new()
	_sprite.texture = base
	_sprite.billboard = BaseMaterial3D.BILLBOARD_DISABLED
	_sprite.shaded = false
	_sprite.alpha_cut = SpriteBase3D.ALPHA_CUT_OPAQUE_PREPASS
	_sprite.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_sprite.centered = false
	_sprite.pixel_size = _figure_height / _figure_pixels(base)
	# Bottom edge on the ground, canvas center on the unit: the feet and the
	# baked shadow then sit on the unit's position.
	_sprite.offset = Vector2(-0.5 * base.get_width(), 0.0)
	_sprite.rotation_degrees = Vector3(-_sprite_pitch_degrees(), 0.0, 0.0)
	_sprite.position = Vector3(0.0, GROUND_LIFT, 0.0) + _sprite.transform.basis.z * DEPTH_BIAS
	_model.add_child(_sprite)
	add_child(_model)
	selection_ring.scale = Vector3.ONE * _bar_scale
	_seat_health_bar()
	set_health_ratio(_health_ratio)
	_last_position = global_position

## Figure height in texture pixels: feet row to the topmost opaque row of
## the base pose (shared opaque-pixel cache with the building views).
static func _figure_pixels(texture: Texture2D) -> float:
	var feet_row := texture.get_height() * (1.0 - FEET_FROM_BOTTOM)
	var top := BuildingView.opaque_top_pixels(texture)
	if top == Vector2.INF or top.y >= feet_row:
		return float(texture.get_height())
	return feet_row - top.y

func _sprite_pitch_degrees() -> float:
	var viewport := get_viewport()
	var cam := viewport.get_camera_3d() if viewport != null else null
	if cam != null:
		return clampf(rad_to_deg(atan2(cam.global_basis.z.y, cam.global_basis.z.z)), 0.0, 90.0)
	return DEFAULT_SPRITE_PITCH_DEGREES

## The bar rides just above the base pose's opaque top, centered over the
## unit; its width scales with the kind like the selection ring.
func _seat_health_bar() -> void:
	var top := BuildingView.opaque_top_pixels(_sprite.texture)
	if top == Vector2.INF:
		return
	var local: Vector3 = _sprite.transform * BuildingView.sprite_local_from_pixel(_sprite, top)
	health_bar.position = Vector3(0.0, local.y + 0.15, local.z)

func _update_heading_and_walk(delta: float) -> void:
	var now := global_position
	var step := now - _last_position if _last_position != Vector3.INF else Vector3.ZERO
	_last_position = now
	if step.x != 0.0:
		if signf(step.x) != signf(_travel_x):
			_travel_x = 0.0
		_travel_x += step.x
		if absf(_travel_x) >= FLIP_TRAVEL:
			_set_facing_left(_travel_x < 0.0)
			_travel_x = 0.0
	if Vector2(step.x, step.z).length_squared() > 1e-8:
		_walk_time += delta
		var phase := _walk_time * WALK_CYCLES_PER_SECOND * TAU
		_model.position.y = absf(sin(phase)) * BOB_FRACTION * _figure_height
		_sprite.rotation_degrees.z = sin(phase) * WADDLE_DEGREES
	elif _walk_time > 0.0:
		# Settle once on stopping; idle units then write no transforms.
		_walk_time = 0.0
		_model.position.y = 0.0
		_sprite.rotation_degrees.z = 0.0

func _set_facing_left(left: bool) -> void:
	_facing_left = left
	if _sprite != null:
		_sprite.flip_h = left

func _face_toward(world_x: float) -> void:
	if is_finite(world_x) and absf(world_x - global_position.x) > 0.01:
		_set_facing_left(world_x < global_position.x)

## Shows the action pose for one beat, facing `target_x` when given. Called
## by the battlefield controller for each attack event and by the Worker
## swing cycle.
func play_action(target_x: float = NAN) -> void:
	if _sprite == null:
		return
	_face_toward(target_x)
	_sprite.texture = POSES[_kind][1]
	_action_left = ACTION_SECONDS
	actions_played += 1

## Pushed by Rust when a Worker starts or stops gathering/constructing;
## `target_x` is the world x of the source or site it works on.
func set_working(working: bool, target_x: float) -> void:
	_working = working
	_swing_clock = SWING_SECONDS if working else 0.0
	if working:
		_face_toward(target_x)

func is_working() -> bool:
	return _working

func facing_left() -> bool:
	return _facing_left

func showing_action() -> bool:
	return _sprite != null and _sprite.texture == POSES[_kind][1]

## The painted sprite, for probes; null before attach.
func sprite() -> Sprite3D:
	return _sprite

## A kill frees this view before the frame's combat events drain, so a
## visible painted view hands its last presentation to CombatFx as it is
## freed; the kill event's death effect then topples it as a corpse. The
## script sees PREDELETE before Node detaches it, so it is still in the tree.
func _notification(what: int) -> void:
	if what != NOTIFICATION_PREDELETE or _sprite == null \
			or not is_inside_tree() or not is_visible_in_tree():
		return
	var fx := get_tree().get_first_node_in_group("combat_fx")
	if fx != null and fx.has_method("remember_freed_unit"):
		fx.call("remember_freed_unit", unit_id(), _sprite, POSES[_kind][0])

## World-space muzzle for ranged tracers, on the action pose the attack just
## switched to; melee kinds and unattached views use the figure's middle.
func muzzle_position() -> Vector3:
	if _sprite == null:
		return global_position + Vector3(0.0, 0.8, 0.0)
	var uv: Vector2 = MUZZLE_UV.get(_kind, Vector2(0.5, 0.6))
	return _sprite.global_transform * BuildingView.sprite_local_from_pixel(_sprite, uv * _sprite.texture.get_size())

func unit_id() -> int:
	return int(get_meta("unit_id", -1))

func team_id() -> int:
	return int(get_meta("team_id", -1))

func unit_kind() -> String:
	return str(get_meta("unit_kind", ""))

func set_selected(selected: bool) -> void:
	selection_ring.visible = selected

## Called from the Rust side on Changed<Health>; the bar appears and shrinks
## once a unit takes damage, and each drop flashes the sprite red. Full
## health stays hidden.
func set_health_ratio(ratio: float) -> void:
	var clamped := clampf(ratio, 0.0, 1.0)
	if clamped < _health_ratio - 0.0001 and _sprite != null:
		_hurt_left = HURT_SECONDS
		_sprite.modulate = HURT_TINT
	_health_ratio = clamped
	health_bar.scale = Vector3(maxf(_health_ratio, 0.02) * _bar_scale, 1.0, 1.0)
	health_bar.visible = _health_ratio < 0.999

func health_ratio() -> float:
	return _health_ratio
