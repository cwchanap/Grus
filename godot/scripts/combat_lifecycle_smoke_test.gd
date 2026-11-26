extends Node

## HPA-449 survival lifecycle smoke. It uses the authored warning telegraph
## to scout with a live human unit, attacks an actual first-wave alien with
## trained Guards, verifies the Crystal reward and fog-gated marker path,
## then pauses, resumes into alien-caused Defeat, checks input lock, and
## restarts to the one-base fixture.

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## RejectReason declaration order (crates/grus-sim/src/commands.rs):
## InsufficientResources is the 9th variant (0-based 8) and SessionLocked
## the 18th (0-based 17); -1 means no rejection.
const REJECT_INSUFFICIENT_RESOURCES := 8
const REJECT_SESSION_LOCKED := 17
const ALIEN_KINDS := ["Swarmer", "Spitter", "Brute", "Behemoth"]
const HudHelpers := preload("res://scripts/hud_helpers.gd")
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

## Drains matching Guard→target combat events on every process_frame —
## which fires before the controller's per-frame cosmetics drain inside
## _process — starting before the attack click and running through its
## acceptance wait, so a same-tick kill cannot be consumed unseen. The
## drain is destructive, so every consumed event is forwarded to the
## controller's cosmetics dispatch — the strikes still flash and trace in
## a windowed run exactly as if _refresh_session had read them. Runs on
## a wall-clock deadline rather than a frame budget so slow renderers stay
## bounded at any fps: box["seen"] records the match, box["done"] the exit.
func _observe_guard_strike(guard_ids: PackedInt32Array, target_id: int, box: Dictionary) -> void:
	var deadline := Time.get_ticks_msec() + 15000
	# The first drain still runs before the right-click is dispatched:
	# strikes the earlier attack-move order already queued are forwarded
	# for cosmetics but must not count as this click's verdict.
	var collecting := false
	while Time.get_ticks_msec() < deadline and not bool(box["seen"]):
		for event in GrusBridge.drain_combat_events():
			_main.call("_dispatch_combat_event", event)
			if collecting \
					and int(event.get("attacker", -1)) in guard_ids \
					and str(event.get("target_kind", "")) == "unit" \
					and int(event.get("target_id", -1)) == target_id:
				box["seen"] = true
		collecting = true
		await get_tree().process_frame
	box["done"] = true

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

func _result_bar() -> Panel:
	return _main.get_node_or_null("HUD/ResultBar") as Panel

func _ug_drawer() -> Panel:
	return _main.get_node("HUD/UndergroundPanel") as Panel

func _session_label() -> Label:
	return _main.get_node("HUD/SessionPanel/SessionLabel") as Label

## The compact Pause card must follow the live root viewport, not only the
## viewport it was entered with: centered in the named size, fully on screen,
## and containing a visible, enabled Resume. Returns "" when it holds.
## The viewport the expand stretch aspect produces for a window size: the base
## layout grows along the one axis the window is longer than 16:9 on.
func _stretched_viewport(window_size: Vector2i) -> Vector2i:
	var base := Vector2(TEST_VIEWPORT_SIZE)
	var scale := minf(float(window_size.x) / base.x, float(window_size.y) / base.y)
	return Vector2i((Vector2(window_size) / scale).round())

func _paused_resize_error(target: Vector2i) -> String:
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Paused":
		return "resize probe left Paused"
	var view := Rect2(Vector2.ZERO, target)
	var card := _session_panel().get_global_rect()
	if not view.encloses(card):
		return "Paused card leaves the %s viewport: card=%s" % [target, card]
	if absf(card.get_center().x - view.get_center().x) > 1.0 \
			or absf(card.get_center().y - view.get_center().y) > 1.0:
		return "Paused card is not centered in the %s viewport: card=%s viewport=%s" % [
			target, card, view]
	var resume := _button("HUD/SessionPanel/ResumeButton") as Button
	if resume == null or not resume.visible or resume.disabled:
		return "Paused Resume is not visible and enabled in the %s viewport" % target
	if not card.encloses(resume.get_global_rect()) \
			or not view.encloses(resume.get_global_rect()):
		return "Paused Resume leaves its card or the %s viewport: resume=%s card=%s" % [
			target, resume.get_global_rect(), card]
	return ""

func _button(path: String) -> Button:
	return _main.get_node(path) as Button

## Prominent primary actions: Paused Resume draws the same bright plate and
## 22 px label as Start/Restart, and the one shared treatment must keep every
## action's ink readable in each interactive state with a spatially distinct
## high-contrast focus ring rather than a recoloured edge. Nodes, wording,
## connections, geometry and Esc paths stay untouched. Called at Paused and
## again at the genuine alien Defeat screen, where Restart is the live action.
func _primary_action_error() -> String:
	var start := _button("HUD/SessionPanel/StartButton")
	var resume := _button("HUD/SessionPanel/ResumeButton")
	var restart := _button("HUD/SessionPanel/RestartButton")
	var primary := start.get_theme_stylebox("normal") as StyleBoxFlat
	var resume_style := resume.get_theme_stylebox("normal") as StyleBoxFlat
	var restart_style := restart.get_theme_stylebox("normal") as StyleBoxFlat
	if primary == null or resume_style == null or restart_style == null:
		return "primary action plate is missing"
	if resume_style.bg_color != primary.bg_color \
			or resume_style.border_color != primary.border_color \
			or restart_style.bg_color != primary.bg_color:
		return "Resume does not share the bright primary plate: resume=%s/%s primary=%s/%s" % [
			resume_style.bg_color, resume_style.border_color,
			primary.bg_color, primary.border_color]
	if resume_style.bg_color.get_luminance() < 0.3:
		return "Resume primary plate is not bright: %s" % resume_style.bg_color
	if resume.get_theme_font_size("font_size") < 22:
		return "Resume label is below the 22 px primary-action floor: %d" % [
			resume.get_theme_font_size("font_size")]
	# Resume is visible only while enabled, so disabled ink is not applicable;
	# every reachable interactive state of all three actions must stay above
	# ordinary-text contrast.
	return SmokeHelpers.primary_ink_focus_error([start, resume, restart], "primary action")

## Test-only focus set-up: the global Tab handler owns focus navigation, so
## these legs focus a primary action directly and then activate it with a real
## key or click. Returns "" only when the viewport focus is actually on it.
func _focus_error(button: Button, what: String) -> String:
	if not button.has_focus() or get_viewport().gui_get_focus_owner() != button:
		return "%s did not receive keyboard focus" % what
	return ""

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
	if str(_main._armed_order) != "" \
			or int(GrusBridge.command_feedback_revision()) != revision:
		_fail("%s: an input handler reached the bridge while not Playing" % where)
		return false
	var analyze := _main.get_node("HUD/ResearchPanel/AnalyzeButton") as Button
	if analyze == null or not analyze.disabled:
		_fail("%s: Analyze is not disabled outside Playing" % where)
		return false
	return true

## HPA-188 fresh-match research boundary, exercised in the zero-Crystal
## window right after Start -> Playing and under the smoke's 20x clock.
## Real viewport input selects the live Research Center and presses its
## enabled Analyze button; the only honest verdict is the typed
## InsufficientResources reject (code 8) with the stockpile and banked
## points untouched. The catalogue-bound tower locks, HUD bar and live ETA
## are checked here; the winning 10-Crystal conversion path is a pure-Rust
## contract, and no test-only resource injection exists.
func _research_boundary_leg() -> bool:
	var camera: Camera3D = _main.camera
	var analyze_button := _main.get_node("HUD/ResearchPanel/AnalyzeButton") as Button
	var research_bar := _main.get_node("HUD/ResearchPanel/ResearchBar") as ProgressBar
	var research_label := _main.get_node("HUD/ResearchPanel/ResearchLabel") as Label
	if camera == null or analyze_button == null or research_bar == null or research_label == null:
		_fail("research HUD nodes are missing under HUD/ResearchPanel")
		return false

	# The static catalogue surface drives everything below: the Analyze
	# price/gain and each advanced tower's point threshold come from the
	# Rust catalogue, never a GDScript constant.
	var research_catalogue: Dictionary = GrusBridge.catalogue_snapshot().get("research", {})
	var target_points := int(research_catalogue.get("target_points", 0))
	var analyze_price := int(research_catalogue.get("analyze_cost", {}).get("crystals", 0))
	var analyze_gain := float(research_catalogue.get("analyze_gain_points", 0.0))
	if target_points <= 0 or analyze_price <= 0 or analyze_gain <= 0.0:
		_fail("catalogue research block is incomplete: %s" % [research_catalogue])
		return false
	var buildings_catalogue: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {})
	for tower in ["FlameTower", "CrystalLance"]:
		var entry: Dictionary = buildings_catalogue.get(tower, {})
		var unlock := int(entry.get("unlock_points", -1))
		var tower_button := _button("HUD/CommandPanel/BuildGrid/%sButton" % tower)
		if tower_button == null or unlock <= 0 or unlock >= target_points:
			_fail("%s button/catalogue unlock metadata is malformed: %s" % [tower, entry])
			return false
		# Fresh points sit below every threshold: the button renders locked
		# with its catalogue-derived meta and unlock hint text.
		if not tower_button.disabled \
				or int(tower_button.get_meta("unlock_points", -1)) != unlock \
				or not tower_button.tooltip_text.contains("Unlocks at %d research" % unlock):
			_fail("%s is not presented locked at its catalogue threshold: disabled=%s meta=%s tooltip=%s" % [
				tower, tower_button.disabled, tower_button.get_meta("unlock_points", "?"), tower_button.tooltip_text])
			return false

	# The fixture's own stockpile is exactly zero Crystals — below every
	# catalogue Analyze price — so this leg's press is deterministically
	# unaffordable. No branch on live stockpile is needed or taken.
	if int(GrusBridge.economy_snapshot().get("crystals", -1)) != 0:
		_fail("fresh Playing stockpile is not exactly zero Crystals: %s" % [GrusBridge.economy_snapshot()])
		return false

	# Real selection only: march every live Team-1 unit west of the live
	# Research Center view so no unit can shadow the building pick inside
	# its 20px radius. The destination derives from the live view — never
	# a copied fixture coordinate.
	var research_center := _building_view(1)
	if research_center == null:
		_fail("live Research Center view is missing before the Analyze leg")
		return false
	var clear_ids := PackedInt32Array()
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == 1:
			clear_ids.append(int(node.get_meta("unit_id", -1)))
	if clear_ids.is_empty():
		_fail("no live Team-1 units to clear from the Research Center pick radius")
		return false
	var clear_target := Vector2(
		research_center.global_position.x - 16.0,
		research_center.global_position.z)
	var clear_revision := int(GrusBridge.command_feedback_revision())
	if not GrusBridge.move_units(clear_ids, clear_target):
		_fail("worker clear-out move was not queued")
		return false
	if not await _wait_until(
			func(): return int(GrusBridge.command_feedback_revision()) > clear_revision \
					and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			10.0, "worker clear-out move was not accepted by the sim"):
		return false
	var rc_screen: Vector2 = camera.unproject_position(research_center.global_position)
	if not Rect2(Vector2.ZERO, Vector2(TEST_VIEWPORT_SIZE)).has_point(rc_screen):
		_fail("Research Center projects outside the viewport: %s" % rc_screen)
		return false
	if not await _wait_until(func():
			for node in get_tree().get_nodes_in_group("unit_views"):
				var view := node as Node3D
				if view == null or int(view.get_meta("team_id", -1)) != 1:
					continue
				if camera.unproject_position(view.global_position).distance_to(rc_screen) <= 40.0:
					return false
			return true,
			15.0, "workers never cleared the Research Center pick radius"):
		return false

	# Real input selects the actual building view: a left click on its
	# projected position must select building 1 and clear unit selection.
	await _mouse_click(rc_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(_main.selected_building_id) == 1 \
					and _main.selected_ids.is_empty(),
			5.0, "left-click on the live Research Center did not select building 1"):
		return false
	# Enabled follows the session+selection contract — Playing with the
	# owned, completed Research Center selected — never affordability.
	if not await _wait_until(func(): return not analyze_button.disabled,
			5.0, "Analyze stayed disabled with the owned Research Center selected while Playing"):
		return false

	var points_before := float(GrusBridge.research_snapshot().get("points", -1.0))
	var feedback_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), analyze_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	# Dispatch evidence is the sim's own verdict: the pressed button queues
	# the Analyze command, whose reject bumps the shared feedback revision
	# with the typed reason — no Crystal moves and no points bank. The HUD
	# text is part of that condition, not a follow-up read: the reject lands
	# in a godot-bevy fixed tick inside _physics_process, while command_
	# status mirrors it in _process — under fps throttling or loaded frames
	# several physics ticks can elapse before one render pass, so waiting on
	# the revision alone could observe the stale "Analyze queued" transient.
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > feedback_revision \
					and str(_main.command_status.text) == "Command rejected (InsufficientResources)",
			10.0, "the enabled Analyze press produced neither the sim verdict nor its HUD reject text"):
		return false
	if int(GrusBridge.last_reject_code()) != REJECT_INSUFFICIENT_RESOURCES \
			or str(GrusBridge.command_feedback()) != "Command rejected (InsufficientResources)":
		_fail("zero-Crystal Analyze did not surface the typed InsufficientResources reject: code=%d feedback=%s" % [
			int(GrusBridge.last_reject_code()), GrusBridge.command_feedback()])
		return false
	if str(_main.command_status.text) != "Command rejected (InsufficientResources)":
		_fail("HUD did not surface the Analyze reject text: %s" % [_main.command_status.text])
		return false
	if int(GrusBridge.economy_snapshot().get("crystals", -1)) != 0:
		_fail("rejected Analyze changed the zero-Crystal stockpile: %s" % [GrusBridge.economy_snapshot()])
		return false
	var points_after := float(GrusBridge.research_snapshot().get("points", -1.0))
	if points_after - points_before >= analyze_gain:
		_fail("rejected Analyze banked the +%d reward: %f -> %f" % [analyze_gain, points_before, points_after])
		return false

	# The live surface advances under the 20x clock: banked points and the
	# HUD bar rise while the ETA estimate shrinks. The displayed countdown
	# is captured too — a frozen or wrong label could hide behind a healthy
	# bridge value when only a "contains ETA" shape is checked.
	var snapshot_before: Dictionary = GrusBridge.research_snapshot()
	var points_wait := float(snapshot_before.get("points", -1.0))
	var eta_wait := float(snapshot_before.get("eta_seconds", -1.0))
	var bar_wait := research_bar.value
	var label_wait := research_label.text
	var label_eta_wait := SmokeHelpers.research_label_eta_seconds(label_wait)
	if points_wait < 0.0 or eta_wait < 0.0 or label_eta_wait < 0:
		_fail("Playing research surface is not numeric/positive: %s (label=%s)" % [snapshot_before, label_wait])
		return false
	for _frame in 40:
		await get_tree().physics_frame
	var snapshot_after: Dictionary = GrusBridge.research_snapshot()
	var eta_after := float(snapshot_after.get("eta_seconds", -1.0))
	if float(snapshot_after.get("points", -1.0)) <= points_wait \
			or eta_after >= eta_wait \
			or research_bar.value <= bar_wait:
		_fail("research surface froze under 20x: %s -> %s (bar %f -> %f)" % [
			snapshot_before, snapshot_after, bar_wait, research_bar.value])
		return false
	# The rendered ETA is the same live estimate: over ~13 simulated seconds
	# it must visibly tick down, and each capture must agree with its own
	# snapshot's ceil-ed seconds within a rendered frame of skew (the label
	# mirrors the previous _process frame's read) — ±5 s covers that plus
	# the HUD's ceil() rounding while still catching a frozen label.
	var label_after := research_label.text
	var label_eta_after := SmokeHelpers.research_label_eta_seconds(label_after)
	if label_eta_after < 0 or label_eta_after >= label_eta_wait \
			or absi(label_eta_wait - int(ceilf(eta_wait))) > 5 \
			or absi(label_eta_after - int(ceilf(eta_after))) > 5:
		_fail("research label does not track the live ETA: '%s' -> '%s' (snapshot %.2f -> %.2f)" % [
			label_wait, label_after, eta_wait, eta_after])
		return false
	if not is_equal_approx(research_bar.max_value, float(target_points)):
		_fail("research bar max is not the catalogue target: %f vs %d" % [
			research_bar.max_value, target_points])
		return false

	# Leave the building deselected for the production legs: a real click on
	# open ground right of the new bottom selection/command cards clears the
	# pick. Analyze intentionally stays enabled while Playing with a living
	# Research Center, and the same real button click now resolves that live
	# core and queues Analyze without a selected building — the authoritative
	# zero-Crystal reject remains the only honest verdict.
	var open_ground := Vector2(TEST_VIEWPORT_SIZE.x * 0.7, TEST_VIEWPORT_SIZE.y * 0.42)
	await _mouse_click(open_ground, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(_main.selected_building_id) == -1 \
					and _main.selected_ids.is_empty() and not analyze_button.disabled,
			5.0, "open-ground click did not deselect the Research Center while Analyze stayed enabled"):
		return false
	if int(GrusBridge.economy_snapshot().get("crystals", -1)) != 0:
		_fail("selection-free Analyze leg is not the zero-Crystal window: %s" % [GrusBridge.economy_snapshot()])
		return false
	var crystals_before_deselected := int(GrusBridge.economy_snapshot().get("crystals", -1))
	var points_before_deselected := float(GrusBridge.research_snapshot().get("points", -1.0))
	var feedback_before_deselected := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), analyze_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > feedback_before_deselected \
					and str(_main.command_status.text) == "Command rejected (InsufficientResources)",
			10.0, "selection-free Analyze press produced neither the sim verdict nor its reject text"):
		return false
	if int(GrusBridge.last_reject_code()) != REJECT_INSUFFICIENT_RESOURCES \
			or str(GrusBridge.command_feedback()) != "Command rejected (InsufficientResources)":
		_fail("selection-free zero-Crystal Analyze did not surface the typed InsufficientResources reject: code=%d feedback=%s" % [
			int(GrusBridge.last_reject_code()), GrusBridge.command_feedback()])
		return false
	if int(GrusBridge.economy_snapshot().get("crystals", -1)) != crystals_before_deselected:
		_fail("selection-free rejected Analyze changed the zero-Crystal stockpile")
		return false
	if float(GrusBridge.research_snapshot().get("points", -1.0)) - points_before_deselected >= analyze_gain:
		_fail("selection-free rejected Analyze banked the +%d reward" % int(analyze_gain))
		return false
	return true

func _blip_cue_count() -> int:
	var counts = _fx.get("cue_counts")
	if not counts is Dictionary:
		return -1
	return int((counts as Dictionary).get("generic_blip", -1))

func _cue_counts_empty() -> bool:
	var counts = _fx.get("cue_counts")
	return counts is Dictionary and (counts as Dictionary).is_empty()

func _cue_telemetry_leg() -> bool:
	var players = _fx.get("_players")
	for cue_id in ["gun_turret_fire", "swarmer_death", "winter_wind"]:
		var pilot_player: AudioStreamPlayer = (players as Dictionary).get(cue_id) as AudioStreamPlayer
		if pilot_player == null or not pilot_player.stream is AudioStreamOggVorbis:
			_fail("pilot cue %s has no OGG stream" % cue_id)
			return false
		if cue_id == "winter_wind" and not (pilot_player.stream as AudioStreamOggVorbis).loop:
			_fail("pilot wind stream does not loop")
			return false
	var player: AudioStreamPlayer = null
	if players is Dictionary:
		player = (players as Dictionary).get("generic_blip") as AudioStreamPlayer
	if player == null:
		_fail("blip cue player is missing from the cue table: %s" % [players])
		return false
	if not _cue_counts_empty():
		_fail("cue telemetry did not start empty: %s" % [_fx.get("cue_counts")])
		return false
	_fx.call("play_hit_cue")
	if _blip_cue_count() != 1:
		_fail("accepted blip cue was not counted: %s" % [_fx.get("cue_counts")])
		return false
	_fx.call("play_hit_cue")
	if _blip_cue_count() != 1:
		_fail("immediate repeat inside the 150 ms throttle was counted: %s" % [_fx.get("cue_counts")])
		return false
	_fx.call("clear_all")
	if not _cue_counts_empty():
		_fail("clear_all left cue telemetry: %s" % [_fx.get("cue_counts")])
		return false
	_fx.call("play_hit_cue")
	if _blip_cue_count() != 1:
		_fail("post-clear request inside the old throttle window was not counted: %s" % [_fx.get("cue_counts")])
		return false
	_fx.call("play_hit_cue")
	if _blip_cue_count() != 1:
		_fail("post-clear repeat inside the fresh throttle window was counted: %s" % [_fx.get("cue_counts")])
		return false
	OS.delay_msec(200)
	_fx.call("play_hit_cue")
	if _blip_cue_count() != 2:
		_fail("blip cue did not count again after the 150 ms throttle expired: %s" % [_fx.get("cue_counts")])
		return false
	_fx.call("clear_all")
	return true

func _restart_settled() -> bool:
	if not _fixture_views_settled():
		return false
	var ids := {}
	for node in get_tree().get_nodes_in_group("unit_views"):
		ids[int(node.get_meta("unit_id", -1))] = true
	return ids.size() == 4 and ids.has(1) and ids.has(2) and ids.has(3) and ids.has(4)

func _snapshot_has_safe_surface(snapshot: Dictionary) -> bool:
	if snapshot.size() != 7:
		return false
	for key in ["wave", "next_wave", "seconds_remaining", "elapsed_seconds", "incoming_zones", "live_aliens", "timer_total_seconds"]:
		if not snapshot.has(key):
			return false
	var total: Variant = snapshot.get("timer_total_seconds")
	if typeof(total) != TYPE_FLOAT and typeof(total) != TYPE_INT:
		return false
	if float(total) <= 0.0:
		return false
	var zones: Array = snapshot.get("incoming_zones", [])
	for zone in zones:
		if zone.size() != 3 or not zone.has("name") \
				or not zone.has("anchor_x") or not zone.has("anchor_y"):
			return false
	return true

func _success_marker() -> String:
	return "GRUS_COMBAT_LIFECYCLE_SMOKE_OK stage=restart-fresh fixture=4/1/18 warning=North live_aliens=true fog=hidden_then_revealed markers=visibility_gated controller_attack_move=accepted controller_visible_alien_right_click=accepted right_click_target=guard_event_readback crystal_reward=1 pause_frozen=true result=alien_defeat result_reject=session_locked restart_wave_reset=true research_boundary=analyze_reject8_zero_crystal research_progress=20x_advancing restart_research=zero blip_cue=counted_throttled_cleared"

func _run() -> void:
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame
	_main = get_node("Main") as Node3D
	_fx = _main.get_node("CombatFx") as Node3D
	_minimap = _main.get_node("HUD/Minimap") as Control

	if not _cue_telemetry_leg():
		return

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
	var result_bar := _main.get_node_or_null("HUD/ResultBar") as Panel
	if result_bar == null or result_bar.visible:
		_fail("Result bar is missing or visible during the Start phase")
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
	var start_capture := await SmokeHelpers.capture_frame(get_tree(), "combat-start")
	if not start_capture.is_empty():
		_fail(start_capture)
		return
	# Focused Start evidence uses the same test-side focus convention as the
	# Pause leg: the global Tab handler owns focus navigation, so the frame is
	# set up here and released before the real Start transition.
	var start_control := _button("HUD/SessionPanel/StartButton")
	start_control.grab_focus()
	if not await _wait_until(
			func(): return _focus_error(start_control, "Start").is_empty(), 5.0,
			"Start did not take the test-set keyboard focus"):
		return
	var start_focused_capture := await SmokeHelpers.capture_frame(get_tree(), "start-focused")
	if not start_focused_capture.is_empty():
		_fail(start_focused_capture)
		return
	start_control.release_focus()

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
	if int((_fx.get("cue_counts") as Dictionary).get("winter_wind", 0)) < 1:
		_fail("Playing did not start the pilot wind cue")
		return

	# HPA-188: the fresh match's exact zero-Crystal stockpile makes this the
	# only deterministic InsufficientResources window — the enabled Analyze
	# button, the real Research Center pick and the live research surface
	# are exercised before the first Crystal can be earned.
	if not await _research_boundary_leg():
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
	var scout_target := Vector2(anchor.x, anchor.y + 22.0)
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
	# North-of-the-city staging point, far enough south that its projection
	# clears the re-laid-out top research card: a click aimed under a HUD
	# STOP surface would be swallowed rather than exercising the order.
	var defender_target := Vector2(anchor.x, anchor.y + 32.0)
	var defenders_revision := int(GrusBridge.command_feedback_revision())
	if not await _select_guards(guards, guard_ids):
		return
	# Keyboard Attack-move must also leave an active build placement; these are
	# Guards, so the subsequent real right-click is an accepted command and is
	# awaited authoritatively below.
	var bunkhouse_tile := _button("HUD/CommandPanel/BuildGrid/BunkhouseButton")
	SmokeHelpers.push_mouse_click(get_tree(), bunkhouse_tile.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(_main._placement_kind) != "Bunkhouse" or str(_main._armed_order) != "":
		_fail("build tile did not arm placement before the Guard attack-move crossover")
		return
	await _key_tap(KEY_A)
	if str(_main._placement_kind) != "" or str(_main._armed_order) != "attack_move":
		_fail("controller A-key did not leave placement before arming Guard attack-move")
		return
	var defender_screen: Vector2 = _main.camera.unproject_position(Vector3(defender_target.x, 0.0, defender_target.y))
	# The "queued" text is a synchronous transient written inside the input
	# dispatch itself; _process later mirrors the real verdict over it. Read
	# it before any frame await — under fps throttling several physics ticks
	# (and the verdict) can land before the next render pass.
	SmokeHelpers.push_mouse_click(get_tree(), defender_screen, MOUSE_BUTTON_RIGHT)
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
			return int(snapshot.get("wave", 0)) >= 1 and int(snapshot.get("live_aliens", 0)) >= 10,
			45.0, "first wave did not launch ten live aliens"):
		return
	# The spawn-visibility sample below must land while the entrants still sit
	# in their fogged spawn cells. Two pacing facts stack against that at the
	# windowed renderer's ~4.5 FPS: the metadata-stamp pass that first makes
	# alien views countable can lag the spawn by ~2 wall-s, and at 5x (or even
	# 1x) the swarm uses that window to march several cells toward the scout.
	# The scout therefore waits beyond the whole slot spread (anchor+22 keeps
	# every spawn slot >=2.5 cells outside its 10-cell vision), and 0.25x (the
	# smoke_test gate-corridor precedent for render-paced sampling) pins the
	# remaining drift under ~2 cells. The 5x combat march resumes right after
	# the check.
	GrusBridge.set_sim_speed(0.25)
	if not await _wait_until(func(): return _alien_views().size() >= 10, 5.0,
			"live wave aliens never reached the Godot view tree"):
		return
	# The allied scout should not reveal every entrant: live alien views remain
	# in the scene tree under fog while only current-vision aliens become
	# minimap markers.
	if _hidden_alien_count() == 0:
		_fail("wave spawn exposed every live alien despite fog of war")
		return
	GrusBridge.set_sim_speed(5.0)
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
	# The approved layout's research card covers the northern approach at the
	# default framing. Recenter through the minimap's own real click path when
	# the alien projects under a HUD surface; the click then lands on open
	# world center where a player could actually attack it.
	var target_screen: Vector2 = _main.camera.unproject_position(target.global_position)
	if SmokeHelpers.hud_rects(_main.get_node("HUD")).any(func(rect: Rect2) -> bool: return rect.has_point(target_screen)):
		var minimap := _main.get_node("HUD/Minimap") as Control
		SmokeHelpers.push_mouse_click(get_tree(),
			minimap.get_global_rect().position
				+ Vector2(target.global_position.x, target.global_position.z) * (minimap.size / Vector2(128.0, 96.0)),
			MOUSE_BUTTON_LEFT)
		await get_tree().process_frame
		await get_tree().process_frame
		if not is_instance_valid(target):
			# The recentering click's frame wait is enough for the defense to
			# kill the scouted alien — its view frees with the sim entity, so
			# re-pick the revealed target rather than read a freed node.
			if not _visible_alien(target_box):
				_fail("the revealed alien died before the attack could be clicked")
				return
			target = target_box["view"]
		target_screen = _main.camera.unproject_position(target.global_position)
	var target_id := int(target.get_meta("unit_id", -1))
	var crystals_before := int(GrusBridge.economy_snapshot().get("crystals", -1))
	var attack_revision := int(GrusBridge.command_feedback_revision())
	# Target-identity read-back (HPA-697 fix round 2): the accepted
	# right-click must strike the picked alien — a combat event must name one
	# selected Guard as attacker and the winning view's real unit id as a
	# "unit" target, proving the dispatched command's actual kind and id.
	# The observer starts BEFORE the click: if all three Guards strike in
	# the tick that accepts the attack (3 × 15 damage exceeds the Swarmer's
	# 40 health), those events would otherwise sit in the shared queue while
	# the acceptance wait still polls, and the controller's per-frame
	# cosmetics drain would consume them before this observer ever ran.
	var strike := {"seen": false, "done": false}
	_observe_guard_strike(guard_ids, target_id, strike)
	# Same synchronous-transient read as the attack-move announcement above:
	# the verdict mirror in _process may legitimately replace it before any
	# awaited frame completes under throttled fps.
	SmokeHelpers.push_mouse_click(get_tree(), target_screen, MOUSE_BUTTON_RIGHT)
	if str(_main.command_status.text) != "Attack command queued":
		_fail("controller right-click did not queue a direct attack on the visible alien: %s" % [_main.command_status.text])
		return
	if not await _wait_until(
			func(): return int(GrusBridge.command_feedback_revision()) > attack_revision \
					and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			10.0, "simulation rejected the live-alien attack"):
		return
	if not await _wait_until(func(): return bool(strike["seen"]) or bool(strike["done"]),
			20.0, "the strike observer never finished"):
		return
	if not bool(strike["seen"]):
		_fail("the accepted attack never struck the picked alien as a Guard-targeted unit %d" % target_id)
		return
	if not await _wait_until(func(): return bool(strike["done"]), 5.0,
			"the strike observer never stopped draining"):
		return
	# The observer forwards its drained events straight into the cosmetics
	# dispatch, so its strikes alone could satisfy the final blip check even
	# if the controller's own per-frame drain never ran. Snapshot here: any
	# later generic_blip must have arrived through _refresh_session.
	var observer_forwarded_blips := _blip_cue_count()
	if not await _wait_until(
			func(): return int(GrusBridge.economy_snapshot().get("crystals", -1)) > crystals_before,
			15.0, "Guard combat never killed an alien or credited Crystals"):
		return
	if int(GrusBridge.economy_snapshot().get("crystals", -1)) != crystals_before + 1:
		_fail("one Swarmer kill did not credit exactly one Crystal: %s" % [GrusBridge.economy_snapshot()])
		return

	# A phase change clears the one modal order: arm Attack-move through its
	# real tile, pause via the bridge, and require the controller to drop it
	# before any resumed gameplay click can consume it.
	var attack_tile := _main.get_node("HUD/CommandPanel/OrderGrid/AttackMoveButton") as Button
	if attack_tile == null:
		_fail("Attack-move order tile is missing")
		return
	SmokeHelpers.push_mouse_click(get_tree(), attack_tile.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(_main._armed_order) != "attack_move":
		_fail("Attack-move tile did not arm before the pause phase change")
		return
	# Pause freezes health and combat-event totals. Resume into a real alien
	# Defeat; the remaining wave still attacks the sole Research Center. Both
	# transitions take the visible Button path (hit test, pressed signal and
	# handler), not a direct bridge call.
	# HPA-698 visual polish: the compact Pause composition must not permanently
	# replace the Start/Result card. Capture the untouched Start geometry while
	# Playing, then require the real Result transition to restore it.
	var start_panel_rect := _session_panel().get_global_rect()
	var pause_button := _button("HUD/PauseButton")
	if not pause_button.visible or pause_button.disabled:
		_fail("Pause button is not visible and enabled while Playing")
		return
	SmokeHelpers.push_mouse_click(get_tree(), pause_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Paused", 5.0,
			"real Pause button click never reached Paused"):
		return
	if not await _wait_until(func(): return str(_main._armed_order) == "", 5.0,
			"Paused phase did not clear the armed order"):
		return
	if not await _wait_until(
			func(): return _session_panel().visible \
					and _button("HUD/SessionPanel/ResumeButton").visible \
					and not _button("HUD/PauseButton").visible \
					and not _button("HUD/SessionPanel/QuitButton").visible \
					and not _result_bar().visible \
					and _session_label().text == "Paused",
			5.0, "Paused overlay is not showing Resume with Pause/Quit hidden"):
		return
	# The Pause card is a compact, centered composition with Resume inside it
	# and on screen; the Start/Result card geometry is restored on transition.
	var paused_rect := _session_panel().get_global_rect()
	var pause_viewport := Rect2(Vector2.ZERO, TEST_VIEWPORT_SIZE)
	var paused_resume_rect := _button("HUD/SessionPanel/ResumeButton").get_global_rect()
	if not await _wait_until(func():
				var card := _session_panel().get_global_rect()
				return card.size.x <= 420.0 and card.size.y <= 280.0 \
					and absf(card.get_center().x - pause_viewport.get_center().x) <= 1.0 \
					and absf(card.get_center().y - pause_viewport.get_center().y) <= 1.0 \
					and pause_viewport.encloses(card) \
					and card.encloses(_button("HUD/SessionPanel/ResumeButton").get_global_rect()),
			5.0, "Paused overlay is not a compact, centered composition with a reachable Resume: card=%s resume=%s" % [
				paused_rect, paused_resume_rect]):
		return
	# Prominent Resume: the same real Pause entry must show the bright primary
	# action, not a neutral secondary plate.
	if not await _wait_until(func(): return _primary_action_error().is_empty(), 5.0,
			"Pause card actions are not clear primaries: %s" % _primary_action_error()):
		return
	# Live resize while Paused: the card must follow the viewport without
	# locking window size, including when returning to the required 1280 x 720
	# target, and must stay a usable composition throughout. Gameplay remains
	# paused; only the root viewport size changes.
	# The project stretches the 1280 x 720 layout with the expand aspect, so a
	# 16:9 window keeps the base viewport and only other aspects grow it: the
	# targets are a 16:10 and a 4:3 window, then back to the base size.
	var resize_targets: Array[Vector2i] = [
		Vector2i(1600, 1000), Vector2i(960, 720), TEST_VIEWPORT_SIZE]
	for window_size in resize_targets:
		var resize_size := _stretched_viewport(window_size)
		get_tree().root.size = window_size
		if not await _wait_until(
				func(): return get_viewport().get_visible_rect().size == Vector2(resize_size),
				5.0, "root viewport did not resize to %s" % resize_size):
			return
		if not await _wait_until(
				func(): return _paused_resize_error(resize_size).is_empty(), 5.0,
				"Paused card did not follow the live %s viewport" % resize_size):
			return
	var wind_player := (_fx.get("_players") as Dictionary).get("winter_wind") as AudioStreamPlayer
	if not wind_player.stream_paused:
		_fail("Paused did not pause the pilot wind cue")
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
	# Paused inspection stays available: a real Tab opens the drawer without
	# resuming, and the drawer survives Resume so the Result transition below
	# is the one that must close it.
	var ug_drawer := _ug_drawer()
	await _key_tap(KEY_TAB)
	if not ug_drawer.visible \
			or str(GrusBridge.session_snapshot().get("phase", "")) != "Paused":
		_fail("real Tab during Paused did not open the inspection drawer")
		return
	var resume_button := _button("HUD/SessionPanel/ResumeButton")
	if not resume_button.visible or resume_button.disabled:
		_fail("Resume button is not visible and enabled while Paused")
		return
	# Focused Pause evidence. The global Tab handler owns focus navigation, so
	# this leg sets focus test-side and then activates with a real Enter; the
	# unfocused/focused pair isolates the ring pixels from every other change.
	var unfocused_capture := await SmokeHelpers.capture_frame(get_tree(), "pause-unfocused")
	if not unfocused_capture.is_empty():
		_fail(unfocused_capture)
		return
	resume_button.grab_focus()
	if not await _wait_until(func(): return _focus_error(resume_button, "Resume").is_empty(), 5.0,
			"Resume did not take the test-set keyboard focus"):
		return
	var focused_capture := await SmokeHelpers.capture_frame(get_tree(), "pause-focused")
	if not focused_capture.is_empty():
		_fail(focused_capture)
		return
	await _key_tap(KEY_ENTER)
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Playing", 5.0,
			"real Enter on focused Resume never resumed to Playing"):
		return
	# Re-enter Pause so the original real mouse Resume click remains covered:
	# the open drawer closes on the first Esc, the second pauses, then Tab
	# reopens the inspection drawer for the unchanged survive-Resume check.
	await _key_tap(KEY_ESCAPE)
	await _key_tap(KEY_ESCAPE)
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Paused", 5.0,
			"second real Pause leg never reached Paused"):
		return
	await _key_tap(KEY_TAB)
	if not await _wait_until(func():
				return str(GrusBridge.session_snapshot().get("phase", "")) == "Paused" \
					and ug_drawer.visible,
			5.0, "second Pause leg did not reopen the inspection drawer"):
		return
	SmokeHelpers.push_mouse_click(get_tree(), resume_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Playing", 5.0,
			"real Resume button click never resumed to Playing"):
		return
	if not ug_drawer.visible:
		_fail("Paused inspection drawer did not survive Resume into Playing")
		return
	# Opt-in real-render evidence of the damaged painted Research Center under
	# its real assault: the world health bar renders only below full health, so
	# a capture run waits for actual damage instead of staging any state.
	if SmokeHelpers.capture_enabled():
		if not await _wait_until(
				func(): return float(_building_view(1).call("health_ratio")) < 0.999,
				45.0, "the final assault never damaged the Research Center for the damaged-building capture"):
			return
		var damaged_capture := await SmokeHelpers.capture_frame(get_tree(), "damaged-building")
		if not damaged_capture.is_empty():
			_fail(damaged_capture)
			return
	# The Research Center is the only lose condition: its health chip appears
	# once it is damaged, names the alert while the hit is fresh, mirrors the
	# live health ratio, and is gone with the in-match HUD on Result.
	var rc_alert := _main.get_node("HUD/RcAlert") as Panel
	var rc_alert_label := _main.get_node("HUD/RcAlert/RcAlertLabel") as Label
	var rc_health_bar := _main.get_node("HUD/RcAlert/RcHealthBar") as ProgressBar
	if not await _wait_until(func(): return rc_alert.visible, 45.0,
			"damaged Research Center never raised its HUD health chip"):
		return
	if not rc_alert_label.text.begins_with("Research Center ") \
			or rc_health_bar.value <= 0.0 or rc_health_bar.value >= 0.999:
		_fail("Research Center chip is not a damaged-health readout: '%s' %s" % [
			rc_alert_label.text, rc_health_bar.value])
		return
	if not await _wait_until(func(): return str(GrusBridge.session_snapshot().get("phase", "")) == "Result", 45.0,
			"surviving aliens never destroyed the sole Research Center"):
		return
	# The controller hides the chip in _process, which can lag the physics-frame
	# poll that saw the phase flip, so wait for it like the drawer below.
	if not await _wait_until(func(): return not rc_alert.visible, 5.0,
			"Research Center chip stayed visible on the Result screen"):
		return
	# Entering Result closes the drawer even though it was open before the
	# outcome, and Tab cannot reopen it there.
	if not await _wait_until(func(): return not ug_drawer.visible, 5.0,
			"Result phase still exposes the UndergroundPanel drawer"):
		return
	await _key_tap(KEY_TAB)
	if ug_drawer.visible \
			or str(GrusBridge.session_snapshot().get("phase", "")) != "Result":
		_fail("Tab reopened or leaked through the Result phase")
		return
	var result: Dictionary = GrusBridge.session_snapshot()
	if int(result.get("winner_team", -1)) != 2:
		_fail("alien-caused Result is not Team 2 Defeat: %s" % [result])
		return
	if not await _wait_until(
			func(): return _session_panel().visible and not _session_label().visible \
					and (_main.get_node("HUD/SessionBackdrop/ResultTitle") as Label).text == "Defeat" \
					and _button("HUD/SessionPanel/RestartButton").visible \
					and _button("HUD/SessionPanel/QuitButton").visible,
			5.0, "Result overlay is not showing Defeat with Restart/Quit"):
		return
	# The genuine alien Defeat screen is where Restart is the live primary, so
	# the shared plate must keep the same readable ink and distinct focus ring
	# here as it does on the real Start and Pause screens.
	var defeat_primary_error := _primary_action_error()
	if not defeat_primary_error.is_empty():
		_fail("Result Restart is not a clear primary action: %s" % defeat_primary_error)
		return
	# Leaving Pause restores the Start/Result card exactly; otherwise Restart/
	# Quit would inherit the compact Pause card's offsets.
	if _session_panel().get_global_rect() != start_panel_rect:
		_fail("Result did not restore the Start/Result session card geometry: result=%s start=%s" % [
			_session_panel().get_global_rect(), start_panel_rect])
		return
	var result_banner := _main.get_node("HUD/SessionBackdrop/ResultBanner") as TextureRect
	if result_banner == null or not result_banner.visible \
			or result_banner.texture == null \
			or not result_banner.texture.resource_path.ends_with("screens/defeat_banner.png") \
			or result_banner.mouse_filter != Control.MOUSE_FILTER_IGNORE:
		_fail("Result overlay is not showing the noninteractive Defeat banner art")
		return
	# The source banners carry no text, so the actual session-driven outcome
	# title must be visibly rendered over them with the two Result actions.
	var result_title := _main.get_node("HUD/SessionBackdrop/ResultTitle") as Label
	if result_title == null or not result_title.visible \
			or result_title.text != "Defeat" \
			or result_title.get_theme_font_size("font_size") < 40 \
			or not Rect2(Vector2.ZERO, Vector2(TEST_VIEWPORT_SIZE)).encloses(result_title.get_global_rect()) \
			or not result_title.get_global_rect().intersects(result_banner.get_global_rect()) \
			or _button("HUD/SessionPanel/RestartButton").disabled \
			or _button("HUD/SessionPanel/QuitButton").disabled:
		_fail("Result overlay does not show the readable session Defeat title over its banner")
		return
	# HPA-698 visual fidelity Result: the existing backdrop still covers the
	# whole viewport, the fog and the renderer's default clear color agree
	# (the opt-in captured frame is what proves the pixels), and each winner's
	# title sits on its own cloth instead of the same fixed box.
	var session_backdrop := _main.get_node("HUD/SessionBackdrop") as Control
	if not session_backdrop.visible \
			or session_backdrop.get_global_rect() != Rect2(Vector2.ZERO, Vector2(TEST_VIEWPORT_SIZE)):
		_fail("Result backdrop does not cover the viewport: %s" % session_backdrop.get_global_rect())
		return
	# One shared backdrop alpha serves Pause and Result, capped at 0.55 so the
	# real battlefield stays legible behind the outcome while the banner and
	# title carry the hierarchy.
	var session_dim := _main.get_node_or_null("HUD/SessionBackdrop/Dim") as ColorRect
	if session_dim == null or not session_dim.visible or session_dim.color.a > 0.55:
		_fail("Result dim is missing, hidden, or darker than the shared 0.55 cap: %s" % [
			session_dim.color if session_dim != null else null])
		return
	# The tint follows the outcome cloth's own warm/cold vocabulary instead of
	# the neutral Pause wash; the alpha cap above still governs legibility.
	if not session_dim.color.r > session_dim.color.b + 0.05:
		_fail("Defeat dim is not warm-tinted: %s" % [session_dim.color])
		return
	var fog_error := SmokeHelpers.fog_clear_color_error(_main.get_node("Battlefield/FogOfWar"))
	if not fog_error.is_empty():
		_fail(fog_error)
		return
	if not result_banner.get_global_rect().encloses(result_title.get_global_rect()):
		_fail("Defeat title is not placed inside its banner cloth")
		return
	var defeat_title_center := result_title.get_global_rect().get_center()
	var defeat_capture := await SmokeHelpers.capture_frame(get_tree(), "combat-defeat")
	if not defeat_capture.is_empty():
		_fail(defeat_capture)
		return
	# Focused Restart evidence on the genuine Defeat screen. This is the same
	# test-set focus convention and shared primary ring; focus is released
	# before the real Restart click below.
	var result_restart := _button("HUD/SessionPanel/RestartButton")
	result_restart.grab_focus()
	if not await _wait_until(
			func(): return _focus_error(result_restart, "Restart").is_empty(), 5.0,
			"Restart did not take the test-set keyboard focus"):
		return
	var defeat_focused_capture := await SmokeHelpers.capture_frame(get_tree(), "defeat-focused")
	if not defeat_focused_capture.is_empty():
		_fail(defeat_focused_capture)
		return
	result_restart.release_focus()
	# Result actions sit in a bottom bar matching the mockup composition. The
	# bar is decorative (mouse IGNORE); the two named buttons stay distinct and
	# reachable, and every printed figure comes from a live read-only snapshot.
	var result_stats := _main.get_node_or_null("HUD/ResultBar/ResultStats") as RichTextLabel
	var restart_button := _button("HUD/SessionPanel/RestartButton")
	var quit_button := _button("HUD/SessionPanel/QuitButton")
	if result_bar == null or not result_bar.visible \
			or result_bar.mouse_filter != Control.MOUSE_FILTER_IGNORE \
			or result_stats == null or not result_stats.is_visible_in_tree():
		_fail("Result bar is missing, not visible, or intercepts mouse input")
		return
	var viewport_rect := Rect2(Vector2.ZERO, Vector2(TEST_VIEWPORT_SIZE))
	var bar_rect := result_bar.get_global_rect()
	if bar_rect.position.y < TEST_VIEWPORT_SIZE.y * 0.75 or not viewport_rect.encloses(bar_rect):
		_fail("Result bar is not a visible bottom bar at 1280x720: %s" % [bar_rect])
		return
	var restart_rect := restart_button.get_global_rect()
	var quit_rect := quit_button.get_global_rect()
	if not bar_rect.encloses(restart_rect) or not bar_rect.encloses(quit_rect) \
			or restart_rect.intersects(quit_rect) \
			or not viewport_rect.encloses(restart_rect) or not viewport_rect.encloses(quit_rect) \
			or restart_button.disabled or quit_button.disabled \
			or not restart_button.is_visible_in_tree() or not quit_button.is_visible_in_tree():
		_fail("Result bar Restart/Quit are not distinct, reachable controls")
		return
	# Composition: the outcome stage is symmetric about the 1280 px viewport,
	# both actions are vertically centred and inset from its right edge, and
	# the snapshot statistics keep their own space clear of them.
	var left_margin := bar_rect.position.x
	var right_margin := float(TEST_VIEWPORT_SIZE.x) - bar_rect.end.x
	if absf(left_margin - right_margin) > 1.0:
		_fail("Result bar is not centered in the 1280 px stage: margins %.1f / %.1f" % [
			left_margin, right_margin])
		return
	if absf(restart_rect.get_center().y - bar_rect.get_center().y) > 1.0 \
			or absf(quit_rect.get_center().y - bar_rect.get_center().y) > 1.0:
		_fail("Result actions are not vertically centered in the bar: restart=%s quit=%s bar=%s" % [
			restart_rect, quit_rect, bar_rect])
		return
	if bar_rect.end.x - quit_rect.end.x < 16.0 or bar_rect.end.x - quit_rect.end.x > 40.0:
		_fail("Result actions are not inset from the bar's right edge: quit=%s bar=%s" % [
			quit_rect, bar_rect])
		return
	if not bar_rect.encloses(result_stats.get_global_rect()) \
			or restart_rect.intersects(result_stats.get_global_rect()) \
			or quit_rect.intersects(result_stats.get_global_rect()):
		_fail("Result statistics do not keep their own space inside the bar: stats=%s restart=%s quit=%s" % [
			result_stats.get_global_rect(), restart_rect, quit_rect])
		return
	if result_stats.get_content_height() > int(result_stats.size.y):
		_fail("Result statistics wrap past their rect (content %d px in a %d px label)" % [
			result_stats.get_content_height(), int(result_stats.size.y)])
		return
	var result_research: Dictionary = GrusBridge.research_snapshot()
	var result_economy: Dictionary = GrusBridge.economy_snapshot()
	var result_wave: Dictionary = GrusBridge.wave_snapshot()
	var result_target := int((GrusBridge.catalogue_snapshot().get("research", {}) as Dictionary).get("target_points", 0))
	var result_elapsed := int(float(result_wave.get("elapsed_seconds", -1.0)))
	for expected in [
			"Time %d:%02d" % [result_elapsed / 60, result_elapsed % 60],
			"Wave %d" % int(result_wave.get("wave", 0)),
			"Research %s/%s" % [
				HudHelpers.format_int(floori(float(result_research.get("points", 0.0)))),
				HudHelpers.format_int(result_target),
			],
			"Pop %d" % int(result_economy.get("population_used", 0)),
			"Crystals %d" % int(result_economy.get("crystals", 0))]:
		if not result_stats.text.contains(expected):
			_fail("Result bar statistics are not snapshot-derived (missing %s): %s" % [expected, result_stats.text])
			return
	# HPA-698 finding 1: the Result figures are icon-led, so each of the four
	# inline images must carry the catalogue-worded alternative the pinned
	# RichTextLabel parser exposes as that ROLE_IMAGE's accessible name.
	var result_alt_error := SmokeHelpers.inline_image_alt_error(result_stats, {
		"res://assets/ui/icons/status/wave_countdown.png": "Wave Countdown",
		"res://assets/ui/icons/status/research_progress.png": "Research Progress",
		"res://assets/ui/icons/status/population.png": "Population",
		"res://assets/ui/icons/resources/crystals.png": "Crystals",
	}, "ResultStats")
	if not result_alt_error.is_empty():
		_fail(result_alt_error)
		return
	# The outcome composition replaces the in-match HUD; no live gameplay
	# control may sit exposed behind or beside the Result bar.
	for path in ["HUD/EconomyPanel", "HUD/ResearchPanel", "HUD/SelectionPanel",
			"HUD/CommandPanel", "HUD/WavePanel", "HUD/WaveRing", "HUD/WaveLabel",
			"HUD/MinimapFrame", "HUD/Minimap", "HUD/CommandStatus",
			"HUD/UndergroundPanel"]:
		if (_main.get_node(path) as CanvasItem).visible:
			_fail("Result phase still exposes in-match HUD %s" % path)
			return
	# Presentation-only variant probe: call the HUD render directly with both
	# winner values and restore the actual session immediately. This is not a
	# gameplay victory journey; it proves the banner/title pair reacts to the
	# same session field without manufacturing sim state.
	var hud := _main.get_node("HUD")
	# Hold the controller's per-frame refresh for the probe only: the awaited
	# capture needs a rendered frame, and leaving _process running would
	# overwrite the presentation-only winner before that frame draws.
	_main.set_process(false)
	hud.call("refresh_session", "Result", 1)
	var victory_title_center := result_title.get_global_rect().get_center()
	if not result_title.visible or result_title.text != "Victory" \
			or result_banner.texture == null \
			or not result_banner.texture.resource_path.ends_with("screens/victory_banner.png") \
			or not result_bar.visible or not result_stats.is_visible_in_tree() \
			or _button("HUD/SessionPanel/RestartButton").disabled \
			or _button("HUD/SessionPanel/QuitButton").disabled \
			or not result_banner.get_global_rect().encloses(result_title.get_global_rect()) \
			or is_equal_approx(victory_title_center.y, defeat_title_center.y):
		_main.set_process(true)
		_fail("presentation probe did not render the Victory title on its own banner cloth height")
		return
	if not session_dim.color.b > session_dim.color.r + 0.03:
		_main.set_process(true)
		_fail("Victory probe dim did not follow the cold outcome tint: %s" % [session_dim.color])
		return
	var victory_capture := await SmokeHelpers.capture_frame(get_tree(), "combat-victory-probe")
	_main.set_process(true)
	if not victory_capture.is_empty():
		_fail(victory_capture)
		return
	hud.call("refresh_session", "Result", 2)
	if not result_title.visible or result_title.text != "Defeat" \
			or result_banner.texture == null \
			or not result_banner.texture.resource_path.ends_with("screens/defeat_banner.png") \
			or not result_bar.visible:
		_fail("presentation probe did not restore the Defeat title/art")
		return
	if not session_dim.color.r > session_dim.color.b + 0.05:
		_fail("presentation probe did not restore the warm Defeat tint: %s" % [session_dim.color])
		return
	var actual_result: Dictionary = GrusBridge.session_snapshot()
	hud.call("refresh_session", str(actual_result.get("phase", "")), int(actual_result.get("winner_team", -1)))
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
	_main._armed_order = "attack_move"
	_fx.call("spawn_death", Vector3.ZERO)
	if int(_fx.call("effect_count")) < 1:
		_fail("transient effect seed for the restart check did not land")
		return
	# Banked research must be non-zero here — the whole Playing journey
	# accrued it — so the post-restart zero assertions below are never
	# vacuous.
	if float(GrusBridge.research_snapshot().get("points", 0.0)) <= 0.0:
		_fail("research never banked points during the defeat journey: %s" % [GrusBridge.research_snapshot()])
		return
	if _blip_cue_count() <= observer_forwarded_blips:
		_fail("no combat event reached the blip cue through the controller's own per-frame drain: %s" % [_fx.get("cue_counts")])
		return
	# The Result-phase Restart button is actually visible here, so a real
	# click exercises the full production path — hit test, pressed signal
	# and handler — rather than a private handler call.
	SmokeHelpers.push_mouse_click(get_tree(),
		_button("HUD/SessionPanel/RestartButton").get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
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
	for path in ["HUD/EconomyPanel", "HUD/SelectionPanel", "HUD/CommandPanel", "HUD/Minimap"]:
		if not (_main.get_node(path) as CanvasItem).visible:
			_fail("restart did not restore in-match HUD %s" % path)
			return
	if _button("HUD/SessionPanel/RestartButton").visible:
		_fail("restart left the Result bar actions visible during Start")
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
			or int(_main.selected_building_id) != -1 or str(_main._armed_order) != "":
		_fail("restart left old selection/control-group state behind")
		return
	if int(_fx.call("effect_count")) != 0:
		_fail("restart left transient combat effects on stage")
		return
	if not _cue_counts_empty():
		_fail("restart left stale cue telemetry: %s" % [_fx.get("cue_counts")])
		return

	# The real restart path wipes banked research with the match: the live
	# snapshot, the HUD bar and the Analyze enable all return to zero.
	if float(GrusBridge.research_snapshot().get("points", -1.0)) != 0.0:
		_fail("restart did not reset research points: %s" % [GrusBridge.research_snapshot()])
		return
	var reset_bar := _main.get_node("HUD/ResearchPanel/ResearchBar") as ProgressBar
	var reset_analyze := _button("HUD/ResearchPanel/AnalyzeButton")
	if not await _wait_until(func():
			return reset_bar.value == 0.0 and reset_analyze.disabled,
			5.0, "restart left stale research HUD progress or an enabled Analyze"):
		return

	var marker := _success_marker()
	if marker.is_empty():
		GrusBridge.set_sim_speed(1.0)
		get_tree().quit(1)
		return
	print(marker)
	GrusBridge.set_sim_speed(1.0)
	var quit_controller = get_node("Main")
	await quit_controller.request_quit()
