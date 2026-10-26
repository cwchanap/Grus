extends Node

## HPA-449 survival lifecycle smoke. It uses the authored warning telegraph
## to scout with a live human unit, attacks an actual first-wave alien with
## trained Guards, verifies the Crystal reward and fog-gated marker path,
## then pauses, resumes into alien-caused Defeat, checks input lock, and
## restarts to the one-base fixture.

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## RejectReason declaration order (crates/grus-sim/src/commands.rs):
## SessionLocked is the 18th variant (0-based 17); -1 means no rejection.
const REJECT_SESSION_LOCKED := 17
const ALIEN_KINDS := ["Swarmer", "Spitter", "Brute", "Behemoth"]
const SmokeHelpers := preload("res://scripts/smoke_helpers.gd")

var _main: Node3D
var _fx: Node3D
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

func _building_view(id: int) -> Node3D:
	return _find_view("building_views", "building_id", id)

func _first_kind_view(group: String, meta: String, value: String, box: Dictionary) -> bool:
	for node in get_tree().get_nodes_in_group(group):
		if str(node.get_meta(meta, "")) == value:
			box["view"] = node as Node3D
			return true
	return false

func _guard_views() -> Array[Node3D]:
	var guards: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == 1 \
				and str(node.get_meta("unit_kind", "")) == "Guard":
			guards.append(node as Node3D)
	return guards

func _alien_views() -> Array[Node3D]:
	var aliens: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == 2 \
				and str(node.get_meta("unit_kind", "")) in ALIEN_KINDS:
			aliens.append(node as Node3D)
	return aliens

func _visible_alien_count() -> int:
	var count := 0
	for alien in _alien_views():
		if alien.is_visible_in_tree():
			count += 1
	return count

func _hidden_alien_count() -> int:
	var count := 0
	for alien in _alien_views():
		if not alien.is_visible_in_tree():
			count += 1
	return count

func _visible_alien(box: Dictionary) -> bool:
	for alien in _alien_views():
		if alien.is_visible_in_tree():
			box["view"] = alien
			return true
	return false

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
	await get_tree().process_frame
	await get_tree().process_frame

func _mouse_drag(start: Vector2, finish: Vector2) -> void:
	var press := InputEventMouseButton.new()
	press.button_index = MOUSE_BUTTON_LEFT
	press.position = start
	press.pressed = true
	Input.parse_input_event(press)

	var motion := InputEventMouseMotion.new()
	motion.position = finish
	motion.relative = finish - start
	motion.button_mask = MOUSE_BUTTON_MASK_LEFT
	Input.parse_input_event(motion)

	var release := InputEventMouseButton.new()
	release.button_index = MOUSE_BUTTON_LEFT
	release.position = finish
	release.pressed = false
	Input.parse_input_event(release)
	await get_tree().process_frame
	await get_tree().process_frame

func _key_tap(keycode: int) -> void:
	var press := InputEventKey.new()
	press.keycode = keycode
	press.pressed = true
	Input.parse_input_event(press)

	var release := InputEventKey.new()
	release.keycode = keycode
	release.pressed = false
	Input.parse_input_event(release)
	await get_tree().process_frame
	await get_tree().process_frame

func _select_guards(guards: Array[Node3D], guard_ids: PackedInt32Array) -> bool:
	var top_left := Vector2(INF, INF)
	var bottom_right := Vector2(-INF, -INF)
	for guard in guards:
		var screen_position: Vector2 = _main.camera.unproject_position(guard.global_position)
		top_left.x = minf(top_left.x, screen_position.x)
		top_left.y = minf(top_left.y, screen_position.y)
		bottom_right.x = maxf(bottom_right.x, screen_position.x)
		bottom_right.y = maxf(bottom_right.y, screen_position.y)
	await _mouse_drag(top_left - Vector2(4.0, 4.0), bottom_right + Vector2(4.0, 4.0))
	var selected := {}
	for id in _main.selected_ids:
		selected[int(id)] = true
	if selected.size() != guard_ids.size():
		_fail("controller drag selected %d units, expected the three trained Guards: %s" % [selected.size(), _main.selected_ids])
		return false
	for id in guard_ids:
		if not selected.has(int(id)):
			_fail("controller drag omitted trained Guard %d: %s" % [id, _main.selected_ids])
			return false
	return true

func _fixture_views_settled() -> bool:
	var units := get_tree().get_nodes_in_group("unit_views")
	var buildings := get_tree().get_nodes_in_group("building_views")
	var resources := get_tree().get_nodes_in_group("resource_views")
	if units.size() != 4 or buildings.size() != 1 or resources.size() != 18:
		return false
	for id in range(1, 5):
		if _find_view("unit_views", "unit_id", id) == null:
			return false
	return _building_view(1) != null

func _building_complete(id: int) -> bool:
	return bool(GrusBridge.building_snapshot(id).get("complete", false))

func _session_panel() -> Panel:
	return _main.get_node("HUD/SessionPanel") as Panel

func _session_label() -> Label:
	return _main.get_node("HUD/SessionPanel/SessionLabel") as Label

func _button(path: String) -> Button:
	return _main.get_node(path) as Button

## Outside Playing neither input handler may reach the bridge: right-click
## is refused, A arms nothing, and the command feedback revision is stable.
func _assert_input_gated(where: String) -> bool:
	var revision := int(GrusBridge.command_feedback_revision())
	_main.call("_issue_context_command", Vector2(TEST_VIEWPORT_SIZE) / 2.0)
	var key := InputEventKey.new()
	key.keycode = KEY_A
	key.pressed = true
	_main.call("_handle_key", key)
	if _main.command_status.text != "Match is not playing":
		_fail("%s: context command was not refused: %s" % [where, _main.command_status.text])
		return false
	if bool(_main._attack_move_armed) \
			or int(GrusBridge.command_feedback_revision()) != revision:
		_fail("%s: an input handler reached the bridge while not Playing" % where)
		return false
	return true

func _restart_settled() -> bool:
	if not _fixture_views_settled():
		return false
	var ids := {}
	for node in get_tree().get_nodes_in_group("unit_views"):
		ids[int(node.get_meta("unit_id", -1))] = true
	return ids.size() == 4 and ids.has(1) and ids.has(2) and ids.has(3) and ids.has(4)

func _snapshot_has_safe_surface(snapshot: Dictionary) -> bool:
	if snapshot.size() != 5:
		return false
	for key in ["wave", "next_wave", "seconds_remaining", "incoming_zones", "live_aliens"]:
		if not snapshot.has(key):
			return false
	var zones: Array = snapshot.get("incoming_zones", [])
	for zone in zones:
		if zone.size() != 3 or not zone.has("name") \
				or not zone.has("anchor_x") or not zone.has("anchor_y"):
			return false
	return true

func _success_marker() -> String:
	return "GRUS_COMBAT_LIFECYCLE_SMOKE_OK stage=restart-fresh fixture=4/1/18 warning=North live_aliens=true fog=hidden_then_revealed markers=visibility_gated controller_attack_move=accepted controller_visible_alien_right_click=accepted crystal_reward=1 pause_frozen=true result=alien_defeat result_reject=session_locked restart_wave_reset=true"

func _run() -> void:
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame
	_main = get_node("Main") as Node3D
	_fx = _main.get_node("CombatFx") as Node3D
	_minimap = _main.get_node("HUD/Minimap") as Control

	if not GrusBridge.set_sim_speed(20.0):
		_fail("failed to accelerate the sim")
		return
	if not await _wait_until(_fixture_views_settled, 20.0,
			"one-base fixture never settled to 4 units / 1 building / 18 resources"):
		return

	var initial_wave: Dictionary = GrusBridge.wave_snapshot()
	if not _snapshot_has_safe_surface(initial_wave) \
			or int(initial_wave.get("wave", -1)) != 0 \
			or int(initial_wave.get("next_wave", -1)) != 1 \
			or not initial_wave.get("incoming_zones", []).is_empty() \
			or int(initial_wave.get("live_aliens", -1)) != 0:
		_fail("fresh Start phase does not expose only the pre-wave snapshot: %s" % [initial_wave])
		return

	var session: Dictionary = GrusBridge.session_snapshot()
	if str(session.get("phase", "")) != "Start":
		_fail("fixture boot session is not Start: %s" % [session])
		return
	if not (_session_panel().visible and _button("HUD/SessionPanel/StartButton").visible \
				and not _button("HUD/SessionPanel/QuitButton").visible):
		_fail("Start overlay is not up during the Start phase")
		return
	if not GrusBridge.move_units(PackedInt32Array([1]), Vector2(30, 44)):
		_fail("bridge refused to queue the Start-phase probe command")
		return
	if not await _wait_until(
			func(): return int(GrusBridge.last_reject_code()) == REJECT_SESSION_LOCKED,
			5.0, "Start-phase gameplay order was not rejected with SessionLocked"):
		return
	if not _assert_input_gated("Start"):
		return

	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return
	if not await _wait_until(
			func(): return not _session_panel().visible and _button("HUD/PauseButton").visible,
			5.0, "Playing phase did not hide the overlay and show Pause"):
		return

	# Build and train through the ordinary fixture/production bridge seams,
	# within the 300-second preparation period. The three Guards have enough
	# combined counter damage to kill a Swarmer and prove Crystal credit.
	var research_center := _building_view(1)
	var barracks_anchor := SmokeHelpers.barracks_anchor(4, research_center)
	if barracks_anchor.x < 0:
		_fail("no valid Barracks anchor in the human Research Center ring")
		return
	if not GrusBridge.place_building(4, "Barracks", barracks_anchor.x, barracks_anchor.y):
		_fail("Barracks placement command was rejected")
		return
	var barracks_box := {"view": null}
	if not await _wait_until(
			func(): return _first_kind_view("building_views", "building_kind", "Barracks", barracks_box),
			20.0, "Barracks view never appeared"):
		return
	var barracks: Node3D = barracks_box["view"]
	var barracks_id := int(barracks.get_meta("building_id", -1))
	if not await _wait_until(_building_complete.bind(barracks_id), 20.0,
			"Barracks never completed during preparation"):
		return
	for _guard in 3:
		if not GrusBridge.enqueue_unit(barracks_id, "Guard"):
			_fail("Barracks refused a Guard training order")
			return
	if not await _wait_until(func(): return _guard_views().size() == 3, 20.0,
			"three trained Guards never appeared"):
		return
	var guards := _guard_views()
	var guard_ids := PackedInt32Array()
	for guard in guards:
		guard_ids.append(int(guard.get_meta("unit_id", -1)))
	if guard_ids.size() != 3 or int(GrusBridge.economy_snapshot().get("population_used", -1)) != 7:
		_fail("trained defender census is wrong: guards=%d economy=%s" % [guard_ids.size(), GrusBridge.economy_snapshot()])
		return

	# Wait for the real 30-second warning, then use its fixture-authored North
	# anchor to position a scout and the defenders. No entry coordinate is
	# copied into this script and no hidden alien location is read.
	var warning_box := {"snapshot": {}}
	if not await _wait_until(func():
			var snapshot: Dictionary = GrusBridge.wave_snapshot()
			if not snapshot.get("incoming_zones", []).is_empty():
				warning_box["snapshot"] = snapshot
				return true
			return false,
			30.0, "first wave warning never appeared"):
		return
	var warning: Dictionary = warning_box["snapshot"]
	if not _snapshot_has_safe_surface(warning) \
			or int(warning.get("wave", -1)) != 0 \
			or int(warning.get("next_wave", -1)) != 1 \
			or float(warning.get("seconds_remaining", 0.0)) > 30.0:
		_fail("first-wave warning metadata is wrong: %s" % [warning])
		return
	var zones: Array = warning.get("incoming_zones", [])
	if zones.size() != 1 or str(zones[0].get("name", "")) != "North":
		_fail("wave 1 warning did not announce the authored North zone: %s" % [zones])
		return
	var anchor := Vector2(float(zones[0].get("anchor_x", -1)), float(zones[0].get("anchor_y", -1)))
	if anchor.x < 0 or anchor.y < 0:
		_fail("North warning omitted its fixture-authored anchor: %s" % [zones[0]])
		return
	if not GrusBridge.set_sim_speed(5.0):
		_fail("failed to set a readable speed for the warning/combat window")
		return
	var scout := _find_view("unit_views", "unit_id", 4)
	var scout_target := Vector2(anchor.x, anchor.y + 14.0)
	var scout_revision := int(GrusBridge.command_feedback_revision())
	if not GrusBridge.move_units(PackedInt32Array([4]), scout_target):
		_fail("scout move toward the announced entry was not queued")
		return
	if not await _wait_until(
			func(): return int(GrusBridge.command_feedback_revision()) > scout_revision \
					and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			10.0, "scout move was not accepted by the simulation"):
		return
	if not await _wait_until(
			func(): return scout.global_position.distance_to(Vector3(scout_target.x, 0.0, scout_target.y)) < 2.0,
			15.0, "scout never reached its warning-derived position"):
		return
	var defender_target := Vector2(anchor.x, anchor.y + 22.0)
	var defenders_revision := int(GrusBridge.command_feedback_revision())
	if not await _select_guards(guards, guard_ids):
		return
	await _key_tap(KEY_A)
	if not bool(_main._attack_move_armed):
		_fail("controller A-key did not arm Guard attack-move")
		return
	var defender_screen: Vector2 = _main.camera.unproject_position(Vector3(defender_target.x, 0.0, defender_target.y))
	await _mouse_click(defender_screen, MOUSE_BUTTON_RIGHT)
	if str(_main.command_status.text) != "Attack-move queued":
		_fail("controller ground right-click did not queue the announced-entry attack-move: %s" % [_main.command_status.text])
		return
	if not await _wait_until(
			func(): return int(GrusBridge.command_feedback_revision()) > defenders_revision \
					and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			10.0, "Guard attack-move was not accepted by the simulation"):
		return

	if not await _wait_until(func():
			var snapshot: Dictionary = GrusBridge.wave_snapshot()
			return int(snapshot.get("wave", 0)) >= 1 and int(snapshot.get("live_aliens", 0)) >= 12,
			15.0, "first wave did not launch twelve live aliens"):
		return
	if not await _wait_until(func(): return _alien_views().size() >= 12, 5.0,
			"live wave aliens never reached the Godot view tree"):
		return
	# The allied scout should not reveal every entrant: live alien views remain
	# in the scene tree under fog while only current-vision aliens become
	# minimap markers.
	if _hidden_alien_count() == 0:
		_fail("wave spawn exposed every live alien despite fog of war")
		return
	await get_tree().process_frame
	if int(_minimap.enemy_marker_count) != _visible_alien_count():
		_fail("minimap alien markers are not gated by the same live visibility: markers=%d visible=%d" % [int(_minimap.enemy_marker_count), _visible_alien_count()])
		return
	var visible_before := _visible_alien_count()
	if not await _wait_until(func(): return _visible_alien_count() > visible_before, 10.0,
			"the scout never revealed an approaching live alien"):
		return
	await get_tree().process_frame
	if int(_minimap.enemy_marker_count) != _visible_alien_count():
		_fail("minimap marker count did not track the revealed live alien set")
		return

	# An ordinary direct-attack command lets the trained defender kill an
	# actual Swarmer; its real catalogue reward must reach the shared stockpile.
	var target_box := {"view": null}
	if not _visible_alien(target_box):
		_fail("no visible live alien target after the scout reveal")
		return
	var target: Node3D = target_box["view"]
	var target_id := int(target.get_meta("unit_id", -1))
	var crystals_before := int(GrusBridge.economy_snapshot().get("crystals", -1))
	var attack_revision := int(GrusBridge.command_feedback_revision())
	await _mouse_click(_main.camera.unproject_position(target.global_position), MOUSE_BUTTON_RIGHT)
	if str(_main.command_status.text) != "Attack command queued":
		_fail("controller right-click did not queue a direct attack on the visible alien: %s" % [_main.command_status.text])
		return
	if not await _wait_until(
			func(): return int(GrusBridge.command_feedback_revision()) > attack_revision \
					and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			10.0, "simulation rejected the live-alien attack"):
		return
	if not await _wait_until(
			func(): return int(GrusBridge.economy_snapshot().get("crystals", -1)) > crystals_before,
			15.0, "Guard combat never killed an alien or credited Crystals"):
		return
	if int(GrusBridge.economy_snapshot().get("crystals", -1)) != crystals_before + 1:
		_fail("one Swarmer kill did not credit exactly one Crystal: %s" % [GrusBridge.economy_snapshot()])
		return

	# Pause freezes health and combat-event totals. Resume into a real alien
	# Defeat; the remaining wave still attacks the sole Research Center.
	if not GrusBridge.set_paused(true):
		_fail("set_paused(true) was refused after the alien kill")
		return
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Paused", 5.0,
			"session never reached Paused"):
		return
	if not await _wait_until(
			func(): return _session_panel().visible \
					and _button("HUD/SessionPanel/ResumeButton").visible \
					and not _button("HUD/PauseButton").visible \
					and not _button("HUD/SessionPanel/QuitButton").visible \
					and _session_label().text == "Paused",
			5.0, "Paused overlay is not showing Resume with Pause/Quit hidden"):
		return
	var wave_before_pause: Dictionary = GrusBridge.wave_snapshot()
	var health_before_pause := float(_building_view(1).call("health_ratio"))
	var events_before_pause := int(GrusBridge.combat_event_total())
	for _frame in 12:
		await get_tree().physics_frame
	var wave_during_pause: Dictionary = GrusBridge.wave_snapshot()
	if float(_building_view(1).call("health_ratio")) != health_before_pause \
			or not is_equal_approx(float(wave_during_pause.get("seconds_remaining", -1.0)), float(wave_before_pause.get("seconds_remaining", -2.0))) \
			or int(GrusBridge.combat_event_total()) != events_before_pause:
		_fail("paused session advanced its wave clock, combat events, or Research Center health")
		return
	if not _assert_input_gated("Paused"):
		return
	if not GrusBridge.set_paused(false):
		_fail("set_paused(false) was refused")
		return
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Playing", 5.0,
			"session never resumed to Playing"):
		return
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Result", 45.0,
			"surviving aliens never destroyed the sole Research Center"):
		return
	var result: Dictionary = GrusBridge.session_snapshot()
	if int(result.get("winner_team", -1)) != 2:
		_fail("alien-caused Result is not Team 2 Defeat: %s" % [result])
		return
	if not await _wait_until(
			func(): return _session_panel().visible and _session_label().text == "Defeat" \
					and _button("HUD/SessionPanel/RestartButton").visible \
					and _button("HUD/SessionPanel/QuitButton").visible,
			5.0, "Result overlay is not showing Defeat with Restart/Quit"):
		return
	if not GrusBridge.move_units(PackedInt32Array([1]), Vector2(30, 44)):
		_fail("bridge refused to queue the Result-phase probe command")
		return
	if not await _wait_until(
			func(): return int(GrusBridge.last_reject_code()) == REJECT_SESSION_LOCKED,
			5.0, "Result-phase gameplay order was not rejected with SessionLocked"):
		return
	if not _assert_input_gated("Result"):
		return

	# Restart removes every old combat view/selection and recreates the
	# pristine Start-state wave clock along with the one-base fixture.
	var old_ids: Array[int] = []
	for guard_id in guard_ids:
		old_ids.append(int(guard_id))
	_main.selected_ids = old_ids
	_main.selected_building_id = barracks_id
	_main._control_groups[1] = old_ids.duplicate()
	_main._attack_move_armed = true
	_fx.call("spawn_death", Vector3.ZERO)
	if int(_fx.call("effect_count")) < 1:
		_fail("transient effect seed for the restart check did not land")
		return
	_main.call("_restart_match")
	if not await _wait_until(_restart_settled, 20.0,
			"restart never re-settled to the exact fresh 4-unit fixture"):
		return
	if get_tree().get_nodes_in_group("building_views").size() != 1 \
			or get_tree().get_nodes_in_group("resource_views").size() != 18:
		_fail("restart left a stale building/resource view")
		return
	for unit in get_tree().get_nodes_in_group("unit_views"):
		var id := int(unit.get_meta("unit_id", -1))
		if id < 1 or id > 4 or absf(float((unit as Node3D).call("health_ratio")) - 1.0) > 0.0001:
			_fail("restart left a stale or damaged unit view id %d" % id)
			return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Start":
		_fail("restart did not return to Start")
		return
	var reset_wave: Dictionary = GrusBridge.wave_snapshot()
	if not _snapshot_has_safe_surface(reset_wave) \
			or int(reset_wave.get("wave", -1)) != 0 \
			or int(reset_wave.get("next_wave", -1)) != 1 \
			or int(reset_wave.get("live_aliens", -1)) != 0 \
			or not reset_wave.get("incoming_zones", []).is_empty():
		_fail("restart did not reset WaveState with the base fixture: %s" % [reset_wave])
		return
	if not (_session_panel().visible and _button("HUD/SessionPanel/StartButton").visible \
				and not _button("HUD/SessionPanel/QuitButton").visible):
		_fail("restart did not bring the Start overlay back with Quit hidden")
		return
	if not _main.selected_ids.is_empty() or not _main._control_groups.is_empty() \
			or int(_main.selected_building_id) != -1 or bool(_main._attack_move_armed):
		_fail("restart left old selection/control-group state behind")
		return
	if int(_fx.call("effect_count")) != 0:
		_fail("restart left transient combat effects on stage")
		return

	var marker := _success_marker()
	if marker.is_empty():
		GrusBridge.set_sim_speed(1.0)
		get_tree().quit(1)
		return
	print(marker)
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(0)
