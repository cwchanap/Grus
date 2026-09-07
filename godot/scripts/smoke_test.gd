extends Node

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

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
	for unit in units:
		if int(unit.get_meta("unit_id", -1)) == 1:
			unit_one = unit as Node3D
			break

	if unit_one == null:
		_fail("unit 1 never received stable UnitId metadata")
		return

	var start := unit_one.global_position
	if not GrusBridge.move_units(PackedInt32Array([1]), Vector2(30.5, 48.5)):
		_fail("Rust command bridge rejected a valid move request")
		return

	for _frame in range(80):
		await get_tree().physics_frame

	if unit_one.global_position.distance_to(start) <= 0.5:
		_fail("command bridge did not move unit 1 through ECS")
		return

	print("GRUS_GODOT_SMOKE_OK units=200 moved=", unit_one.global_position.distance_to(start))
	get_tree().quit(0)
