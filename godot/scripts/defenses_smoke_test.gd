extends "res://scripts/combat_lifecycle_smoke_test.gd"

## HPA-450 integrated journey. Defences are placed through the real build UI;
## only the wave clock is accelerated, through the normal sim-speed bridge.

var _defense_ids: Array[int] = []
var _ring_min := Vector2i.ZERO
var _ring_max := Vector2i.ZERO
var _gate_cell := Vector2i.ZERO
var _health_at_gate_breach: Dictionary = {}
var _turret_tracer_seen := false
var _turret_crystal_kill_seen := false

func _run() -> void:
	var journey_start_msec := Time.get_ticks_msec()
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame
	_main = get_node("Main") as Node3D
	_fx = _main.get_node("CombatFx") as Node3D
	_minimap = _main.get_node("HUD/Minimap") as Control

	if not GrusBridge.set_sim_speed(20.0):
		_fail("could not accelerate the defense journey")
		return
	if not await _wait_until(_fixture_views_settled, 20.0,
			"one-base fixture did not settle before defense placement"):
		return
	if not GrusBridge.start_match():
		_fail("Start did not enter Playing")
		return
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Playing",
			5.0, "session did not enter Playing"):
		return

	var research_center := _building_view(1)
	var builder := _find_view("unit_views", "unit_id", 4)
	if research_center == null or builder == null:
		_fail("live Research Center or Worker view is missing")
		return
	var catalog: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {})
	var wall_spec: Dictionary = catalog.get("Wall", {})
	var gate_spec: Dictionary = catalog.get("Gate", {})
	var turret_spec: Dictionary = catalog.get("GunTurret", {})
	if wall_spec.is_empty() or gate_spec.is_empty() or turret_spec.is_empty():
		_fail("catalogue snapshot omitted a defensive building")
		return

	var rc_anchor := SmokeHelpers.footprint_anchor(research_center)
	_ring_min = rc_anchor - Vector2i(2, 2)
	_ring_max = rc_anchor + Vector2i(int(research_center.scale.x) + 1, int(research_center.scale.z) + 1)
	_gate_cell = Vector2i(
		rc_anchor.x + floori(research_center.scale.x / 2.0), _ring_min.y)
	var ring_cells := _perimeter_cells()
	var required_scrap := int(gate_spec.get("scrap", 0)) \
		+ (ring_cells.size() - 1) * int(wall_spec.get("scrap", 0))
	if int(GrusBridge.economy_snapshot().get("scrap", 0)) < required_scrap:
		_fail("fixture scrap cannot afford the authored wall ring: need %d" % required_scrap)
		return

	if not await _select_worker(builder):
		return
	var gate_before := _building_ids_of_kind("Gate").size()
	if not await _arm_build("Gate"):
		return
	var gate_feedback_revision := int(GrusBridge.command_feedback_revision())
	await _click_cell(_gate_cell, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func(): return _building_ids_of_kind("Gate").size() == gate_before + 1 \
			and int(GrusBridge.command_feedback_revision()) > gate_feedback_revision,
			8.0, "Gate placement did not appear through the controller"):
		return
	if str(_main._placement_kind) != "Gate":
		_fail("successful Gate placement did not keep placement armed")
		return
	var placement_revision := int(GrusBridge.command_feedback_revision())
	await _click_cell(_gate_cell + Vector2i(0, -3), MOUSE_BUTTON_RIGHT)
	if str(_main._placement_kind) != "" \
			or int(GrusBridge.command_feedback_revision()) != placement_revision \
			or str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("right-click while placing Gate did not cancel cleanly without a command or pause")
		return

	if not await _arm_build("Wall"):
		return
	for cell in ring_cells:
		if cell == _gate_cell:
			continue
		var before := _building_ids_of_kind("Wall").size()
		await _click_cell(cell, MOUSE_BUTTON_LEFT)
		if not await _wait_until(
				func(): return _building_ids_of_kind("Wall").size() == before + 1,
				8.0, "Wall placement at %s did not reach the simulation" % [cell]):
			return
		if str(_main._placement_kind) != "Wall":
			_fail("successful Wall placement did not keep placement armed")
			return
	if _building_ids_of_kind("Wall").size() != ring_cells.size() - 1:
		_fail("wall ring census is incomplete")
		return

	# Esc must cancel the armed Wall tool before the normal pause action.
	await _key_tap(KEY_ESCAPE)
	if str(_main._placement_kind) != "" \
			or str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("Esc did not cancel Wall placement while keeping the match Playing")
		return

	var gate_id := _building_ids_of_kind("Gate")[0]
	_defense_ids.append(gate_id)
	for id in _building_ids_of_kind("Wall"):
		_defense_ids.append(id)
	if not await _wait_until(_all_defenses_complete, 40.0,
			"the Worker did not finish the placed wall ring"):
		return
	var gate_view := _building_view(gate_id)
	if gate_view == null:
		_fail("completed Gate view disappeared before the wave")
		return

	# This ground goal is outside the one-cell Gate. With the ring closed, a
	# successful route proves humans traverse the Gate while it is complete.
	var outside_gate := Vector2i(_gate_cell.x - 2, _gate_cell.y - 1)
	if not await _select_worker(builder):
		return
	if not _inside_ring(builder.global_position):
		_fail("construction Worker was outside the closed ring before the Gate traversal proof")
		return
	var move_revision := int(GrusBridge.command_feedback_revision())
	await _mouse_click(_cell_screen(outside_gate), MOUSE_BUTTON_RIGHT)
	if str(_main.command_status.text) != "Move command queued":
		_fail("controller did not issue the ordinary move through the Gate: %s" % [_main.command_status.text])
		return
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > move_revision \
				and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			8.0, "the Gate traversal move was rejected"):
		return
	if not await _wait_until(func():
			return builder.global_position.distance_to(_cell_world(outside_gate)) < 0.8,
			8.0, "the Worker did not cross the completed Gate"):
		return
	var scrap_source := _scrap_source_view()
	if scrap_source == null:
		_fail("no visible Scrap source is available for the Gun Turret cost")
		return
	var gather_revision := int(GrusBridge.command_feedback_revision())
	await _mouse_click(_main.camera.unproject_position(scrap_source.global_position), MOUSE_BUTTON_RIGHT)
	if str(_main.command_status.text) != "Gather command queued":
		_fail("controller did not queue a real Scrap gather for the Gun Turret: %s" % [_main.command_status.text])
		return
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > gather_revision \
				and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			8.0, "the Scrap gather command was rejected"):
		return

	var turret_anchor := Vector2i(rc_anchor.x - 1, _ring_max.y + 3)
	var turret_preview: Dictionary = GrusBridge.placement_preview(
		int(builder.get_meta("unit_id", -1)), "GunTurret", turret_anchor.x, turret_anchor.y)
	if not bool(turret_preview.get("valid", false)):
		_fail("derived southern turret site is not reachable: %s" % [turret_preview])
		return
	var turret_scrap := int(turret_spec.get("scrap", 0))
	if not await _wait_until(func(): return int(GrusBridge.economy_snapshot().get("scrap", 0)) >= turret_scrap,
			25.0, "workers did not gather enough Scrap for the Gun Turret"):
		return
	if not await _arm_build("GunTurret"):
		return
	var turret_before := _building_ids_of_kind("GunTurret").size()
	await _click_cell(turret_anchor, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func(): return _building_ids_of_kind("GunTurret").size() == turret_before + 1,
			8.0, "Gun Turret placement did not reach the simulation"):
		return
	var turret_id := _building_ids_of_kind("GunTurret")[0]
	if not await _wait_until(_building_complete.bind(turret_id), 20.0,
			"Gun Turret did not complete before the first wave"):
		return
	var turret := _building_view(turret_id)
	if turret == null:
		_fail("completed Gun Turret view is missing")
		return

	var initial_wave := GrusBridge.wave_snapshot()
	if int(initial_wave.get("wave", -1)) != 0 or int(initial_wave.get("next_wave", -1)) != 1:
		_fail("defense setup overran the first wave clock: %s" % [initial_wave])
		return
	var crystals_before_wave := int(GrusBridge.economy_snapshot().get("crystals", 0))
	var wave_launch := {"msec": 0}
	if not await _wait_until(func():
			var snapshot: Dictionary = GrusBridge.wave_snapshot()
			if int(snapshot.get("wave", 0)) < 1 or int(snapshot.get("live_aliens", 0)) < 12:
				return false
			wave_launch["msec"] = Time.get_ticks_msec()
			return true,
			35.0, "the real North wave did not launch twelve aliens"):
		return
	if _hidden_alien_count() == 0:
		_fail("the first wave spawned without any alien hidden by authoritative Team-1 fog")
		return
	# Fresh North entrants are beyond the 10-cell unit vision radius from the
	# Gate; the fallback must carry their AttackMove to an approach slot.
	var distant_hidden_alien := false
	for alien in _alien_views():
		if not alien.is_visible_in_tree() and alien.global_position.distance_to(gate_view.global_position) > 10.0:
			distant_hidden_alien = true
	if not distant_hidden_alien:
		_fail("the North wave did not begin with a hidden alien more than ten cells from the Gate")
		return
	var first_launch_msec := int(wave_launch["msec"])

	var gate_state := {"damaged": false}
	if not await _wait_until(func():
			if _building_view(gate_id) == null:
				if _health_at_gate_breach.is_empty():
					_capture_health_at_gate_breach()
				_observe_turret(turret, crystals_before_wave)
				return bool(gate_state["damaged"])
			for alien in _alien_views():
				if _inside_ring(alien.global_position):
					_fail("alien traversed the Gate cell before it was destroyed")
					return false
			if float(gate_view.call("health_ratio")) < 0.999:
				gate_state["damaged"] = true
			_observe_turret(turret, crystals_before_wave)
			return false,
			45.0, "aliens did not AttackMove to and destroy the Gate"):
		return
	if not bool(gate_state["damaged"]) or _building_view(gate_id) != null:
		_fail("Gate breach was not observed: damaged=%s live_view=%s" % [gate_state["damaged"], _building_view(gate_id)])
		return

	if not await _wait_until(func():
			_observe_turret(turret, crystals_before_wave)
			return _post_breach_damage_seen(),
			20.0, "aliens stopped attacking after the Gate breach instead of damaging another structure"):
		return
	if not await _wait_until(func():
			_observe_turret(turret, crystals_before_wave)
			return _turret_crystal_kill_seen,
			10.0, "the completed Gun Turret did not kill a visible alien for Crystals"):
		return
	if not _turret_tracer_seen:
		_fail("Gun Turret combat event did not render a tracer from the building view")
		return

	var elapsed := float(Time.get_ticks_msec() - first_launch_msec) / 1000.0
	if elapsed > 60.0:
		_fail("defense journey exceeded its post-wave time budget: %.1fs" % elapsed)
		return
	GrusBridge.set_sim_speed(1.0)
	print("GRUS_DEFENSE_SMOKE_OK wall_ring=%d gate_human_traversal=true alien_gate_breach=true post_breach_structure_damage=true turret_crystal_kill=true turret_tracer_origin=building_view fog=hidden_distant_fallback=true setup_s=%.1f post_wave_s=%.1f" % [ring_cells.size(), float(first_launch_msec - journey_start_msec) / 1000.0, elapsed])
	get_tree().quit(0)

func _perimeter_cells() -> Array[Vector2i]:
	var cells: Array[Vector2i] = [_gate_cell]
	for x in range(_ring_min.x, _ring_max.x + 1):
		var top := Vector2i(x, _ring_min.y)
		var bottom := Vector2i(x, _ring_max.y)
		if top != _gate_cell:
			cells.append(top)
		cells.append(bottom)
	for y in range(_ring_min.y + 1, _ring_max.y):
		cells.append(Vector2i(_ring_min.x, y))
		cells.append(Vector2i(_ring_max.x, y))
	return cells

func _building_ids_of_kind(kind: String) -> Array[int]:
	var ids: Array[int] = []
	for node in get_tree().get_nodes_in_group("building_views"):
		if str(node.get_meta("building_kind", "")) == kind:
			ids.append(int(node.get_meta("building_id", -1)))
	ids.sort()
	return ids

func _scrap_source_view() -> Node3D:
	for node in get_tree().get_nodes_in_group("resource_views"):
		if str(node.get_meta("resource_kind", "")) == "Scrap":
			return node as Node3D
	return null

func _all_defenses_complete() -> bool:
	for id in _defense_ids:
		if not _building_complete(id):
			return false
	return true

func _capture_health_at_gate_breach() -> void:
	_health_at_gate_breach.clear()
	var core := _building_view(1)
	if core != null:
		_health_at_gate_breach[1] = float(core.call("health_ratio"))
	for id in _building_ids_of_kind("Wall"):
		var view := _building_view(id)
		if view != null:
			_health_at_gate_breach[id] = float(view.call("health_ratio"))

func _post_breach_damage_seen() -> bool:
	var core := _building_view(1)
	if core == null and _health_at_gate_breach.has(1):
		return true
	if core != null and _health_at_gate_breach.has(1) \
			and float(core.call("health_ratio")) < float(_health_at_gate_breach[1]) - 0.001:
		return true
	for key in _health_at_gate_breach:
		var id := int(key)
		if id == 1:
			continue
		var view := _building_view(id)
		if view == null or float(view.call("health_ratio")) < float(_health_at_gate_breach[id]) - 0.001:
			return true
	return false
func _observe_turret(turret: Node3D, crystals_before_wave: int) -> void:
	if not _turret_tracer_seen and is_instance_valid(turret) and _turret_tracer_from(turret):
		_turret_tracer_seen = true
	if not _turret_crystal_kill_seen \
			and int(GrusBridge.economy_snapshot().get("crystals", 0)) > crystals_before_wave:
		_turret_crystal_kill_seen = true

func _select_worker(worker: Node3D) -> bool:
	await _mouse_click(_main.camera.unproject_position(worker.global_position), MOUSE_BUTTON_LEFT)
	if not _main.selected_ids.has(int(worker.get_meta("unit_id", -1))):
		_fail("real controller input did not select the Worker")
		return false
	return true

func _arm_build(kind: String) -> bool:
	var button_name := "GunTurretButton" if kind == "GunTurret" else "%sButton" % kind
	var button := _main.get_node("HUD/CommandPanel/BuildGrid/%s" % button_name) as Button
	await _mouse_click(button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if str(_main._placement_kind) != kind:
		_fail("real build-button input did not arm %s placement" % kind)
		return false
	return true

func _cell_world(cell: Vector2i) -> Vector3:
	return Vector3(cell.x + 0.5, 0.0, cell.y + 0.5)

func _cell_screen(cell: Vector2i) -> Vector2:
	return _main.camera.unproject_position(_cell_world(cell))

func _click_cell(cell: Vector2i, button: int) -> void:
	await _mouse_click(_cell_screen(cell), button)

func _inside_ring(position: Vector3) -> bool:
	return position.x >= _ring_min.x + 1 and position.x < _ring_max.x \
		and position.z >= _ring_min.y + 1 and position.z < _ring_max.y

func _turret_tracer_from(turret: Node3D) -> bool:
	for child in _fx.get_children():
		var view := child as MeshInstance3D
		if view == null or not (view.mesh is BoxMesh):
			continue
		var mesh := view.mesh as BoxMesh
		if not is_equal_approx(mesh.size.x, 0.08):
			continue
		var origin := view.global_position + view.global_basis.z * (mesh.size.z * 0.5)
		if origin.distance_to(turret.global_position + Vector3(0.0, 0.8, 0.0)) < 0.25:
			return true
	return false
