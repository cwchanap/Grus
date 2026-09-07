extends Node

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

func _mouse_click(position: Vector2, button: int) -> void:
	var press := InputEventMouseButton.new()
	press.button_index = button
	press.position = position
	press.pressed = true
	Input.parse_input_event(press)

	var release := InputEventMouseButton.new()
	release.button_index = button
	release.position = position
	release.pressed = false
	Input.parse_input_event(release)

func _run() -> void:
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
	for _frame in range(30):
		await get_tree().process_frame
		units = get_tree().get_nodes_in_group("unit_views")
		for unit in units:
			if int(unit.get_meta("unit_id", -1)) == 1:
				unit_one = unit as Node3D
				break
		if unit_one != null:
			break

	if unit_one == null:
		_fail("unit 1 never received stable UnitId metadata")
		return

	var camera := get_node("Main/Camera3D") as Camera3D
	_mouse_click(camera.unproject_position(unit_one.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame

	var ring := unit_one.get_node("SelectionRing") as MeshInstance3D
	if not ring.visible:
		_fail("left-click input did not select friendly unit 1")
		return

	var start := unit_one.global_position
	var target := Vector3(30.5, 0.0, 48.5)
	_mouse_click(camera.unproject_position(target), MOUSE_BUTTON_RIGHT)

	for _frame in range(80):
		await get_tree().physics_frame

	if unit_one.global_position.distance_to(start) <= 0.5:
		_fail("right-click input did not move unit 1 through ECS")
		return

	print("GRUS_GODOT_SMOKE_OK units=200 moved=", unit_one.global_position.distance_to(start))
	get_tree().quit(0)
