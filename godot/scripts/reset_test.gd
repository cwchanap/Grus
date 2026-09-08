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
		_fail("reset precondition expected 200 unit views, found %d" % units.size())
		return
	if not GrusBridge.has_method("reset_fixture"):
		_fail("fixture reset bridge is missing")
		return
	if not GrusBridge.reset_fixture():
		_fail("fixture reset bridge rejected reset")
		return

	var unique_ids: Dictionary = {}
	for _frame in range(160):
		await get_tree().physics_frame
		await get_tree().process_frame
		units = get_tree().get_nodes_in_group("unit_views")
		unique_ids.clear()
		for unit in units:
			var id := int(unit.get_meta("unit_id", -1))
			if id > 0:
				unique_ids[id] = true
		if units.size() == 200 and unique_ids.size() == 200:
			break

	if units.size() != 200 or unique_ids.size() != 200:
		_fail("fixture reset left duplicate/stale views: nodes=%d unique_ids=%d" % [units.size(), unique_ids.size()])
		return
	for id in range(1, 201):
		if not unique_ids.has(id):
			_fail("fixture reset lost stable UnitId %d" % id)
			return

	print("GRUS_RESET_SMOKE_OK units=200 unique_ids=200")
	get_tree().quit(0)
