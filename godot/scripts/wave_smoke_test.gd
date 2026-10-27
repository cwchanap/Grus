extends "res://scripts/combat_lifecycle_smoke_test.gd"

## Stage 5 presentation checks layered over the real survival journey. The
## parent smoke owns all gameplay; this scene checks its HUD/minimap rendering.

const STAGE5_ALIEN_KINDS := ["Swarmer", "Spitter", "Brute", "Behemoth"]
const UNIT_VIEW_SCENE := preload("res://scenes/unit_view.tscn")

var _stage5_passed := false

func _run() -> void:
	await get_tree().process_frame
	var main := get_node_or_null("Main") as Node3D
	if main == null:
		_stage5_fail("main scene is missing")
		return
	var start_label := main.get_node_or_null("HUD/SessionPanel/SessionLabel") as Label
	if start_label == null or not start_label.text.to_lower().contains("surviv"):
		_stage5_fail("Start overlay does not describe survival: %s" % [start_label.text if start_label != null else "missing"])
		return
	var wave_label := main.get_node_or_null("HUD/WaveLabel") as Label
	if wave_label == null:
		_stage5_fail("wave HUD label is missing")
		return
	# The countdown must be rendered from a live bridge read, not the
	# scene's placeholder text: poll until the label shows the snapshot's
	# next_wave, then require the snapshot's own remaining time on screen.
	var live_wave: Dictionary = {}
	for _frame in 8:
		live_wave = GrusBridge.wave_snapshot()
		if wave_label.text.contains("Next wave %d" % int(live_wave.get("next_wave", -1))):
			break
		await get_tree().process_frame
	var next_wave := int(live_wave.get("next_wave", -1))
	var remaining := maxi(ceili(float(live_wave.get("seconds_remaining", 0.0))), 0)
	var expected_time := "%d:%02d" % [int(remaining / 60), remaining % 60]
	if not wave_label.text.contains("Next wave %d" % next_wave) \
			or not wave_label.text.contains(expected_time):
		_stage5_fail("wave HUD does not render the live snapshot countdown (want 'Next wave %d' + '%s'): %s" % [next_wave, expected_time, wave_label.text])
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
	# White-box presentation probes exercise all four primitive silhouettes;
	# the real wave journey below still uses bridge-authored unit metadata.
	var probes: Array[Node3D] = []
	for index in STAGE5_ALIEN_KINDS.size():
		var probe := UNIT_VIEW_SCENE.instantiate() as Node3D
		probe.name = "RoleProbe%d" % index
		probe.visible = true
		probe.set_meta("unit_id", 900 + index)
		probe.set_meta("team_id", 2)
		probe.set_meta("unit_kind", STAGE5_ALIEN_KINDS[index])
		add_child(probe)
		probes.append(probe)
	await get_tree().process_frame
	await get_tree().process_frame

	var signatures := {}
	for index in probes.size():
		var probe := probes[index]
		var body := probe.get_node("Body") as MeshInstance3D
		var marker := probe.get_node("RoleMarker") as Node3D
		var shapes := PackedStringArray()
		for child in marker.get_children():
			var marker_mesh := child as MeshInstance3D
			if marker_mesh != null and marker_mesh.mesh != null:
				shapes.append(marker_mesh.mesh.get_class())
		shapes.sort()
		var signature := "%.2f:%s" % [body.scale.x, ",".join(shapes)]
		if shapes.is_empty() or signatures.has(signature):
			for old_probe in probes:
				old_probe.queue_free()
			_stage5_fail("alien role %s has no unique primitive silhouette (%s)" % [STAGE5_ALIEN_KINDS[index], signature])
			return false
		signatures[signature] = true
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
	var wave_label := main.get_node("HUD/WaveLabel") as Label
	if int(warning.get("wave", -1)) != 0 or int(warning.get("next_wave", -1)) != 1 \
			or zones.size() != 1 or str(zones[0].get("name", "")) != "North":
		_stage5_fail("warning snapshot is not the first authored zone: %s" % [warning])
		return
	if not wave_label.text.contains("Prep") or not wave_label.text.contains("Next wave 1") \
			or not wave_label.text.contains("Incoming: North"):
		_stage5_fail("HUD does not render the live warning snapshot: %s" % [wave_label.text])
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
			or not wave_label.text.contains("Incoming: North"):
		_stage5_fail("short-lived border ping did not expire while the authoritative HUD warning remained")
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
