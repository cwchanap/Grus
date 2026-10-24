extends Node

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

## The grid cell under a live view — fog cells come from presented views
## (stable ids), never a copied fixture coordinate.
func _cell_of(view: Node3D) -> Vector2i:
	return Vector2i(int(view.global_position.x), int(view.global_position.z))

func _building_view(id: int) -> Node3D:
	for node in get_tree().get_nodes_in_group("building_views"):
		if int(node.get_meta("building_id", -1)) == id:
			return node as Node3D
	return null

## Normal gameplay must actually carry visibility state: a revision >= 1
## (initial refresh ran), a 128x96 packed payload, the own Research Center
## cell (stable BuildingId 1) Visible and the enemy Research Center cell
## (BuildingId 2) Unexplored. If the runtime ever lost its VisibilityMap,
## the absent-resource fallback would silently run full-information —
## these assertions make that a loud failure.
func _assert_runtime_visibility(where: String) -> bool:
	if not GrusBridge.has_method("visibility_revision") \
			or not GrusBridge.has_method("visibility_snapshot"):
		_fail("%s: visibility bridge methods are missing" % where)
		return false
	var own := _building_view(1)
	var enemy := _building_view(2)
	if own == null or enemy == null:
		_fail("%s: Research Center views missing for the fog cells" % where)
		return false
	var revision := int(GrusBridge.visibility_revision())
	if revision < 1:
		_fail("%s: runtime visibility state absent (revision %d) — runtime must not run full-info" % [where, revision])
		return false
	var snap: Dictionary = GrusBridge.visibility_snapshot()
	if int(snap.get("revision", -1)) != revision \
			or int(snap.get("width", -1)) != 128 or int(snap.get("height", -1)) != 96:
		_fail("%s: visibility snapshot metadata mismatch: %s" % [where, snap.keys()])
		return false
	var states: PackedInt32Array = snap.get("states", PackedInt32Array())
	if states.size() != 128 * 96:
		_fail("%s: visibility snapshot payload is not 128x96: %d" % [where, states.size()])
		return false
	var own_cell := _cell_of(own)
	if states[own_cell.y * 128 + own_cell.x] != 2:
		_fail("%s: own Research Center cell %s is not Visible" % [where, own_cell])
		return false
	var enemy_cell := _cell_of(enemy)
	if states[enemy_cell.y * 128 + enemy_cell.x] != 0:
		_fail("%s: enemy Research Center cell %s is not Unexplored" % [where, enemy_cell])
		return false

	# The discrete tile overlay must mirror the same states exactly —
	# compacted row-major instance transforms, visible only while non-empty
	# — and stay explicitly non-interpolated: setup/reset/reveal/hide
	# rebuilds all run in _process while the project enables physics
	# interpolation globally, so inherited interpolation would lerp
	# reassigned tile indices and warn on the outside-physics writes.
	var fog := get_node("Main/Battlefield/FogOfWar")
	var synced := false
	for _frame in range(30):
		var revision_now := int(GrusBridge.visibility_revision())
		snap = GrusBridge.visibility_snapshot()
		if int(snap.get("revision", -1)) == revision_now \
				and int(fog.get("_revision")) == revision_now:
			synced = true
			break
		await get_tree().process_frame
	if not synced:
		_fail("%s: fog overlay never consumed the bridge visibility revision" % where)
		return false
	states = snap.get("states", PackedInt32Array())
	var layers: Array = [fog.get("_unexplored"), fog.get("_explored")]
	if layers[0] == null or layers[1] == null:
		_fail("%s: fog overlay layers are missing" % where)
		return false
	if not bool(ProjectSettings.get_setting("physics/common/physics_interpolation", false)):
		_fail("%s: project physics interpolation is disabled — the non-interpolated overlay contract needs the global setting" % where)
		return false
	for layer in layers:
		if layer.physics_interpolation_mode != Node.PHYSICS_INTERPOLATION_MODE_OFF \
				or layer.is_physics_interpolated_and_enabled():
			_fail("%s: fog overlay layer is not explicitly non-interpolated (physics_interpolation_mode=%d interpolated=%s)" % [where, layer.physics_interpolation_mode, layer.is_physics_interpolated_and_enabled()])
			return false
	for state in layers.size():
		var layer := layers[state] as MultiMeshInstance3D
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
			if x >= 128:
				x = 0
				y += 1
	return true

func _run() -> void:
	var units: Array[Node] = []
	var buildings: Array[Node] = []
	var resources: Array[Node] = []
	for _frame in range(160):
		await get_tree().physics_frame
		await get_tree().process_frame
		units = get_tree().get_nodes_in_group("unit_views")
		buildings = get_tree().get_nodes_in_group("building_views")
		resources = get_tree().get_nodes_in_group("resource_views")
		# Group membership lands at instantiation; the `building_id` meta is
		# stamped one bridge Update later — wait for the actual views the
		# fog assertions below look up, not just the count.
		if units.size() == 8 and buildings.size() == 2 and resources.size() == 18 \
				and _building_view(1) != null and _building_view(2) != null:
			break

	if units.size() != 8 or buildings.size() != 2 or resources.size() != 18:
		_fail("skirmish startup expected 8 units, 2 buildings, 18 resources; found %d/%d/%d" % [units.size(), buildings.size(), resources.size()])
		return
	if not GrusBridge.has_method("reset_fixture"):
		_fail("fixture reset bridge is missing")
		return
	if not await _assert_runtime_visibility("setup"):
		return
	if not GrusBridge.reset_fixture():
		_fail("fixture reset bridge rejected reset")
		return

	var unique_ids: Dictionary = {}
	var building_ids: Dictionary = {}
	var resource_ids: Dictionary = {}
	for _frame in range(160):
		await get_tree().physics_frame
		await get_tree().process_frame
		units = get_tree().get_nodes_in_group("unit_views")
		buildings = get_tree().get_nodes_in_group("building_views")
		resources = get_tree().get_nodes_in_group("resource_views")
		unique_ids.clear()
		building_ids.clear()
		resource_ids.clear()
		for unit in units:
			var id := int(unit.get_meta("unit_id", -1))
			if id > 0:
				unique_ids[id] = true
		for building in buildings:
			var id := int(building.get_meta("building_id", -1))
			if id > 0:
				building_ids[id] = true
		for resource in resources:
			var id := int(resource.get_meta("resource_id", -1))
			if id > 0:
				resource_ids[id] = true
		if units.size() == 8 and unique_ids.size() == 8 \
				and buildings.size() == 2 and building_ids.size() == 2 \
				and resources.size() == 18 and resource_ids.size() == 18:
			break

	if units.size() != 8 or unique_ids.size() != 8:
		_fail("fixture reset left duplicate/stale unit views: nodes=%d unique_ids=%d" % [units.size(), unique_ids.size()])
		return
	if buildings.size() != 2 or building_ids.size() != 2:
		_fail("fixture reset expected 2 unique building views: nodes=%d unique_ids=%d" % [buildings.size(), building_ids.size()])
		return
	if resources.size() != 18 or resource_ids.size() != 18:
		_fail("fixture reset expected 18 unique resource views: nodes=%d unique_ids=%d" % [resources.size(), resource_ids.size()])
		return
	for id in range(1, 9):
		if not unique_ids.has(id):
			_fail("fixture reset lost stable UnitId %d" % id)
			return
	for id in range(1, 3):
		if not building_ids.has(id):
			_fail("fixture reset lost stable BuildingId %d" % id)
			return
	for id in range(1, 19):
		if not resource_ids.has(id):
			_fail("fixture reset lost stable ResourceId %d" % id)
			return
	for building in buildings:
		if str(building.get_meta("building_kind", "")) != "ResearchCenter":
			_fail("building view %d is not a Research Center" % int(building.get_meta("building_id", -1)))
			return
	# Post-reset fog rides the settled view tree: the fresh fixture must
	# boot the same 128x96 visibility payload as the first setup.
	if not await _assert_runtime_visibility("restart"):
		return

	# A normal-skirmish reset returns the session to Start.
	if not GrusBridge.has_method("session_snapshot"):
		_fail("session snapshot bridge is missing")
		return
	var session: Dictionary = GrusBridge.session_snapshot()
	if str(session.get("phase", "")) != "Start":
		_fail("reset did not return the session to Start: %s" % [session])
		return

	var economy: Dictionary = GrusBridge.economy_snapshot()
	if int(economy.get("food", -1)) != 200 or int(economy.get("scrap", -1)) != 300 or int(economy.get("coal", -1)) != 150 or int(economy.get("crystals", -1)) != 0:
		_fail("Team 1 stockpile after reset is not 200/300/150/0: %s" % [economy])
		return
	if int(economy.get("population_used", -1)) != 4 or int(economy.get("population_cap", -1)) != 10:
		_fail("Team 1 population after reset is not 4/10: %s" % [economy])
		return

	print("GRUS_RESET_SMOKE_OK units=8 buildings=2 resources=18 stockpile=200/300/150/0 population=4/10 fog=128x96")
	get_tree().quit(0)
