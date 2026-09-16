extends Node

## HPA-472 combat/lifecycle smoke. Drives the bridge directly — no
## synthesized mouse/keyboard input — and grows with each integration slice:
## Start -> attack bridge -> HP decrease -> death/view removal -> result
## overlay -> input rejection -> restart -> fresh state, plus role
## readability assertions that do not depend on final art.

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## RejectReason discriminant order (crates/grus-sim/src/commands.rs); the
## bridge reports "no rejection" as -1.
const REJECT_SESSION_LOCKED := 18

## Proven-open Barracks anchor for the Team 1 base
## (crates/grus-sim/src/fixture.rs layout).
const BARRACKS_ANCHOR := Vector2i(17, 41)
const ENEMY_TOWN_CENTER_CELL := Vector2(114.0, 48.0)

var _main: Node3D

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(1)

func _wait_until(condition: Callable, timeout_s: float, message: String) -> bool:
	var deadline_ms := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while Time.get_ticks_msec() < deadline_ms:
		if condition.call():
			return true
		await get_tree().physics_frame
	_fail("%s (waited %.0fs)" % [message, timeout_s])
	return false

func _find_view(group: String, meta: String, id: int) -> Node3D:
	for node in get_tree().get_nodes_in_group(group):
		if int(node.get_meta(meta, -1)) == id:
			return node as Node3D
	return null

func _unit_view(id: int) -> Node3D:
	return _find_view("unit_views", "unit_id", id)

func _building_view(id: int) -> Node3D:
	return _find_view("building_views", "building_id", id)

func _fixture_views_settled() -> bool:
	var units := get_tree().get_nodes_in_group("unit_views")
	var buildings := get_tree().get_nodes_in_group("building_views")
	var resources := get_tree().get_nodes_in_group("resource_views")
	if units.size() != 8 or buildings.size() != 2 or resources.size() != 18:
		return false
	for unit in units:
		if int(unit.get_meta("unit_id", -1)) <= 0:
			return false
	return true

func _run() -> void:
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame
	_main = get_node("Main") as Node3D

	if not GrusBridge.set_sim_speed(20.0):
		_fail("failed to accelerate the sim")
		return

	if not await _wait_until(_fixture_views_settled, 20.0,
			"skirmish startup never settled to 8 units / 2 buildings / 18 resources"):
		return

	# Boot: normal skirmish sits in Start and gameplay orders are rejected
	# while the session is locked.
	var session: Dictionary = GrusBridge.session_snapshot()
	if str(session.get("phase", "")) != "Start":
		_fail("skirmish boot session is not Start: %s" % [session])
		return
	if not GrusBridge.move_units(PackedInt32Array([1]), Vector2(30, 44)):
		_fail("bridge refused to queue the Start-phase probe command")
		return
	if not await _wait_until(
			func(): return int(GrusBridge.last_reject_code()) == REJECT_SESSION_LOCKED, 5.0,
			"Start-phase gameplay order was not rejected with SessionLocked (%d)" % REJECT_SESSION_LOCKED):
		return

	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return

	# Attack bridge: the target kind is a closed string; anything else is
	# rejected at the bridge before a command is queued.
	if GrusBridge.attack_units(PackedInt32Array([1]), "cactus", 5):
		_fail("attack_units accepted an invalid target kind")
		return
	if GrusBridge.attack_units(PackedInt32Array([1]), "unit", 0):
		_fail("attack_units accepted a zero target id")
		return
	if GrusBridge.attack_units(PackedInt32Array([]), "unit", 5):
		_fail("attack_units accepted an empty id list")
		return
	if not GrusBridge.attack_units(PackedInt32Array([1]), "unit", 5):
		_fail("attack_units refused to queue a well-formed attack")
		return
	if not GrusBridge.attack_move_units(PackedInt32Array([1]), Vector2(20, 44)):
		_fail("attack_move_units refused to queue")
		return

	# Health bars and role markers ride the views: fresh units report a full
	# health ratio, and every kind has a distinct primitive presentation
	# (body scale + marker) that does not depend on final art.
	for id in range(1, 9):
		var unit := _unit_view(id)
		if unit == null:
			_fail("unit view %d missing for role/health assertions" % id)
			return
		if absf(float(unit.call("health_ratio")) - 1.0) > 0.0001:
			_fail("fresh unit %d health ratio is not 1.0" % id)
			return
	var unit_view_script := preload("res://scripts/unit_view.gd")
	var role_specs: Dictionary = unit_view_script.ROLE_SPECS
	var presentations := {}
	for kind in ["Villager", "Spearman", "Archer", "Cavalry"]:
		if not role_specs.has(kind):
			_fail("role spec missing for %s" % kind)
			return
		var spec: Dictionary = role_specs[kind]
		var presentation := "%.1f/%s" % [float(spec.get("body_scale", 0.0)), str(spec.get("marker", "?"))]
		if presentations.has(presentation):
			_fail("role presentation duplicated between kinds: %s" % presentation)
			return
		presentations[presentation] = kind
	var villager := _unit_view(1)
	var villager_body := villager.get_node("Body") as MeshInstance3D
	if absf(villager_body.scale.x - 0.8) > 0.0001 \
			or (villager.get_node("RoleMarker") as Node3D).get_child_count() != 0:
		_fail("villager role presentation is not the small unmarked body")
		return

	print("GRUS_COMBAT_LIFECYCLE_SMOKE_OK stage=roles-health start_reject=session_locked invalid_target_kinds=3 role_kinds=4")
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(0)
