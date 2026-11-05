extends "res://scripts/benchmark_200.gd"

var _captured_failure := ""

func _ready() -> void:
	call_deferred("_run_liveness_regressions")

func _fail(message: String) -> void:
	_captured_failure = message

func _regression_fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

func _probe(group: String, meta: Dictionary) -> Node3D:
	var probe := Node3D.new()
	add_child(probe)
	probe.add_to_group(group)
	for key in meta:
		probe.set_meta(key, meta[key])
	return probe

func _quiet_verdict(frame: int) -> bool:
	_last_combat_event_total = int(GrusBridge.combat_event_total())
	return _liveness_verdict(0, frame)

func _run_liveness_regressions() -> void:
	var alien := _probe("unit_views", {"unit_id": 9001, "team_id": 2, "unit_kind": "Swarmer"})
	var turret := _probe("building_views", {"building_id": 9001, "team_id": 1, "building_kind": "GunTurret"})

	_captured_failure = ""
	if _quiet_verdict(20) or _captured_failure.is_empty():
		_regression_fail("liveness verdict passed a quiet window contested only by a surviving team-1 building")
		return

	var defender := _probe("unit_views", {"unit_id": 9002, "team_id": 1, "unit_kind": "Guard"})
	_captured_failure = ""
	if _quiet_verdict(40) or _captured_failure.is_empty():
		_regression_fail("liveness verdict passed a quiet window contested by live units")
		return

	turret.queue_free()
	defender.queue_free()
	await get_tree().process_frame
	await get_tree().process_frame
	_captured_failure = ""
	if not _quiet_verdict(60) or not _captured_failure.is_empty():
		_regression_fail("liveness verdict did not pass cleanly once team 1 was fully extinct")
		return

	var mover := _probe("unit_views", {"unit_id": 9003, "team_id": 1, "unit_kind": "Rifleman"})
	_captured_failure = ""
	if not _liveness_verdict(3, 80) or not _captured_failure.is_empty():
		_regression_fail("liveness verdict failed a contested window that showed movement")
		return
	mover.queue_free()
	alien.queue_free()

	print("GRUS_BENCHMARK_LIVENESS_SMOKE_OK building_presence=fails_quiet unit_presence=fails_quiet extinct=passes_quiet movement=passes")
	get_tree().quit(0)
