extends Node

## HPA-473 scouting smoke — the single growing fog scenario (later AI tasks
## extend this same scene/script). Drives the real bridge and controller:
## enemies boot hidden behind runtime fog, a villager march scouts the
## expansion tree and the enemy start into view, marching home hides them
## again while explored resources persist, a stale enemy-building selection
## clears itself, building snapshots stay empty behind fog, the minimap
## never marks a hidden enemy, and a minimap click recenters the camera
## without issuing any gameplay command.

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## Authored fixture geometry (crates/grus-sim/src/fixture.rs).
const OWN_TOWN_CENTER_CELL := Vector2i(12, 46)
const ENEMY_TOWN_CENTER_CELL := Vector2i(112, 46)
const EXPANSION_TREE_ID := 14
const GOLD_ID := 6

## March destinations: the expansion tree at (43,18), a vantage 4-8 cells
## from the enemy start, and open ground back out of vision.
const TREE_VANTAGE := Vector2(44.5, 19.5)
const ENEMY_VANTAGE := Vector2(108.5, 44.5)
const HOME_CELL := Vector2(95.5, 44.5)

var _camera: Camera3D
var _minimap: Control

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

func _resource_view(id: int) -> Node3D:
	return _find_view("resource_views", "resource_id", id)

func _shown(view: Node3D) -> bool:
	return view != null and view.is_visible_in_tree()

func _hidden(view: Node3D) -> bool:
	return view != null and not view.is_visible_in_tree()

func _all_enemies_hidden() -> bool:
	if not _hidden(_building_view(2)):
		return false
	for id in range(5, 9):
		if not _hidden(_unit_view(id)):
			return false
	return true

func _fixture_views_settled() -> bool:
	var units := get_tree().get_nodes_in_group("unit_views")
	var buildings := get_tree().get_nodes_in_group("building_views")
	var resources := get_tree().get_nodes_in_group("resource_views")
	if units.size() != 8 or buildings.size() != 2 or resources.size() != 18:
		return false
	# Views only join the pickable world once identity metadata (and the
	# same-pass visibility stamp) has run — wait for that, not bare nodes.
	for unit in units:
		if int(unit.get_meta("unit_id", -1)) <= 0:
			return false
	for building in buildings:
		if int(building.get_meta("building_id", -1)) <= 0:
			return false
	for resource in resources:
		if int(resource.get_meta("resource_id", -1)) <= 0:
			return false
	return true

## A queued move is not acceptance: the sim's own feedback must say so.
func _accepted_since(revision: int) -> bool:
	return int(GrusBridge.command_feedback_revision()) > revision \
		and str(GrusBridge.command_feedback()).begins_with("Command accepted")

func _move_scout(unit_id: int, target: Vector2, what: String) -> bool:
	var revision_before := int(GrusBridge.command_feedback_revision())
	if not GrusBridge.move_units(PackedInt32Array([unit_id]), target):
		_fail("%s: move was not queued" % what)
		return false
	return await _wait_until(_accepted_since.bind(revision_before), 10.0,
			"%s: move was not accepted by the sim" % what)

func _cell_state(states: PackedInt32Array, cell: Vector2i) -> int:
	return states[cell.y * 128 + cell.x]

func _team_unit_views(team: int) -> Array:
	var views := []
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == team:
			views.append(node)
	return views

## Minimap enemy markers must equal the currently presented enemy views —
## the AI keeps training and moving units, so a marker count of zero is not
## a stable expectation; the fog contract is.
func _presented_enemy_view_count() -> int:
	var count := 0
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == 2 and _shown(node as Node3D):
			count += 1
	for node in get_tree().get_nodes_in_group("building_views"):
		if int(node.get_meta("team_id", -1)) == 2 and _shown(node as Node3D):
			count += 1
	return count

func _run() -> void:
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame
	_camera = get_node("Main/Camera3D") as Camera3D
	_minimap = get_node("Main/HUD/Minimap") as Control

	if not GrusBridge.set_sim_speed(20.0):
		_fail("failed to accelerate the sim")
		return

	if not await _wait_until(_fixture_views_settled, 20.0,
			"skirmish startup never settled to 8 units / 2 buildings / 18 resources"):
		return

	# Runtime fog state is live from setup: revision >= 1, full payload, own
	# start Visible, enemy start Unexplored.
	if not await _wait_until(func(): return int(GrusBridge.visibility_revision()) >= 1, 10.0,
			"runtime visibility revision never advanced past 0"):
		return
	var snap: Dictionary = GrusBridge.visibility_snapshot()
	if int(snap.get("revision", -1)) < 1 or int(snap.get("width", -1)) != 128 \
			or int(snap.get("height", -1)) != 96:
		_fail("visibility snapshot metadata is wrong: %s" % [snap.keys()])
		return
	var states: PackedInt32Array = snap.get("states", PackedInt32Array())
	if states.size() != 128 * 96:
		_fail("visibility snapshot payload is not 128x96: %d" % states.size())
		return
	if _cell_state(states, OWN_TOWN_CENTER_CELL) != 2:
		_fail("own Town Center cell did not boot Visible")
		return
	if _cell_state(states, ENEMY_TOWN_CENTER_CELL) != 0:
		_fail("enemy Town Center cell did not boot Unexplored")
		return

	# Initial presentation: enemies hidden (but alive in the view tree),
	# friendlies and explored starting resources visible.
	for id in range(5, 9):
		if not _hidden(_unit_view(id)):
			_fail("enemy villager %d did not boot hidden" % id)
			return
	if not _hidden(_building_view(2)):
		_fail("enemy Town Center view did not boot hidden")
		return
	for id in range(1, 5):
		if not _shown(_unit_view(id)):
			_fail("friendly villager %d is not visible" % id)
			return
	if not _shown(_building_view(1)):
		_fail("own Town Center view is not visible")
		return
	if not _shown(_resource_view(GOLD_ID)):
		_fail("starting gold view is not visible")
		return
	if not _hidden(_resource_view(EXPANSION_TREE_ID)):
		_fail("expansion tree view did not boot hidden")
		return
	# Fog gates the HUD: a hidden enemy building must not expose its state.
	if not GrusBridge.building_snapshot(2).is_empty():
		_fail("building_snapshot(2) leaked data for a hidden enemy building")
		return
	# The minimap carries no hidden-enemy marker.
	if int(_minimap.enemy_marker_count) != 0:
		_fail("minimap drew enemy markers while every enemy is hidden")
		return
	if int(_minimap.friendly_marker_count) < 4:
		_fail("minimap lost friendly markers")
		return

	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return

	# The Team-2 economic AI ships inside normal setup and this smoke never
	# disables it. Ordinary AI progress must show up as a fifth Team-2 unit
	# view: Team 2 boots with exactly four villagers, and only a live
	# controller can queue and pay for a replacement through production.
	if not await _wait_until(func(): return _team_unit_views(2).size() >= 5, 90.0,
			"the Team-2 AI never trained its replacement villager"):
		return

	# Scout leg 1: reveal the expansion tree.
	if not await _move_scout(1, TREE_VANTAGE, "tree scout"):
		return
	if not await _wait_until(func(): return _shown(_resource_view(EXPANSION_TREE_ID)), 60.0,
			"scout never revealed the expansion tree"):
		return

	# Scout leg 2: reveal the enemy start.
	if not await _move_scout(1, ENEMY_VANTAGE, "enemy scout"):
		return
	if not await _wait_until(func(): return _shown(_building_view(2)) and _shown(_unit_view(5)), 60.0,
			"scout never revealed the enemy Town Center/villagers"):
		return
	var enemy_snap: Dictionary = GrusBridge.building_snapshot(2)
	if enemy_snap.is_empty() or int(enemy_snap.get("team_id", -1)) != 2:
		_fail("visible enemy Town Center snapshot is missing or wrong: %s" % [enemy_snap])
		return
	if int(_minimap.enemy_marker_count) < 1:
		_fail("minimap shows no enemy markers after the reveal")
		return
	# A visible enemy building may be selected, and reconcile keeps it.
	_main_selected_building(2)
	for _frame in 3:
		await get_tree().process_frame
	if int(_main_selected_building()) != 2:
		_fail("reconcile dropped the selection of a visible enemy building")
		return

	# Scout leg 3: march home — vision shrinks, enemies hide, exploration
	# persists.
	if not await _move_scout(1, HOME_CELL, "return march"):
		return
	if not await _wait_until(_all_enemies_hidden, 60.0,
			"the enemy start never fully hid after the scout left"):
		return
	for id in range(5, 9):
		if not _hidden(_unit_view(id)):
			_fail("enemy villager %d did not hide after the scout left" % id)
			return
	if not _shown(_resource_view(EXPANSION_TREE_ID)):
		_fail("explored expansion tree lost its view after leaving vision")
		return
	if int(_main_selected_building()) != -1:
		_fail("stale selected enemy building was never cleared once hidden")
		return
	if not GrusBridge.building_snapshot(2).is_empty():
		_fail("building_snapshot(2) leaked data after the enemy hid again")
		return
	# The hidden enemy Town Center must not be a marker. The live AI keeps
	# training/moving Team-2 units, so markers are checked against the fog
	# contract (only presented enemies may be marked) instead of a brittle
	# count of zero.
	for _frame in 2:
		await get_tree().process_frame
	if int(_minimap.enemy_marker_count) != _presented_enemy_view_count():
		_fail("minimap enemy markers (%d) do not match presented enemy views (%d)"
				% [int(_minimap.enemy_marker_count), _presented_enemy_view_count()])
		return

	# Minimap click recenters the camera and never queues a command.
	var feedback_revision := int(GrusBridge.command_feedback_revision())
	var click_at := _minimap.get_global_rect().position + Vector2(20, 80)
	var press := InputEventMouseButton.new()
	press.button_index = MOUSE_BUTTON_LEFT
	press.position = click_at
	press.pressed = true
	# push_input drives the same Window->Viewport GUI routing an OS click
	# takes; parse_input_event's buffered path is unreliable at this depth in
	# headless runs.
	get_tree().root.push_input(press)
	var release := InputEventMouseButton.new()
	release.button_index = MOUSE_BUTTON_LEFT
	release.position = click_at
	release.pressed = false
	get_tree().root.push_input(release)
	await get_tree().process_frame
	await get_tree().process_frame
	if not _camera.position.is_equal_approx(Vector3(20.5, _camera.position.y, 80.5)):
		_fail("minimap click did not recenter the camera: %s" % [_camera.position])
		return
	if int(GrusBridge.command_feedback_revision()) != feedback_revision:
		_fail("minimap click leaked a gameplay command")
		return

	if not GrusBridge.set_sim_speed(1.0):
		_fail("failed to restore sim speed")
		return

	print("GRUS_SCOUTING_AI_SMOKE_OK boot_hidden=enemies_tc initial_reveal=own_start scout_reveal=tree14_enemy_start hide_on_leave=true explored_persist=true stale_selection_cleared=true snapshot_fog_gated=true minimap_markers=fog_gated recenter=command_free ai_progress=replacement_trained ai_present=true revision=%d" % int(GrusBridge.visibility_revision()))
	get_tree().quit(0)

func _main_selected_building(value := -99) -> int:
	var main := get_node("Main") as Node
	if value != -99:
		main.set("selected_building_id", value)
	return int(main.get("selected_building_id"))
