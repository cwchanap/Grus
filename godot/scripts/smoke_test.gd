extends Node

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

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
		if units.size() == 200:
			break

	if units.size() != 200:
		_fail("expected 200 ECS-backed unit views, found %d" % units.size())
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
		enemy = _find_unit(units, 101)
		if unit_one != null and unit_two != null and unit_three != null and enemy != null:
			break

	if unit_one == null or unit_two == null or unit_three == null or enemy == null:
		_fail("stable UnitId metadata did not initialize for retained fixture")
		return

	var cadence_start := unit_three.global_position
	if not GrusBridge.move_units(PackedInt32Array([3]), Vector2(36.5, 44.5)):
		_fail("20 Hz cadence probe move was rejected")
		return
	await get_tree().create_timer(1.0).timeout
	var cadence_distance := unit_three.global_position.distance_to(cadence_start)
	if cadence_distance < 8.0 or cadence_distance > 16.0:
		_fail("authoritative simulation is not running near 20 Hz: speed-12 unit moved %.2f units in one second" % cadence_distance)
		return
	if not GrusBridge.stop_units(PackedInt32Array([3])):
		_fail("20 Hz cadence probe stop was rejected")
		return
	for _frame in range(2):
		await get_tree().physics_frame

	var controller := get_node("Main")
	var camera := get_node("Main/Camera3D") as Camera3D
	var input_shield := get_node_or_null("Main/HUD/InputShield") as Control
	var clear_screen := Vector2(viewport_size.x * 0.5, viewport_size.y - 40.0)
	var first_click := camera.unproject_position(unit_one.global_position)
	_mouse_click(first_click, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame

	if not _ring(unit_one).visible:
		var nearest_id := -1
		var nearest_distance := INF
		for unit in get_tree().get_nodes_in_group("unit_views"):
			if int(unit.get_meta("team_id", -1)) != 1:
				continue
			var distance := camera.unproject_position((unit as Node3D).global_position).distance_to(first_click)
			if distance < nearest_distance:
				nearest_distance = distance
				nearest_id = int(unit.get_meta("unit_id", -1))
		_fail("left-click diagnostics point=%s shield=%s shield_hit=%s selected=%s nearest=%d distance=%.2f unit1_pos=%s" % [first_click, input_shield.get_global_rect() if input_shield != null else Rect2(), input_shield.get_global_rect().has_point(first_click) if input_shield != null else false, controller.selected_ids, nearest_id, nearest_distance, unit_one.global_position])
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
		_fail("Shift-click did not add a second friendly unit")
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
		_fail("control-group recall did not restore selected stable IDs")
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
		_fail("box selection did not include projected friendly units")
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

	if input_shield == null:
		_fail("HUD input shield is missing")
		return
	var feedback_before_ui := int(GrusBridge.command_feedback_revision())
	_mouse_click(input_shield.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_two).visible or int(GrusBridge.command_feedback_revision()) != feedback_before_ui:
		_fail("HUD click leaked into world controls")
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

	print("GRUS_GODOT_SMOKE_OK units=200 controls=input-derived")
	get_tree().quit(0)
