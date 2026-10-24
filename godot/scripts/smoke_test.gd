extends Node

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## Stable ids for the fog-scouting leg — positions are read off the live
## views at run time, never copied from the fixture into GDScript: the
## expansion salvage ruin (ResourceId 14) and the passive eastern
## Research Center (BuildingId 2).
const EXPANSION_SALVAGE_ID := 14
const ENEMY_RESEARCH_CENTER_ID := 2

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

func _modifier_event(keycode: int, pressed: bool) -> void:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.pressed = pressed
	Input.parse_input_event(event)

func _mouse_click(position: Vector2, button: int, shift_pressed := false) -> void:
	var press := InputEventMouseButton.new()
	press.button_index = button
	press.position = position
	press.pressed = true
	press.shift_pressed = shift_pressed
	Input.parse_input_event(press)

	var release := InputEventMouseButton.new()
	release.button_index = button
	release.position = position
	release.pressed = false
	release.shift_pressed = shift_pressed
	Input.parse_input_event(release)

func _mouse_drag(start: Vector2, finish: Vector2, button: int) -> void:
	var press := InputEventMouseButton.new()
	press.button_index = button
	press.position = start
	press.pressed = true
	Input.parse_input_event(press)

	var motion := InputEventMouseMotion.new()
	motion.position = finish
	motion.relative = finish - start
	if button == MOUSE_BUTTON_LEFT:
		motion.button_mask = MOUSE_BUTTON_MASK_LEFT
	elif button == MOUSE_BUTTON_MIDDLE:
		motion.button_mask = MOUSE_BUTTON_MASK_MIDDLE
	Input.parse_input_event(motion)

	var release := InputEventMouseButton.new()
	release.button_index = button
	release.position = finish
	release.pressed = false
	Input.parse_input_event(release)

func _key_tap(keycode: int, ctrl_pressed := false) -> void:
	var press := InputEventKey.new()
	press.keycode = keycode
	press.pressed = true
	press.ctrl_pressed = ctrl_pressed
	Input.parse_input_event(press)

	var release := InputEventKey.new()
	release.keycode = keycode
	release.pressed = false
	release.ctrl_pressed = ctrl_pressed
	Input.parse_input_event(release)

func _find_unit(units: Array[Node], id: int) -> Node3D:
	for unit in units:
		if int(unit.get_meta("unit_id", -1)) == id:
			return unit as Node3D
	return null

func _ring(unit: Node3D) -> MeshInstance3D:
	return unit.get_node("SelectionRing") as MeshInstance3D

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

## Fog overlay buffers are authoritative only once the overlay's _process
## consumed the same visibility revision the bridge reports; a marching
## scout keeps the revision ticking ahead of it. Returns the snapshot the
## overlay actually rendered, or {} after a loud failure.
func _settled_fog_snapshot(where: String) -> Dictionary:
	var fog := get_node("Main/Battlefield/FogOfWar")
	for _attempt in range(8):
		var synced := func() -> bool:
			var revision := int(GrusBridge.visibility_revision())
			return revision >= 0 and int(fog.get("_revision")) == revision
		if not await _wait_until(synced, 30.0,
				"%s: fog overlay never consumed the bridge visibility revision" % where):
			return {}
		var snap: Dictionary = GrusBridge.visibility_snapshot()
		if int(snap.get("revision", -1)) == int(fog.get("_revision")):
			return snap
		# The revision moved during the read; the next attempt catches up.
	_fail("%s: visibility snapshot never matched the consumed fog revision" % where)
	return {}

## Fog overlay contract: the two MultiMesh layers are discrete tiles
## compacted in row-major order by state — Unexplored (opaque) and
## Explored (translucent); Visible cells draw no tile. The buffers must
## equal the snapshot exactly — instance count, packed per-tile transform
## floats, layer visibility — and under the project's globally enabled
## physics interpolation each layer must be explicitly non-interpolated:
## a rebuild reassigns instance indices to different cells rather than
## moving a mesh, so an interpolated tile is a wrong tile, and buffer
## writes from _process trip the engine's outside-physics-transform
## warning.
func _assert_fog_overlay(where: String, snap: Dictionary) -> bool:
	var fog := get_node("Main/Battlefield/FogOfWar")
	var layers: Array[MultiMeshInstance3D] = [fog.get("_unexplored"), fog.get("_explored")]
	if layers[0] == null or layers[1] == null:
		_fail("%s: fog overlay layers are missing" % where)
		return false
	if not bool(ProjectSettings.get_setting("physics/common/physics_interpolation", false)):
		_fail("%s: project physics interpolation is disabled — the non-interpolated overlay contract requires the global setting" % where)
		return false
	for layer in layers:
		if layer.physics_interpolation_mode != Node.PHYSICS_INTERPOLATION_MODE_OFF:
			_fail("%s: fog overlay layer is not explicitly non-interpolated (physics_interpolation_mode=%d)" % [where, layer.physics_interpolation_mode])
			return false
		if layer.is_physics_interpolated_and_enabled():
			_fail("%s: fog overlay layer is still physics-interpolated — compacted tiles would lerp between unrelated cells" % where)
			return false
	var width := int(snap.get("width", 0))
	var states: PackedInt32Array = snap.get("states", PackedInt32Array())
	if width <= 0 or states.is_empty() or states.size() % width != 0:
		_fail("%s: malformed visibility snapshot for the overlay check: %s" % [where, snap.keys()])
		return false
	for state in layers.size():
		var layer := layers[state]
		var multimesh := layer.multimesh
		var expected := 0
		for value in states:
			if value == state:
				expected += 1
		if multimesh.instance_count != expected:
			_fail("%s: fog layer %d drew %d tiles; snapshot has %d state-%d cells" % [where, state, multimesh.instance_count, expected, state])
			return false
		if layer.visible != (expected > 0):
			_fail("%s: fog layer %d visibility is %s; expected %s for %d tiles" % [where, state, layer.visible, expected > 0, expected])
			return false
		if expected == 0:
			continue
		# Exact tile buffer: 12 floats per TRANSFORM_3D tile — identity
		# basis (offsets 0/5/10) and the cell-center origin (offsets
		# 3/7/11) in the same row-major compaction order _fill writes.
		# get_buffer round-trips through the stored multimesh data on every
		# renderer, including the headless dummy driver.
		var buffer: PackedFloat32Array = multimesh.get_buffer()
		if buffer.size() != expected * 12:
			_fail("%s: fog layer %d buffer is %d floats; expected %d for %d tiles" % [where, state, buffer.size(), expected * 12, expected])
			return false
		var index := 0
		var x := 0
		var y := 0
		for value in states:
			if value == state:
				var o := index * 12
				var want_origin := Vector3(x + 0.5, float(fog.FOG_Y), y + 0.5)
				var got_origin := Vector3(buffer[o + 3], buffer[o + 7], buffer[o + 11])
				var basis_ok := is_equal_approx(buffer[o], 1.0) and is_zero_approx(buffer[o + 1]) \
						and is_zero_approx(buffer[o + 2]) and is_zero_approx(buffer[o + 4]) \
						and is_equal_approx(buffer[o + 5], 1.0) and is_zero_approx(buffer[o + 6]) \
						and is_zero_approx(buffer[o + 8]) and is_zero_approx(buffer[o + 9]) \
						and is_equal_approx(buffer[o + 10], 1.0)
				if not basis_ok or not got_origin.is_equal_approx(want_origin):
					_fail("%s: fog layer %d tile %d origin %s != cell (%d, %d) at %s" % [where, state, index, got_origin, x, y, want_origin])
					return false
				index += 1
			x += 1
			if x >= width:
				x = 0
				y += 1
	return true

func _run() -> void:
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame

	var viewport_size := get_viewport().get_visible_rect().size
	if viewport_size != Vector2(TEST_VIEWPORT_SIZE):
		_fail("smoke viewport did not resize to project baseline: %s" % viewport_size)
		return

	var units: Array[Node] = []
	for _frame in range(160):
		await get_tree().physics_frame
		units = get_tree().get_nodes_in_group("unit_views")
		if units.size() == 8:
			break

	if units.size() != 8:
		_fail("expected 8 ECS-backed unit views, found %d" % units.size())
		return

	var unit_one: Node3D = null
	var unit_two: Node3D = null
	var unit_three: Node3D = null
	var enemy: Node3D = null
	for _frame in range(30):
		await get_tree().process_frame
		units = get_tree().get_nodes_in_group("unit_views")
		unit_one = _find_unit(units, 1)
		unit_two = _find_unit(units, 2)
		unit_three = _find_unit(units, 3)
		enemy = _find_unit(units, 5)
		if unit_one != null and unit_two != null and unit_three != null and enemy != null:
			break

	if unit_one == null or unit_two == null or unit_three == null or enemy == null:
		_fail("stable UnitId metadata did not initialize for skirmish fixture")
		return
	for id in range(1, 5):
		var member := _find_unit(units, id)
		if member == null or int(member.get_meta("team_id", -1)) != 1:
			_fail("Team 1 worker %d is missing or has the wrong team metadata" % id)
			return
	if int(enemy.get_meta("team_id", -1)) != 2:
		_fail("Team 2 worker does not start at UnitId 5")
		return

	# Normal skirmish boots into Start; gameplay opens only via start_match.
	if not GrusBridge.has_method("start_match") or not GrusBridge.has_method("session_snapshot"):
		_fail("session bridge methods are missing")
		return
	var boot_session: Dictionary = GrusBridge.session_snapshot()
	if str(boot_session.get("phase", "")) != "Start":
		_fail("skirmish boot session is not Start: %s" % [boot_session])
		return
	# First-run controls aid: the Start overlay must surface the controls label
	# before gameplay opens. Tokens match case-insensitively against the label's
	# own text; A/S must appear as standalone words, not substrings.
	var controls_label := get_node("Main/HUD/SessionPanel/ControlsLabel") as Label
	if controls_label == null or not controls_label.is_visible_in_tree():
		_fail("HUD/SessionPanel/ControlsLabel is missing or not visible before Start")
		return
	var controls_text := controls_label.text.to_lower()
	for token in ["right click", "minimap", "esc"]:
		if not controls_text.contains(token):
			_fail("ControlsLabel is missing '%s': %s" % [token, controls_label.text])
			return
	var control_words := controls_text.replace("\n", " ").split(" ", false)
	for token in ["a", "s"]:
		if not token in control_words:
			_fail("ControlsLabel is missing standalone token '%s': %s" % [token, controls_label.text])
			return
	# This scripted choreography runs against a passive Team 2: no opponent
	# controller exists to lift before gameplay opens.
	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return
	await get_tree().process_frame
	if controls_label.is_visible_in_tree():
		_fail("ControlsLabel stayed visible after the match reached Playing")
		return

	var camera := get_node("Main/Camera3D") as Camera3D
	var command_status := get_node("Main/HUD/CommandStatus") as Label
	var clear_screen := Vector2(viewport_size.x * 0.5, viewport_size.y - 40.0)

	# Camera framing contract, before any pan/zoom/minimap interaction: the
	# live Team-1 Research Center view (stable BuildingId 1) must project
	# into the central half of the viewport. The projection is independent —
	# nothing copies the authored anchor into GDScript — so a layout or
	# camera drift fails loudly here.
	var research_center_view: Node3D = null
	for _frame in range(60):
		await get_tree().process_frame
		research_center_view = _building_view(1)
		if research_center_view != null:
			break
	if research_center_view == null:
		_fail("Research Center view 1 never initialized for the camera check")
		return
	var center_screen := camera.unproject_position(research_center_view.global_position)
	if center_screen.x < viewport_size.x * 0.25 or center_screen.x > viewport_size.x * 0.75 \
			or center_screen.y < viewport_size.y * 0.25 or center_screen.y > viewport_size.y * 0.75:
		_fail("central Research Center projects outside the viewport's central half: %s" % center_screen)
		return

	# Rendered-ruins contract: the bridge hands over the same inclusive
	# rectangles the simulation blocks, and the battlefield instantiates
	# exactly one BoxMesh per rect whose size/center derive from the
	# payload — never a copied coordinate.
	var ruin_rects: PackedInt32Array = GrusBridge.obstacle_rects()
	if ruin_rects.is_empty() or ruin_rects.size() % 4 != 0:
		_fail("obstacle_rects payload is malformed: %s" % [ruin_rects])
		return
	var ruins_root := get_node("Main/Battlefield/Ruins") as Node3D
	var ruin_count := ruin_rects.size() / 4
	if ruins_root.get_child_count() != ruin_count:
		_fail("rendered ruin count %d != bridge payload %d" % [ruins_root.get_child_count(), ruin_count])
		return
	for i in ruin_count:
		var min_cell := Vector2i(ruin_rects[i * 4], ruin_rects[i * 4 + 1])
		var max_cell := Vector2i(ruin_rects[i * 4 + 2], ruin_rects[i * 4 + 3])
		var cells := max_cell - min_cell + Vector2i.ONE
		var ruin := ruins_root.get_node("Ruin%d" % i) as MeshInstance3D
		var mesh := ruin.mesh as BoxMesh
		var want_size := Vector3(cells.x, 2.0, cells.y)
		var want_center := Vector3(min_cell.x + cells.x * 0.5, 1.0, min_cell.y + cells.y * 0.5)
		if mesh == null or mesh.size != want_size or ruin.position != want_center:
			_fail("ruin %d rendered %s at %s != bridge-derived %s at %s" % [
				i, mesh.size if mesh != null else Vector3.ZERO, ruin.position, want_size, want_center])
			return

	var size_before_zoom := camera.size
	_mouse_click(Vector2(640.0, 360.0), MOUSE_BUTTON_WHEEL_UP)
	await get_tree().process_frame
	if camera.size >= size_before_zoom:
		_fail("mouse wheel did not zoom the orthographic camera")
		return

	var camera_before_pan := Vector2(camera.position.x, camera.position.z)
	_mouse_drag(Vector2(640.0, 360.0), Vector2(680.0, 385.0), MOUSE_BUTTON_MIDDLE)
	await get_tree().process_frame
	var camera_after_pan := Vector2(camera.position.x, camera.position.z)
	if camera_after_pan.distance_to(camera_before_pan) <= 0.1:
		_fail("middle-drag did not pan the camera")
		return

	_mouse_click(camera.unproject_position(unit_one.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame

	if not _ring(unit_one).visible:
		_fail("left-click input did not select friendly unit 1 after camera pan/zoom")
		return

	var start := unit_one.global_position
	var target := Vector3(30.5, 0.0, 48.5)
	_mouse_click(camera.unproject_position(target), MOUSE_BUTTON_RIGHT)

	for _frame in range(80):
		await get_tree().physics_frame

	if unit_one.global_position.distance_to(start) <= 0.5:
		_fail("right-click input did not move unit 1 through ECS")
		return

	_modifier_event(KEY_SHIFT, true)
	await get_tree().process_frame
	_mouse_click(camera.unproject_position(unit_two.global_position), MOUSE_BUTTON_LEFT, true)
	await get_tree().process_frame
	_modifier_event(KEY_SHIFT, false)
	await get_tree().process_frame
	if not _ring(unit_one).visible or not _ring(unit_two).visible:
		_fail("Shift-click did not add a second friendly unit after camera pan/zoom")
		return

	_modifier_event(KEY_CTRL, true)
	await get_tree().process_frame
	_key_tap(KEY_1, true)
	await get_tree().process_frame
	_modifier_event(KEY_CTRL, false)
	await get_tree().process_frame
	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if _ring(unit_one).visible or _ring(unit_two).visible:
		_fail("clicking outside friendly units did not clear selection")
		return
	_key_tap(KEY_1)
	await get_tree().process_frame
	if not _ring(unit_one).visible or not _ring(unit_two).visible:
		_fail("control-group recall did not restore selected stable IDs after camera pan/zoom")
		return

	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	var screen_two := camera.unproject_position(unit_two.global_position)
	var screen_three := camera.unproject_position(unit_three.global_position)
	var box_start := Vector2(minf(screen_two.x, screen_three.x) - 10.0, minf(screen_two.y, screen_three.y) - 10.0)
	var box_end := Vector2(maxf(screen_two.x, screen_three.x) + 10.0, maxf(screen_two.y, screen_three.y) + 10.0)
	_mouse_drag(box_start, box_end, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_two).visible or not _ring(unit_three).visible:
		_fail("box selection did not include projected friendly units after camera pan/zoom")
		return

	_mouse_click(camera.unproject_position(unit_two.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	var stop_target := Vector3(40.5, 0.0, 44.5)
	_mouse_click(camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	for _frame in range(8):
		await get_tree().physics_frame
	_key_tap(KEY_S)
	for _frame in range(2):
		await get_tree().physics_frame
	var stopped_position := unit_two.global_position
	for _frame in range(15):
		await get_tree().physics_frame
	if unit_two.global_position.distance_to(stopped_position) > 0.05:
		_fail("Stop input did not settle the active move order")
		return

	var input_shield := get_node_or_null("Main/HUD/InputShield") as Control
	if input_shield == null:
		_fail("HUD input shield is missing")
		return
	var feedback_before_ui := int(GrusBridge.command_feedback_revision())
	_mouse_click(input_shield.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_two).visible or int(GrusBridge.command_feedback_revision()) != feedback_before_ui:
		_fail("HUD click leaked into world controls")
		return

	# A genuinely blocked destination: the center of a bridge-reported ruin
	# rectangle (quad 4, west of the city) — real impassable terrain, never
	# the friendly Research Center, and its projection lands on open HUD.
	var ruin_min := Vector2i(ruin_rects[16], ruin_rects[17])
	var ruin_max := Vector2i(ruin_rects[18], ruin_rects[19])
	var ruin_cells := ruin_max - ruin_min + Vector2i.ONE
	var blocked_target := Vector3(
		ruin_min.x + ruin_cells.x * 0.5, 0.0, ruin_min.y + ruin_cells.y * 0.5)
	_mouse_click(camera.unproject_position(blocked_target), MOUSE_BUTTON_RIGHT)
	for _frame in range(3):
		await get_tree().physics_frame
	await get_tree().process_frame
	if not command_status.text.to_lower().contains("unreachable"):
		_fail("blocked destination did not surface unreachable feedback: %s" % command_status.text)
		return

	var cadence_start := unit_three.global_position
	if not GrusBridge.move_units(PackedInt32Array([3]), Vector2(36.5, 44.5)):
		_fail("20 Hz cadence probe move was rejected")
		return
	var cadence_started_ms := Time.get_ticks_msec()
	var previous_visual := unit_three.global_position
	var max_visual_step := 0.0
	var visual_changes := 0
	while Time.get_ticks_msec() - cadence_started_ms < 1000:
		await get_tree().process_frame
		var visual_position := unit_three.global_position
		var visual_step := visual_position.distance_to(previous_visual)
		if visual_step > 0.0001:
			visual_changes += 1
			max_visual_step = maxf(max_visual_step, visual_step)
		previous_visual = visual_position

	var cadence_distance := unit_three.global_position.distance_to(cadence_start)
	if cadence_distance < 4.0 or cadence_distance > 8.0:
		_fail("authoritative simulation is not running near 20 Hz: speed-6 unit moved %.2f units in one second" % cadence_distance)
		return
	# 0.25 is one frame of a speed-6 unit plus margin (about 0.1-0.2 healthy),
	# so any frame hitch over ~40 ms trips it too: on a host under heavy CPU
	# load this fails without an interpolation fault. Read the worst frame
	# time before debugging interpolation.
	if max_visual_step >= 0.25:
		_fail("20 Hz presentation is not interpolated: max visual step %.3f across %d visual changes" % [max_visual_step, visual_changes])
		return

	# HPA-445 fog/minimap contract (moved out of the removed scouting-AI
	# smoke): a human scout — worker 4, untouched by the choreography
	# above — reveals a hidden resource and the passive enemy start, then
	# leaves again. Presentation must track sim fog throughout: enemy views
	# and the HUD data path open only under current vision, explored
	# resources persist, a stale enemy selection clears itself, and enemy
	# minimap markers appear/disappear with the presented views. The
	# minimap click at the end recenters the camera without queuing any
	# gameplay command.
	if not GrusBridge.set_sim_speed(20.0):
		_fail("failed to accelerate the sim for the scouting leg")
		return
	var main := get_node("Main") as Node3D
	var minimap := get_node("Main/HUD/Minimap") as Control

	# Scout targets derive from the live views — the expansion ruin and the
	# enemy Research Center — so the choreography tracks whatever layout
	# the fixture authors instead of a copied coordinate list.
	var salvage_view := _resource_view(EXPANSION_SALVAGE_ID)
	var enemy_view := _building_view(ENEMY_RESEARCH_CENTER_ID)
	if salvage_view == null or enemy_view == null:
		_fail("scout target views are missing (salvage %s, enemy %s)" % [salvage_view, enemy_view])
		return
	var salvage_cell := Vector2i(int(salvage_view.global_position.x), int(salvage_view.global_position.z))
	var enemy_cell := Vector2i(int(enemy_view.global_position.x), int(enemy_view.global_position.z))
	# March destinations: beside the expansion ruin (its own cell is
	# blocked), a vantage inside vision of the enemy start, and open ground
	# far enough east of the base to drop every enemy back out of vision.
	var salvage_vantage := Vector2(salvage_view.global_position.x + 1.5, salvage_view.global_position.z + 1.5)
	var enemy_vantage := Vector2(enemy_view.global_position.x - 5.0, enemy_view.global_position.z - 4.5)
	var home_cell := Vector2(enemy_view.global_position.x - 18.5, enemy_view.global_position.z - 3.5)

	# Pre-scout baseline: the earlier choreography stayed inside own
	# vision, so the enemy start and the expansion salvage are still
	# Unexplored — enemy views hidden (but alive in the view tree), the
	# fog-gated building snapshot empty, and no enemy minimap markers.
	var pre_snap: Dictionary = GrusBridge.visibility_snapshot()
	var pre_states: PackedInt32Array = pre_snap.get("states", PackedInt32Array())
	if pre_states.size() != 128 * 96:
		_fail("visibility snapshot payload is not 128x96: %d" % pre_states.size())
		return
	if _cell_state(pre_states, salvage_cell) != 0 \
			or _cell_state(pre_states, enemy_cell) != 0:
		_fail("scout targets did not stay Unexplored before the leg")
		return
	for id in range(5, 9):
		if not _hidden(_unit_view(id)):
			_fail("enemy worker %d was not hidden before the scout leg" % id)
			return
	if not _hidden(_building_view(2)):
		_fail("enemy Research Center view was not hidden before the scout leg")
		return
	if not _hidden(_resource_view(EXPANSION_SALVAGE_ID)):
		_fail("expansion salvage view was not hidden before the scout leg")
		return
	if not GrusBridge.building_snapshot(2).is_empty():
		_fail("building_snapshot(2) leaked data for a hidden enemy building")
		return
	if int(minimap.enemy_marker_count) != 0:
		_fail("minimap drew enemy markers while every enemy is hidden")
		return
	if int(minimap.friendly_marker_count) < 4:
		_fail("minimap lost friendly markers")
		return

	# The discrete tile overlay itself: buffers must mirror the fog states
	# exactly and stay explicitly non-interpolated under the project's
	# global physics interpolation — see _assert_fog_overlay.
	var fog_snap_pre := await _settled_fog_snapshot("pre-scout")
	if fog_snap_pre.is_empty() or not _assert_fog_overlay("pre-scout", fog_snap_pre):
		return

	# Scout leg 1: reveal the expansion salvage.
	if not await _move_scout(4, salvage_vantage, "salvage scout"):
		return
	if not await _wait_until(func(): return _shown(_resource_view(EXPANSION_SALVAGE_ID)), 60.0,
			"scout never revealed the expansion salvage"):
		return

	# Scout leg 2: reveal the enemy start — views, the fog-gated building
	# snapshot, and minimap markers all open together.
	if not await _move_scout(4, enemy_vantage, "enemy scout"):
		return
	if not await _wait_until(func(): return _shown(_building_view(2)) and _shown(_unit_view(5)), 60.0,
			"scout never revealed the enemy Research Center/workers"):
		return
	var enemy_snap: Dictionary = GrusBridge.building_snapshot(2)
	if enemy_snap.is_empty() or int(enemy_snap.get("team_id", -1)) != 2:
		_fail("visible enemy Research Center snapshot is missing or wrong: %s" % [enemy_snap])
		return
	if int(minimap.enemy_marker_count) < 1:
		_fail("minimap shows no enemy markers after the reveal")
		return
	# A visible enemy building may be selected, and reconcile keeps it.
	# Deliberate test seam: the selection is poked directly because the
	# enemy Research Center boots hidden under fog, and the real picking funnel
	# (battlefield _select_at) rightly skips not-visible-in-tree views —
	# selecting it through a real click would first require marching the
	# scout into the enemy start, machinery the fog contract under test
	# does not need.
	main.set("selected_building_id", 2)
	for _frame in 3:
		await get_tree().process_frame
	if int(main.get("selected_building_id")) != 2:
		_fail("reconcile dropped the selection of a visible enemy building")
		return

	# Reveal transition: the compacted buffers must track the live states
	# with no lerped tiles — the scout may still be marching, so the check
	# samples whatever settled revision the overlay actually consumed.
	var fog_snap_revealed := await _settled_fog_snapshot("revealed")
	if fog_snap_revealed.is_empty() or not _assert_fog_overlay("revealed", fog_snap_revealed):
		return

	# Scout leg 3: march home — vision shrinks, enemies hide, exploration
	# persists. With a passive Team 2 nothing can wander back into vision,
	# so the hidden enemies' marker count must return to exactly zero.
	if not await _move_scout(4, home_cell, "return march"):
		return
	if not await _wait_until(_all_enemies_hidden, 60.0,
			"the enemy start never fully hid after the scout left"):
		return
	if not _shown(_resource_view(EXPANSION_SALVAGE_ID)):
		_fail("explored expansion salvage lost its view after leaving vision")
		return
	if int(main.get("selected_building_id")) != -1:
		_fail("stale selected enemy building was never cleared once hidden")
		return
	if not GrusBridge.building_snapshot(2).is_empty():
		_fail("building_snapshot(2) leaked data after the enemy hid again")
		return
	for _frame in 2:
		await get_tree().process_frame
	if int(minimap.enemy_marker_count) != 0:
		_fail("minimap kept enemy markers after every enemy hid")
		return
	# The fog data agrees: scouted cells keep Explored instead of snapping
	# back to Unexplored.
	var fog_snap: Dictionary = GrusBridge.visibility_snapshot()
	var fog_states: PackedInt32Array = fog_snap.get("states", PackedInt32Array())
	if fog_states.size() != 128 * 96:
		_fail("visibility snapshot lost its payload")
		return
	if _cell_state(fog_states, salvage_cell) != 1 \
			or _cell_state(fog_states, enemy_cell) != 1:
		_fail("explored fog did not persist after the scout left")
		return

	# Hide transition: exploration persists as state 1 tiles — same exact
	# buffer contract on the other side of the reveal.
	var fog_snap_hidden := await _settled_fog_snapshot("hidden")
	if fog_snap_hidden.is_empty() or not _assert_fog_overlay("hidden", fog_snap_hidden):
		return

	# Minimap click recenters the camera and never queues a command.
	var feedback_revision := int(GrusBridge.command_feedback_revision())
	var click_at := minimap.get_global_rect().position + Vector2(20, 80)
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
	# The camera is pitched down: the click must center the *viewed* ground
	# cell — where the view axis hits y=0 — computed from the logical
	# transform because `project_ray_*` read the physics-interpolated one.
	var forward := -camera.global_transform.basis.z.normalized()
	var ground_hit := camera.global_position + forward * (-camera.global_position.y / forward.y)
	if not Vector2(ground_hit.x, ground_hit.z).is_equal_approx(Vector2(20.5, 80.5)):
		_fail("minimap click did not center the clicked ground cell: %s" % [ground_hit])
		return
	if int(GrusBridge.command_feedback_revision()) != feedback_revision:
		_fail("minimap click leaked a gameplay command")
		return

	if not GrusBridge.set_sim_speed(1.0):
		_fail("failed to restore sim speed")
		return

	print("GRUS_GODOT_SMOKE_OK units=8 controls=input-derived camera=pan-zoomed unreachable=feedback cadence=20hz interpolation=max_step_%.3f fog=scout_reveal_hide explored_persist=true stale_selection_cleared=true markers=fog_gated recenter=command_free" % max_visual_step)
	get_tree().quit(0)
