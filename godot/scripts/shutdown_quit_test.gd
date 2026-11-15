extends Node

## Focused shutdown contract: the best-effort audio drain branches, the normal
## Start/Pause/Resume/Restart lifecycle, and controller guards while a quit is
## pending. The process exits through the controller's real request_quit()
## drain, so a clean exit also proves the success path still tears down.

const MARKER := "GRUS_SHUTDOWN_QUIT_OK"
const MAIN := "Main"

var _main: Node3D
var _fx: Node3D
var _failures: Array[String] = []

func _ready() -> void:
	call_deferred("_run")

func _note(ok: bool, message: String) -> bool:
	if not ok:
		_failures.append(message)
		push_error(message)
	return ok

func _phase() -> String:
	return str(GrusBridge.session_snapshot().get("phase", ""))

func _wait_phase(phase: String, seconds := 5.0) -> bool:
	var deadline := Time.get_ticks_msec() + int(seconds * 1000.0)
	while Time.get_ticks_msec() < deadline:
		if _phase() == phase:
			return true
		await get_tree().process_frame
	return false

func _wait_until(predicate: Callable, seconds := 5.0) -> bool:
	var deadline := Time.get_ticks_msec() + int(seconds * 1000.0)
	while Time.get_ticks_msec() < deadline:
		if predicate.call():
			return true
		await get_tree().process_frame
	return bool(predicate.call())

func _button(path: String) -> Button:
	return _main.get_node(path) as Button

func _tile(grid_path: String, kind: String) -> Button:
	for child in _main.get_node(grid_path).get_children():
		if child is Button and str(child.get_meta("kind", "")) == kind:
			return child as Button
	return null

## Scripted drain checks. `stop_and_drain` takes an injectable clock (and a
## short timeout) so the callback-credit, missed-observation and timeout
## branches are testable without a real stalled device.
func _assert_drain_branches() -> void:
	# Small callbacks: ~128-frame credits never reach proof in two resets, so
	# the drain must keep observing until the credited frames cover the target.
	var calls := [0]
	var small := func() -> float:
		calls[0] += 1
		if calls[0] == 1:
			return 0.020
		return [0.0028, 0.0027, 0.0009][(calls[0] - 2) % 3]
	# Generous bound for this leg: the first windowed frames can stall on
	# renderer warm-up well past the production deadline.
	var observed = await _fx.stop_and_drain(small, 2000)
	_note(observed, "small-callback drain did not reach the target")
	_note(calls[0] >= 12, "small-callback drain stopped after %d clock reads; it must not accept two callbacks as proof" % calls[0])

	# Missed observations: a clock that only ever increases (sampling never
	# catches a reset) must end at the deadline, not pretend to drain.
	var rising := [0]
	var rising_clock := func() -> float:
		rising[0] += 1
		return 0.0001 * rising[0]
	var start := Time.get_ticks_msec()
	var missed = await _fx.stop_and_drain(rising_clock, 40)
	var missed_ms := Time.get_ticks_msec() - start
	_note(not missed, "a never-decreasing clock was accepted as drained")
	_note(missed_ms >= 40, "missed-observation drain returned before its deadline: %d ms" % missed_ms)

	# Stalled clock: no reset at all must also end at the deadline.
	start = Time.get_ticks_msec()
	var stalled = await _fx.stop_and_drain(func() -> float: return 0.010, 40)
	var stalled_ms := Time.get_ticks_msec() - start
	_note(not stalled, "a constant clock was accepted as drained")
	_note(stalled_ms >= 40, "stalled drain returned before its deadline: %d ms" % stalled_ms)

func _run() -> void:
	_main = get_node(MAIN) as Node3D
	_fx = _main.get_node("CombatFx") as Node3D
	for _frame in 5:
		await get_tree().process_frame

	await _assert_drain_branches()

	# Normal startup: no match yet, so a build tile must not arm placement.
	_note(_phase() != "Playing", "the fixture starts in Playing (phase %s)" % _phase())
	_note(not _main.call("_playing"), "the controller reports Playing before Start")
	var barracks := _tile("HUD/CommandPanel/BuildGrid", "Barracks")
	if _note(barracks != null, "the Barracks build tile is missing"):
		barracks.pressed.emit()
		_note(str(_main.get("_placement_kind")) == "", "a build tile armed placement before Start")

	# Normal session controls still work before the quit is committed.
	_note(await _wait_phase("Start"), "restart/start fixture did not report Start")
	_button("HUD/SessionPanel/StartButton").pressed.emit()
	if _note(await _wait_phase("Playing"), "Start did not enter Playing"):
		_note(await _wait_until(func() -> bool: return bool(_main.call("_playing"))),
			"the controller does not report Playing after Start")
	_button("HUD/PauseButton").pressed.emit()
	_note(await _wait_phase("Paused"), "Pause did not enter Paused")
	_button("HUD/SessionPanel/ResumeButton").pressed.emit()
	_note(await _wait_phase("Playing"), "Resume did not return to Playing")

	# Normal restart still reseeds the fixture and returns to Start.
	_button("HUD/SessionPanel/RestartButton").pressed.emit()
	_note(await _wait_phase("Start"), "Restart did not return to Start")
	var reseeded := func() -> bool:
		return get_tree().get_nodes_in_group("unit_views").size() == 4 \
			and get_tree().get_nodes_in_group("building_views").size() == 1
	_note(await _wait_until(reseeded),
		"Restart did not reseed four worker views and one building view")
	_button("HUD/SessionPanel/StartButton").pressed.emit()
	_note(await _wait_phase("Playing"), "Start after Restart did not enter Playing")
	_note(await _wait_until(func() -> bool: return bool(_main.call("_playing"))),
		"the controller does not report Playing after the second Start")

	# Pending quit: freeze the Playing session through the Rust pause seam and
	# reject every controller mutation until the drain ends.
	var units_before := get_tree().get_nodes_in_group("unit_views").size()
	var buildings_before := get_tree().get_nodes_in_group("building_views").size()
	_main.notification(Node.NOTIFICATION_WM_CLOSE_REQUEST)
	_note(bool(_main.get("_shutting_down")), "the close request did not enter pending shutdown")
	_note(not _main.call("_playing"), "pending shutdown still reports Playing")
	_note(_phase() == "Paused", "pending shutdown did not pause the Playing session (phase %s)" % _phase())

	var frozen_phase := _phase()
	_button("HUD/SessionPanel/RestartButton").pressed.emit()
	_note(_phase() == frozen_phase, "Restart changed the phase during pending shutdown: %s" % _phase())
	_button("HUD/SessionPanel/StartButton").pressed.emit()
	_note(_phase() == frozen_phase, "Start changed the phase during pending shutdown: %s" % _phase())
	_button("HUD/SessionPanel/ResumeButton").pressed.emit()
	_note(_phase() == frozen_phase, "Resume changed the phase during pending shutdown: %s" % _phase())
	_button("HUD/PauseButton").pressed.emit()
	_note(_phase() == frozen_phase, "Pause changed the phase during pending shutdown: %s" % _phase())
	_note(get_tree().get_nodes_in_group("unit_views").size() == units_before,
		"a session button reseeded unit views during pending shutdown")
	_note(get_tree().get_nodes_in_group("building_views").size() == buildings_before,
		"a session button reseeded building views during pending shutdown")

	if barracks != null:
		barracks.pressed.emit()
		_note(str(_main.get("_placement_kind")) == "",
			"a build tile armed placement during pending shutdown")
	_main.call("_on_analyze_pressed")
	_note(not _main.call("_playing"), "an Analyze attempt re-enabled Playing")

	var drawer := _main.get_node("HUD/UndergroundPanel") as Control
	var drawer_visible := drawer.visible
	var tab := InputEventKey.new()
	tab.keycode = KEY_TAB
	tab.pressed = true
	_main._input(tab)
	_note(drawer.visible == drawer_visible, "Tab toggled the drawer during pending shutdown")

	_main.notification(Node.NOTIFICATION_WM_CLOSE_REQUEST)
	_note(bool(_main.get("_shutting_down")), "a second close cleared pending shutdown")

	if _failures.is_empty():
		print(MARKER)
		return
	print("GRUS_SHUTDOWN_QUIT_FAILED: " + " | ".join(_failures))
	get_tree().quit(1)
