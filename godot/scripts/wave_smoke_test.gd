extends "res://scripts/combat_lifecycle_smoke_test.gd"

## Stage 5 presentation checks layered over the real survival journey. The
## parent smoke owns all gameplay; this scene checks its HUD/minimap rendering.

const STAGE5_ALIEN_KINDS := ["Swarmer", "Spitter", "Brute", "Behemoth"]
const UNIT_MODEL_KINDS := ["Worker", "Guard", "Rifleman"] + STAGE5_ALIEN_KINDS
const UNIT_VIEW_SCENE := preload("res://scenes/unit_view.tscn")

var _stage5_passed := false
var _wave_ring: Control
var _incoming_glow: Control

func _visible_text(control: Control) -> String:
	if control is RichTextLabel:
		return (control as RichTextLabel).get_parsed_text()
	return str(control.get("text"))

func _rich_text_has_icon(control: Control, resource_path: String) -> bool:
	return control is RichTextLabel and (control as RichTextLabel).text.contains(resource_path)

func _ring_matches(snapshot: Dictionary) -> bool:
	var total := float(snapshot.get("timer_total_seconds", 0.0))
	if total <= 0.0:
		return false
	var expected := clampf(float(snapshot.get("seconds_remaining", 0.0)) / total, 0.0, 1.0)
	return absf(float(_wave_ring.ring_ratio) - expected) <= 0.05

func _glow_visibility() -> Dictionary:
	var result := {}
	for direction in ["North", "East", "South", "West"]:
		result[direction] = (_incoming_glow.get_node(direction) as Control).visible
	return result

func _wave_hud_fits(control: Control) -> bool:
	var rect := control.get_global_rect()
	var content_size := control.get_minimum_size()
	if control is RichTextLabel:
		var rich_label := control as RichTextLabel
		content_size = Vector2(rich_label.get_content_width(), rich_label.get_content_height())
		if rich_label.get_visible_line_count() != rich_label.get_line_count():
			return false
	rect.size = rect.size.max(content_size)
	return Rect2(Vector2.ZERO, get_viewport().get_visible_rect().size).encloses(rect)

## The compact card is narrower than the text it used to hold: every line must
## stay inside the label's own rectangle, not merely inside the viewport.
func _wave_text_fits(control: Control) -> bool:
	var rich_label := control as RichTextLabel
	if rich_label == null:
		return false
	if rich_label.get_visible_line_count() != rich_label.get_line_count():
		return false
	return rich_label.get_content_width() <= rich_label.size.x + 0.5 \
			and rich_label.get_content_height() <= rich_label.size.y + 0.5

## Plausible minimap anchors for copied warning probes; the live fixture's own
## anchors are asserted separately below.
const ZONE_ANCHORS := {
	"North": Vector2i(64, 0), "East": Vector2i(128, 48),
	"South": Vector2i(64, 96), "West": Vector2i(0, 48),
}

## A copied warning snapshot that matches the wave director's own rotation: the
## authored wave 20 is three zones and the regular interval is 100 s, so the
## probe keeps wave/next/timer internally consistent instead of "wave 1 with
## wave-20 zones". Never sent to the sim.
func _wave_warning_probe(snapshot: Dictionary, wave: int, next_wave: int,
		zone_names: Array) -> Dictionary:
	var probe: Dictionary = snapshot.duplicate(true)
	probe["wave"] = wave
	probe["next_wave"] = next_wave
	probe["timer_total_seconds"] = 100.0
	probe["seconds_remaining"] = 29.0
	probe["live_aliens"] = 0
	var zones: Array = []
	for zone_name in zone_names:
		var anchor: Vector2i = ZONE_ANCHORS[zone_name]
		zones.append({"name": zone_name, "anchor_x": anchor.x, "anchor_y": anchor.y})
	probe["incoming_zones"] = zones
	return probe

## A copied non-warning snapshot for one wave index. `zone_names` empty leaves
## the incoming warning cleared.
func _wave_idle_probe(snapshot: Dictionary, wave: int, next_wave: int) -> Dictionary:
	return _wave_warning_probe(snapshot, wave, next_wave, [])

## The wave readout contract for one rendered (live or copied) snapshot: the
## countdown text fits its own rectangle, the warning's full direction names are
## visible in the readout, and the same names are in the card/label accessible
## descriptions rather than only in a pointer tooltip. When the warning is
## cleared, the warning readout and its semantic names must be cleared too.
func _wave_readout_error(main: Node3D, expected: Array, zones: Array, exact: Array,
		where: String) -> String:
	var wave_label := main.get_node("HUD/WaveLabel") as RichTextLabel
	var warning := main.get_node_or_null("HUD/WaveWarning") as RichTextLabel
	var card := main.get_node("HUD/WavePanel") as Control
	if warning == null:
		return "%s has no WaveWarning readout node" % where
	var text := _visible_text(wave_label)
	for needle in expected:
		if not text.contains(str(needle)):
			return "%s wave readout lost \"%s\": %s" % [where, needle, text]
	if not _wave_text_fits(wave_label) or wave_label.text.split("\n").size() != 3:
		return "%s main wave readout does not fit its rectangle on 3 lines: %s" % [where, text]
	var warning_text := _visible_text(warning)
	if zones.is_empty():
		if warning.is_visible_in_tree() or not warning_text.is_empty():
			return "%s cleared warning still reads \"%s\"" % [where, warning_text]
		for description in [wave_label.accessibility_description, card.accessibility_description]:
			if str(description).to_lower().contains("incoming"):
				return "%s cleared warning kept an incoming description: %s" % [
					where, description]
	else:
		if not warning.is_visible_in_tree():
			return "%s warning readout is hidden" % where
		if not card.get_global_rect().encloses(warning.get_global_rect()):
			return "%s warning readout escapes the card: %s / %s" % [
				where, warning.get_global_rect(), card.get_global_rect()]
		if not _wave_text_fits(warning):
			return "%s warning readout does not fit its rectangle: %s" % [
				where, warning_text]
		for zone_name in zones:
			if not warning.text.contains("Incoming: ") \
					or not warning.text.contains(str(zone_name)):
				return "%s visible warning lost the full direction \"%s\": %s" % [
					where, zone_name, warning.text]
			for description in [wave_label.accessibility_description,
					card.accessibility_description]:
				if not str(description).contains(str(zone_name)):
					return "%s accessible description lost \"%s\": %s" % [
						where, zone_name, description]
	for value in exact:
		if not str(wave_label.accessibility_description).contains(str(value)):
			return "%s accessible description lost the exact value \"%s\": %s" % [
				where, value, wave_label.accessibility_description]
	return ""

func _run() -> void:
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame
	var main := get_node_or_null("Main") as Node3D
	if main == null:
		_stage5_fail("main scene is missing")
		return
	var start_label := main.get_node_or_null("HUD/SessionPanel/SessionLabel") as Label
	if start_label == null or not start_label.text.to_lower().contains("surviv"):
		_stage5_fail("Start overlay does not describe survival: %s" % [start_label.text if start_label != null else "missing"])
		return
	var wave_label := main.get_node_or_null("HUD/WaveLabel") as Control
	if wave_label == null:
		_stage5_fail("wave HUD label is missing")
		return
	# HPA-698 fix round: the incoming directions get a readout of their own so
	# their full names stay visible; the label's accessible description carries
	# the exact wave indices for non-hover access.
	var wave_warning := main.get_node_or_null("HUD/WaveWarning") as RichTextLabel
	if wave_warning == null:
		_stage5_fail("wave HUD has no separate incoming-warning readout")
		return
	_wave_ring = main.get_node_or_null("HUD/WaveRing")
	_incoming_glow = main.get_node_or_null("HUD/IncomingGlow")
	if _wave_ring == null or _incoming_glow == null:
		_stage5_fail("wave countdown ring or incoming edge glow is missing")
		return
	if _incoming_glow.mouse_filter != Control.MOUSE_FILTER_IGNORE:
		_stage5_fail("incoming edge glow is interactive instead of mouse IGNORE")
		return
	var wave_panel := main.get_node_or_null("HUD/WavePanel") as Control
	if wave_panel == null or not wave_panel.get_global_rect().encloses(wave_label.get_global_rect()) \
			or not wave_panel.get_global_rect().encloses(_wave_ring.get_global_rect()):
		_stage5_fail("wave ring/label escape their panel: panel=%s ring=%s label=%s" % [
			wave_panel.get_global_rect() if wave_panel != null else "missing",
			_wave_ring.get_global_rect(), wave_label.get_global_rect()])
		return
	# HPA-698 visual follow-up: the wave card reads as part of the top rail, not
	# as a second floating bar far below it. The opening targets above the card
	# keep the economy smoke's clearance probe; this pins the composition.
	var top_bar := main.get_node_or_null("HUD/EconomyPanel") as Control
	if top_bar == null:
		_stage5_fail("top bar is missing for the wave-card rail check")
		return
	var rail_gap := wave_panel.get_global_rect().position.y - top_bar.get_global_rect().end.y
	if rail_gap < 0.0 or rail_gap > 8.0:
		_stage5_fail("wave card sits %.1f px below the top bar; the top rail gaps it 0..8 px" % [rail_gap])
		return
	# The countdown must be rendered from a live bridge read, not the
	# scene's placeholder text: poll until the label shows the snapshot's
	# next_wave, then require the snapshot's own remaining time on screen.
	var live_wave: Dictionary = {}
	for _frame in 8:
		live_wave = GrusBridge.wave_snapshot()
		if _visible_text(wave_label).contains("Next wave %d" % int(live_wave.get("next_wave", -1))):
			break
		await get_tree().process_frame
	var next_wave := int(live_wave.get("next_wave", -1))
	var remaining := maxi(ceili(float(live_wave.get("seconds_remaining", 0.0))), 0)
	var expected_time := "%d:%02d" % [int(remaining / 60), remaining % 60]
	var start_text := _visible_text(wave_label)
	if not _rich_text_has_icon(wave_label, "res://assets/ui/icons/status/wave_countdown.png"):
		_stage5_fail("wave HUD is missing status/wave_countdown.png")
		return
	if _rich_text_has_icon(wave_label, "res://assets/ui/icons/status/incoming_zone.png") \
			or _rich_text_has_icon(wave_warning, "res://assets/ui/icons/status/incoming_zone.png"):
		_stage5_fail("wave HUD shows incoming-zone art before a warning")
		return
	if not start_text.contains("Next wave %d" % next_wave) \
			or not start_text.contains(expected_time) or not _wave_hud_fits(wave_label) \
			or not _wave_text_fits(wave_label) \
			or wave_label.text.split("\n").size() != 3:
		_stage5_fail("wave HUD does not render the live snapshot countdown without clipping (want 'Next wave %d' + '%s' on 3 lines): %s" % [next_wave, expected_time, start_text])
		return
	# The warning readout is cleared before any zone; the readout's accessible
	# description still names the exact live wave index without hover.
	var initial_readout := _wave_readout_error(main, ["Prep", "Next wave %d" % next_wave],
		[], [str(next_wave)], "initial wave readout")
	if not initial_readout.is_empty():
		_stage5_fail(initial_readout)
		return
	if not _ring_matches(live_wave) or bool(_wave_ring.warning):
		_stage5_fail("idle wave ring does not render seconds_remaining/timer_total_seconds: ring=%s snapshot=%s" % [
			_wave_ring.ring_ratio, live_wave])
		return
	for direction in _glow_visibility():
		if bool(_glow_visibility()[direction]):
			_stage5_fail("incoming edge glow %s is visible before any warning zone" % direction)
			return
	if not await _probe_alien_roles():
		return

	var initial_wave: Dictionary = GrusBridge.wave_snapshot()
	if int(initial_wave.get("wave", -1)) != 0 or int(initial_wave.get("next_wave", -1)) != 1 \
			or not initial_wave.get("incoming_zones", []).is_empty():
		_stage5_fail("fresh Start wave snapshot is not the expected public surface: %s" % [initial_wave])
		return
	var minimap := main.get_node_or_null("HUD/Minimap") as Control
	if minimap == null or not _has_property(minimap, "incoming_ping_points"):
		_stage5_fail("minimap does not expose the wave-snapshot ping points")
		return
	var initial_pings: PackedVector2Array = minimap.get("incoming_ping_points")
	if not initial_pings.is_empty():
		_stage5_fail("minimap exposed an unannounced zone ping during Start: %s" % [initial_pings])
		return

	if not GrusBridge.set_sim_speed(20.0):
		_stage5_fail("could not accelerate Start timer freeze check")
		return
	for _frame in 12:
		await get_tree().physics_frame
	var after_start_wait: Dictionary = GrusBridge.wave_snapshot()
	if not is_equal_approx(float(after_start_wait.get("seconds_remaining", -1.0)), float(initial_wave.get("seconds_remaining", -2.0))):
		_stage5_fail("Start advanced the wave clock: before=%s after=%s" % [initial_wave, after_start_wait])
		return

	call_deferred("_verify_warning_presentation", main)
	await super._run()

func _probe_alien_roles() -> bool:
	# Presentation probes exercise all seven imported unit models;
	# the real wave journey below still uses bridge-authored unit metadata.
	var probes: Array[Node3D] = []
	for index in UNIT_MODEL_KINDS.size():
		var probe := UNIT_VIEW_SCENE.instantiate() as Node3D
		probe.name = "RoleProbe%d" % index
		probe.visible = true
		probe.set_meta("unit_id", 900 + index)
		probe.set_meta("team_id", 1 if index < 3 else 2)
		probe.set_meta("unit_kind", UNIT_MODEL_KINDS[index])
		add_child(probe)
		probes.append(probe)
	await get_tree().process_frame
	await get_tree().process_frame

	var meshes := {}
	for index in probes.size():
		var probe := probes[index]
		var model := probe.get_node_or_null("Model")
		var parts := model.find_children("*", "MeshInstance3D", true, false) if model != null else []
		var mesh: Mesh = parts[0].mesh if parts.size() == 1 else null
		var material := parts[0].get_active_material(0) as StandardMaterial3D if mesh != null else null
		if mesh == null or meshes.has(mesh) or material == null \
				or material.albedo_texture == null \
				or material.texture_filter != BaseMaterial3D.TEXTURE_FILTER_NEAREST \
				or probe.get_node("Body").visible:
			for old_probe in probes:
				old_probe.queue_free()
			_stage5_fail("unit role %s did not load a distinct textured model" % UNIT_MODEL_KINDS[index])
			return false
		meshes[mesh] = true
	for probe in probes:
		probe.queue_free()
	await get_tree().process_frame
	await get_tree().process_frame
	return true

func _verify_warning_presentation(main: Node3D) -> void:
	var warning_box := {"snapshot": {}}
	if not await _wait_until(func():
			var snapshot: Dictionary = GrusBridge.wave_snapshot()
			if not snapshot.get("incoming_zones", []).is_empty() \
					and float(snapshot.get("seconds_remaining", 0.0)) >= 27.0:
				warning_box["snapshot"] = snapshot
				return true
			return false,
			30.0, "warning never reached the wave presentation smoke"):
		_stage5_passed = false
		return
	await get_tree().process_frame
	await get_tree().process_frame
	var warning: Dictionary = warning_box["snapshot"]
	var zones: Array = warning.get("incoming_zones", [])
	var wave_label := main.get_node("HUD/WaveLabel") as Control
	if int(warning.get("wave", -1)) != 0 or int(warning.get("next_wave", -1)) != 1 \
			or zones.size() != 1 or str(zones[0].get("name", "")) != "North":
		_stage5_fail("warning snapshot is not the first authored zone: %s" % [warning])
		return
	if not _ring_matches(warning) or not bool(_wave_ring.warning):
		_stage5_fail("warning ring is not the amber snapshot ratio: ring=%s snapshot=%s" % [
			_wave_ring.ring_ratio, warning])
		return
	var glow := _glow_visibility()
	if not bool(glow["North"]) or bool(glow["East"]) or bool(glow["South"]) or bool(glow["West"]):
		_stage5_fail("directional edge glow does not match the North warning zone: %s" % [glow])
		return
	# HPA-698 fix round: the incoming directions own a visible readout, so the
	# full names stay readable and are also in the accessible descriptions
	# without hover. The pointer tooltip remains a second surface.
	var wave_warning := main.get_node("HUD/WaveWarning") as RichTextLabel
	var wave_card := main.get_node("HUD/WavePanel") as Control
	var live_readout := _wave_readout_error(main, ["Prep", "Next wave 1"],
		["North"], ["1"], "live single-zone warning")
	if not live_readout.is_empty():
		_stage5_fail(live_readout)
		return
	if not _rich_text_has_icon(wave_warning, "res://assets/ui/icons/status/incoming_zone.png"):
		_stage5_fail("incoming icon is not in the warning readout")
		return
	if not str(wave_card.tooltip_text).contains("Incoming: North"):
		_stage5_fail("wave card tooltip lost the full warning zone name: %s" % [
			wave_card.tooltip_text])
		return
	# Presentation-only copied snapshots: wave 20's authored three-zone
	# rotation, its cleared state, a realistic three-digit wave, the display
	# fallback boundary and the i64 index bound. The controller's per-frame
	# refresh is suspended so each probe survives its frames; the exact full
	# values must stay in the readout's accessible description. No command,
	# sim state or gameplay data changes.
	var probe_errors := PackedStringArray()
	var probes := [
		["wave-20 three-zone warning", _wave_warning_probe(warning, 19, 20,
			["West", "North", "East"]),
			["Wave 19", "Next wave 20"], ["West", "North", "East"], ["19", "20"]],
		["cleared warning", _wave_idle_probe(warning, 19, 20),
			["Wave 19", "Next wave 20"], [], ["19", "20"]],
		["three-digit wave", _wave_idle_probe(warning, 123, 124),
			["Wave 123", "Next wave 124"], [], ["123", "124"]],
		["large wave fallback", _wave_idle_probe(warning, 999999, 1000000),
			[], [], ["999999", "1000000"]],
		["i64 wave boundary", _wave_idle_probe(warning, 9223372036854775806,
			9223372036854775807), [], [],
			["9223372036854775806", "9223372036854775807"]],
	]
	main.set_process(false)
	for probe in probes:
		(main.get_node("HUD") as Node).call("refresh_wave", probe[1])
		await get_tree().process_frame
		await get_tree().process_frame
		var probe_error := _wave_readout_error(main, probe[2], probe[3], probe[4], probe[0])
		if not probe_error.is_empty():
			probe_errors.append(probe_error)
		# Opt-in evidence of the authored wave-20 three-zone readout; synthetic
		# copied snapshot, never a real wave 20.
		if str(probe[0]) == "wave-20 three-zone warning":
			var probe_capture := await SmokeHelpers.capture_frame(get_tree(),
				"wave-warning-three-zone-probe")
			if not probe_capture.is_empty():
				probe_errors.append(probe_capture)
	(main.get_node("HUD") as Node).call("refresh_wave", warning)
	main.set_process(true)
	await get_tree().process_frame
	if not probe_errors.is_empty():
		_stage5_fail(probe_errors[0])
		return
	# R1: the controller's frame order is `_refresh_session` then `_refresh_hud`
	# (so `refresh_session` then `refresh_wave`). A frozen warning must not
	# outlive the Result hide-list, and the same order must restore it while
	# Playing/Paused and clear it after a restart-shaped surface. Presentation-
	# only copied snapshots; no command, sim state or gameplay data changes.
	var session_errors := PackedStringArray()
	var frozen_warning := _wave_warning_probe(warning, 19, 20, ["West", "North", "East"])
	var hud := main.get_node("HUD") as Node
	var warning_control := main.get_node("HUD/WaveWarning") as RichTextLabel
	var readout_control := main.get_node("HUD/WaveLabel") as RichTextLabel
	main.set_process(false)
	for order_case in [["Result", 1, false], ["Playing", -1, true], ["Paused", -1, true]]:
		var order_phase := str(order_case[0])
		var want_visible: bool = order_case[2]
		hud.call("refresh_session", order_phase, int(order_case[1]))
		hud.call("refresh_wave", frozen_warning)
		await get_tree().process_frame
		await get_tree().process_frame
		if warning_control.is_visible_in_tree() != want_visible:
			session_errors.append("%s after the session→wave order: warning visible=%s, want %s" % [
				order_phase, warning_control.is_visible_in_tree(), want_visible])
		if not want_visible:
			if wave_card.is_visible_in_tree() or readout_control.is_visible_in_tree():
				session_errors.append("%s still exposes the wave card/readout" % order_phase)
			for description_owner in [readout_control, wave_card]:
				if str(description_owner.accessibility_description).to_lower().contains("incoming"):
					session_errors.append("%s kept an incoming accessible description" % order_phase)
			# Opt-in evidence of the fixed Result composition with a frozen
			# warning; synthetic presentation-only Result, never a real win.
			var result_capture := await SmokeHelpers.capture_frame(get_tree(),
				"wave-result-warning-probe")
			if not result_capture.is_empty():
				session_errors.append(result_capture)
		elif not _wave_text_fits(warning_control):
			session_errors.append("%s: restored warning names do not fit" % order_phase)
	# Restart-shaped cleared surface through the same production order.
	hud.call("refresh_session", "Playing", -1)
	hud.call("refresh_wave", _wave_idle_probe(warning, 0, 1))
	await get_tree().process_frame
	await get_tree().process_frame
	if warning_control.is_visible_in_tree() or not _visible_text(warning_control).is_empty():
		session_errors.append("restart-shaped wave surface still shows the warning")
	if str(readout_control.accessibility_description).to_lower().contains("incoming"):
		session_errors.append("restart-shaped wave surface kept an incoming description")
	# Restore the live Playing warning exactly as the next controller frame does.
	hud.call("refresh_session", "Playing", -1)
	hud.call("refresh_wave", warning)
	main.set_process(true)
	await get_tree().process_frame
	if not session_errors.is_empty():
		_stage5_fail(session_errors[0])
		return
	# Real-render evidence of the compact card's warning state; writes nothing
	# unless GRUS_CAPTURE_DIR names a directory.
	var warning_capture := await SmokeHelpers.capture_frame(get_tree(), "wave-warning")
	if not warning_capture.is_empty():
		_stage5_fail(warning_capture)
		return
	# HPA-698 access follow-up: the card clears the northwest corridor to its
	# right at the authored framing; its static tooltip must still answer the
	# real pointer, not just exist as a property string.
	var panel_hover := InputEventMouseMotion.new()
	panel_hover.position = wave_card.get_global_rect().get_center()
	get_tree().root.push_input(panel_hover)
	await get_tree().process_frame
	await get_tree().process_frame
	if get_viewport().gui_get_hovered_control() != wave_card:
		_stage5_fail("wave card tooltip surface is not the real hover target: %s" % [
			get_viewport().gui_get_hovered_control()])
		return
	var minimap := main.get_node("HUD/Minimap") as Control
	var pings: PackedVector2Array = minimap.get("incoming_ping_points")
	if pings.size() != zones.size():
		_stage5_fail("minimap ping count differs from announced zones: pings=%s zones=%s" % [pings, zones])
		return
	for index in zones.size():
		var zone: Dictionary = zones[index]
		var anchor := Vector2(float(zone.get("anchor_x", -1)), float(zone.get("anchor_y", -1)))
		if not pings[index].is_equal_approx(anchor):
			_stage5_fail("minimap ping did not use the fixture anchor: got=%s expected=%s" % [pings[index], anchor])
			return
	if int(warning.get("live_aliens", -1)) != 0 or int(minimap.get("enemy_marker_count")) != 0:
		_stage5_fail("public zone ping was conflated with individual enemy markers: snapshot=%s" % [warning])
		return
	if not await _wait_until(func():
			var snapshot: Dictionary = GrusBridge.wave_snapshot()
			return float(snapshot.get("seconds_remaining", 30.0)) < 24.0 \
					and not snapshot.get("incoming_zones", []).is_empty(),
			6.0, "wave border ping never expired inside its warning window"):
		_stage5_passed = false
		return
	await get_tree().process_frame
	await get_tree().process_frame
	var expired_pings: PackedVector2Array = minimap.get("incoming_ping_points")
	if not expired_pings.is_empty() \
			or not _visible_text(wave_warning).contains("North") \
			or not _rich_text_has_icon(wave_warning, "res://assets/ui/icons/status/incoming_zone.png"):
		_stage5_fail("short-lived border ping did not expire while the authoritative HUD warning remained")
		return
	var still_warning: Dictionary = GrusBridge.wave_snapshot()
	if not bool(_wave_ring.warning) or not _ring_matches(still_warning) or not bool(_glow_visibility()["North"]):
		_stage5_fail("amber ring/edge glow dropped while incoming_zones still announced North: %s" % [still_warning])
		return
	_stage5_passed = true

func _success_marker() -> String:
	if not _stage5_passed:
		return ""
	return "GRUS_WAVE_SMOKE_OK start_pause_clock_frozen=true warning_hud=true snapshot_anchor_pings=true fog_markers_gated=true role_probes=white_box journey=kill_crystal_defeat_restart"

func _stage5_fail(message: String) -> void:
	_stage5_passed = false
	_fail("wave presentation smoke: " + message)

func _has_property(object: Object, property_name: String) -> bool:
	for property in object.get_property_list():
		if str(property.get("name", "")) == property_name:
			return true
	return false
