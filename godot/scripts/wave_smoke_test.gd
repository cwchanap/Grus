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
	_wave_ring = main.get_node_or_null("HUD/WaveRing")
	_incoming_glow = main.get_node_or_null("HUD/IncomingGlow")
	if _wave_ring == null or _incoming_glow == null:
		_stage5_fail("wave countdown ring or incoming edge glow is missing")
		return
	if _incoming_glow.mouse_filter != Control.MOUSE_FILTER_IGNORE:
		_stage5_fail("incoming edge glow is interactive instead of mouse IGNORE")
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
	if _rich_text_has_icon(wave_label, "res://assets/ui/icons/status/incoming_zone.png"):
		_stage5_fail("wave HUD shows incoming-zone art before a warning")
		return
	if not start_text.contains("Next wave %d" % next_wave) \
			or not start_text.contains(expected_time) or not _wave_hud_fits(wave_label):
		_stage5_fail("wave HUD does not render the live snapshot countdown without clipping (want 'Next wave %d' + '%s'): %s" % [next_wave, expected_time, start_text])
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
	var warning_text := _visible_text(wave_label)
	if not warning_text.contains("Prep") or not warning_text.contains("Next wave 1") \
			or not warning_text.contains("Incoming: North") \
			or not _rich_text_has_icon(wave_label, "res://assets/ui/icons/status/incoming_zone.png") \
			or not _wave_hud_fits(wave_label):
		_stage5_fail("HUD does not render the live warning snapshot and incoming icon without clipping: %s" % [warning_text])
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
			or not _visible_text(wave_label).contains("Incoming: North") \
			or not _rich_text_has_icon(wave_label, "res://assets/ui/icons/status/incoming_zone.png"):
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
