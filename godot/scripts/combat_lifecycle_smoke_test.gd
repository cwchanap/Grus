extends Node

## HPA-472 combat/lifecycle smoke. Drives the bridge for seam contracts and
## the battlefield controller's input handlers directly (constructed
## arguments, no synthesized OS input) for the input path: Start -> attack
## bridge -> HP decrease -> death/view removal -> result overlay -> input
## rejection -> restart -> fresh state, plus role readability assertions
## that do not depend on final art.

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## RejectReason declaration order (crates/grus-sim/src/commands.rs):
## SessionLocked is the 19th variant (0-based 18). The bridge exposes
## numeric codes only (last_reject_code) and reports "no rejection" as -1.
const REJECT_SESSION_LOCKED := 18

## Proven-open Barracks anchor for the Team 1 base
## (crates/grus-sim/src/fixture.rs layout).
const BARRACKS_ANCHOR := Vector2i(17, 41)
## Walkable ground beside the enemy Town Center footprint (112..115, 46..49);
## attack-move here must acquire the Town Center within the 8-unit radius.
const ENEMY_TOWN_CENTER_CELL := Vector2(118.5, 44.5)

var _main: Node3D
var _fx: Node3D

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(1)

func _wait_until(condition: Callable, timeout_s: float, message: String) -> bool:
	var frame_budget := int(ceilf(timeout_s * Engine.physics_ticks_per_second))
	for _frame in frame_budget:
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

func _first_kind_view(group: String, meta: String, value: String, box: Dictionary) -> bool:
	for node in get_tree().get_nodes_in_group(group):
		if str(node.get_meta(meta, "")) == value:
			box["view"] = node as Node3D
			return true
	return false

func _session_panel() -> Panel:
	return _main.get_node("HUD/SessionPanel") as Panel

func _session_label() -> Label:
	return _main.get_node("HUD/SessionPanel/SessionLabel") as Label

func _button(path: String) -> Button:
	return _main.get_node(path) as Button

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


## Screen position of a world point through the controller's own camera —
## the exact projection its picking and ground targeting invert.
func _screen_of(world_point: Vector3) -> Vector2:
	return _main.camera.unproject_position(world_point)


## Presses A through the controller's own key handler (a constructed
## InputEventKey argument — no OS event is injected into the tree).
func _press_a() -> void:
	var key := InputEventKey.new()
	key.keycode = KEY_A
	key.pressed = true
	_main.call("_handle_key", key)


## Outside Playing neither input handler may reach the bridge: the context
## command is refused with status text, A arms nothing, and the command
## feedback revision (bumped by every queued command) stays put.
func _assert_input_gated(where: String) -> bool:
	var revision := int(GrusBridge.command_feedback_revision())
	_main.call("_issue_context_command", Vector2(TEST_VIEWPORT_SIZE) / 2.0)
	_press_a()
	if _main.command_status.text != "Match is not playing":
		_fail("%s: context command was not refused: %s" % [where, _main.command_status.text])
		return false
	if bool(_main._attack_move_armed) \
			or int(GrusBridge.command_feedback_revision()) != revision:
		_fail("%s: an input handler reached the bridge while not Playing" % where)
		return false
	return true

## The restart is only settled when every seeded villager 1..8 is present
## with no stale views (e.g. the dead Spearman's node still alive) mixed in.
func _restart_settled() -> bool:
	if not _fixture_views_settled():
		return false
	var ids := {}
	for node in get_tree().get_nodes_in_group("unit_views"):
		ids[int(node.get_meta("unit_id", -1))] = true
	for id in range(1, 9):
		if not ids.has(id):
			return false
	return ids.size() == 8

func _run() -> void:
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame
	_main = get_node("Main") as Node3D
	_fx = _main.get_node("CombatFx") as Node3D

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
	if not (_session_panel().visible and _button("HUD/SessionPanel/StartButton").visible \
				and not _button("HUD/SessionPanel/QuitButton").visible):
		_fail("Start overlay is not up during the Start phase (Quit must stay Result-only)")
		return
	if not GrusBridge.move_units(PackedInt32Array([1]), Vector2(30, 44)):
		_fail("bridge refused to queue the Start-phase probe command")
		return
	if not await _wait_until(
			func(): return int(GrusBridge.last_reject_code()) == REJECT_SESSION_LOCKED, 5.0,
			"Start-phase gameplay order was not rejected with SessionLocked (%d)" % REJECT_SESSION_LOCKED):
		return

	# Controller-level input gating during Start: right-click and A refuse
	# without queuing anything.
	if not _assert_input_gated("Start"):
		return

	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return
	if not await _wait_until(
			func(): return not _session_panel().visible and _button("HUD/PauseButton").visible, 5.0,
			"Playing phase did not hide the overlay and show the Pause button"):
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

	# Combat journey: produce a combatant, then attack enemy villager 5.
	if not GrusBridge.place_building(4, "Barracks", BARRACKS_ANCHOR.x, BARRACKS_ANCHOR.y):
		_fail("Barracks placement was rejected")
		return
	var barracks_box := {"view": null}
	if not await _wait_until(func(): return _first_kind_view("building_views", "building_kind", "Barracks", barracks_box), 25.0,
			"Barracks view never appeared"):
		return
	var barracks: Node3D = barracks_box["view"]
	var barracks_id := int(barracks.get_meta("building_id"))
	if absf(float(barracks.call("health_ratio")) - 1.0) > 0.0001:
		_fail("fresh Barracks health ratio is not 1.0")
		return
	if not await _wait_until(
			func(): return bool(GrusBridge.building_snapshot(barracks_id).get("complete", false)), 40.0,
			"Barracks never completed"):
		return
	if not GrusBridge.enqueue_unit(barracks_id, "Spearman"):
		_fail("Spearman training was rejected")
		return
	var spear_box := {"view": null}
	if not await _wait_until(func(): return _first_kind_view("unit_views", "unit_kind", "Spearman", spear_box), 30.0,
			"Spearman never trained"):
		return
	var spearman: Node3D = spear_box["view"]
	var spear_id := int(spearman.get_meta("unit_id"))
	if spear_id < 9:
		_fail("trained Spearman reused a seeded id: %d" % spear_id)
		return
	var spear_body := spearman.get_node("Body") as MeshInstance3D
	if absf(spear_body.scale.x - 1.0) > 0.0001 \
			or (spearman.get_node("RoleMarker") as Node3D).get_child_count() != 1:
		_fail("Spearman role presentation is not the marked full-size body")
		return

	# Controller-level attack-move: arm through the A-key handler, then
	# right-click open ground in the north corridor — the arm must clear and
	# the Spearman must march (movement-observed AttackMove). This runs while
	# the Spearman is still beside the home Barracks: issued next to the
	# enemy Town Center it would correctly acquire the building inside
	# ATTACK_MOVE_RADIUS and fight in place instead of marching.
	var ground_click := _screen_of(Vector3(62.5, 0.0, 12.5))
	var march_selection: Array[int] = [spear_id]
	_main.selected_ids = march_selection
	_press_a()
	if not bool(_main._attack_move_armed):
		_fail("the A-key handler did not arm attack-move")
		return
	_main.call("_issue_context_command", ground_click)
	if bool(_main._attack_move_armed) \
			or _main.command_status.text != "Attack-move queued":
		_fail("armed ground click did not queue an attack-move: %s"
				% _main.command_status.text)
		return
	var spear_before := spearman.global_position
	if not await _wait_until(
			func(): return spearman.global_position.distance_to(spear_before) > 2.0, 15.0,
			"A-armed attack-move never moved the Spearman"):
		return

	var victim := _unit_view(5)
	if victim == null:
		_fail("enemy villager 5 view is missing before the attack")
		return
	# Controller-level right-click attack: select the Spearman and drive
	# _issue_context_command with the screen position of enemy villager 5 —
	# the enemy-picking path must queue the same bridge attack.
	var enemy_click := _screen_of(victim.global_position)
	var attack_selection: Array[int] = [spear_id]
	_main.selected_ids = attack_selection
	_main.call("_issue_context_command", enemy_click)
	if _main.command_status.text != "Attack command queued":
		_fail("controller right-click on the enemy did not queue an attack: %s"
				% _main.command_status.text)
		return
	# Null-safe lookup: the view node is freed the moment the unit dies.
	if not await _wait_until(
			func(): return _unit_view(5) == null \
					or float(_unit_view(5).call("health_ratio")) < 1.0, 20.0,
			"victim HP never decreased through the health bar"):
		return
	if not await _wait_until(func(): return _fx.call("effect_count") > 0, 20.0,
			"no transient combat effects spawned during combat"):
		return
	if not await _wait_until(func(): return _unit_view(5) == null, 20.0,
			"dead unit 5 view was never removed"):
		return

	# Selection truth is the live view tree, not the cosmetic kill event —
	# at 20x a later fixed tick can wipe the event before the drain. Seed
	# dead unit 5 into the selection and a control group; reconcile must
	# prune both without any event arriving.
	var dead_five: Array[int] = [5]
	_main.selected_ids = dead_five
	_main._control_groups[3] = dead_five.duplicate()
	if not await _wait_until(
			func(): return _main.selected_ids.is_empty() and _main._control_groups[3].is_empty(), 5.0,
			"dead unit 5 was never reconciled out of selection/control groups"):
		return

	# Attack-move onto the enemy Town Center: acquisition must pick it up
	# and drop its health through the building health bar.
	if not GrusBridge.attack_move_units(PackedInt32Array([spear_id]), ENEMY_TOWN_CENTER_CELL):
		_fail("attack_move_units refused the Town Center push")
		return
	var enemy_tc := _building_view(2)
	if enemy_tc == null:
		_fail("enemy Town Center view 2 is missing")
		return
	if not await _wait_until(func(): return float(enemy_tc.call("health_ratio")) < 1.0, 30.0,
			"attack-move never damaged the enemy Town Center"):
		return

	# Pause freezes the world and the cosmetics: the last Playing tick's
	# combat events stay in the bridge and must not replay during Paused.
	if not GrusBridge.set_paused(true):
		_fail("set_paused(true) was refused")
		return
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Paused", 5.0,
			"session never reached Paused"):
		return
	if not await _wait_until(
			func(): return _session_panel().visible and _button("HUD/SessionPanel/ResumeButton").visible \
					and not _button("HUD/PauseButton").visible \
					and not _button("HUD/SessionPanel/QuitButton").visible \
					and _session_label().text == "Paused", 5.0,
			"Paused overlay is not showing Resume with Pause/Quit hidden"):
		return
	var fx_before := int(_fx.call("effect_count"))
	var hp_before := float(enemy_tc.call("health_ratio"))
	for _frame in 40:
		await get_tree().physics_frame
	if int(_fx.call("effect_count")) > fx_before \
			or float(enemy_tc.call("health_ratio")) != hp_before:
		_fail("paused session kept simulating or replayed stale combat effects")
		return

	# Controller-level input gating while Paused.
	if not _assert_input_gated("Paused"):
		return

	if not GrusBridge.set_paused(false):
		_fail("set_paused(false) was refused")
		return
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Playing", 5.0,
			"session never resumed to Playing"):
		return

	# First Town Center destruction settles the Result: overlay shows
	# Victory with Restart/Quit for the local team.
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Result", 60.0,
			"enemy Town Center destruction never reached Result"):
		return
	var result: Dictionary = GrusBridge.session_snapshot()
	if int(result.get("winner_team", -1)) != 1:
		_fail("Result winner is not team 1: %s" % [result])
		return
	if not await _wait_until(
			func(): return _session_panel().visible and _session_label().text == "Victory!" \
					and _button("HUD/SessionPanel/RestartButton").visible \
					and _button("HUD/SessionPanel/QuitButton").visible, 5.0,
			"Result overlay is not showing Victory with Restart/Quit"):
		return

	# Result locks gameplay orders just like Start did.
	if not GrusBridge.move_units(PackedInt32Array([1]), Vector2(30, 44)):
		_fail("bridge refused to queue the Result-phase probe command")
		return
	if not await _wait_until(
			func(): return int(GrusBridge.last_reject_code()) == REJECT_SESSION_LOCKED, 5.0,
			"Result-phase gameplay order was not rejected with SessionLocked"):
		return

	# Controller-level input gating during Result.
	if not _assert_input_gated("Result"):
		return

	# The destroyed Town Center reconciles out of selected_building_id the
	# same way — the settle tick's preserved kill event is cosmetic only.
	_main.selected_building_id = 2
	if not await _wait_until(
			func(): return int(_main.selected_building_id) == -1, 5.0,
			"destroyed Town Center 2 was never reconciled out of selected_building_id"):
		return

	# Restart hygiene: dead stable ids (the slain villager 5 and Spearman)
	# sit in selection/control groups, a live effect is on the stage, then
	# the restart must clear all of it while the bridge reseeds fresh ids.
	var dead_ids: Array[int] = [5, spear_id]
	_main.selected_ids = dead_ids
	_main.selected_building_id = 2
	_main._control_groups[1] = dead_ids.duplicate()
	_main._attack_move_armed = true
	# Seed at least one live effect for the restart-hygiene check. A
	# killing-blow effect drained on the settle frame may legitimately still
	# be alive here, so only "nothing landed" is a failure.
	_fx.call("spawn_death", Vector3.ZERO)
	if int(_fx.call("effect_count")) < 1:
		_fail("transient effect seed for the restart check did not land")
		return
	_main.call("_restart_match")
	if not await _wait_until(_restart_settled, 20.0,
			"restart never re-settled to the exact fresh 8-unit fixture"):
		return
	var fresh_units := get_tree().get_nodes_in_group("unit_views")
	for unit in fresh_units:
		var id := int(unit.get_meta("unit_id", -1))
		if id < 1 or id > 8:
			_fail("restart left a stale unit view id %d" % id)
			return
		if absf(float((unit as Node3D).call("health_ratio")) - 1.0) > 0.0001:
			_fail("restarted unit %d is not at full health" % id)
			return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Start":
		_fail("restart did not return the session to Start")
		return
	if not (_session_panel().visible and _button("HUD/SessionPanel/StartButton").visible \
				and not _button("HUD/SessionPanel/QuitButton").visible):
		_fail("restart did not bring the Start overlay back with Quit hidden")
		return
	if not _main.selected_ids.is_empty() or not _main._control_groups.is_empty() \
			or int(_main.selected_building_id) != -1 or bool(_main._attack_move_armed):
		_fail("restart left dead selection/control-group state behind")
		return
	if int(_fx.call("effect_count")) != 0:
		_fail("restart left transient combat effects on the stage")
		return

	print("GRUS_COMBAT_LIFECYCLE_SMOKE_OK stage=restart-fresh start_reject=session_locked invalid_target_kinds=3 role_kinds=4 hp_decreased=true death_removed=true effects=spawned pause_frozen=true overlay=victory result_reject=session_locked controller_attack=queued controller_attack_move=marched input_gated=start_paused_result selection_reconciled=true restart_cleared=true")
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(0)
