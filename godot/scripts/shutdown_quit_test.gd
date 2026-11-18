extends Node

## Focused shutdown contract: the best-effort audio drain branches, the normal
## Start/Pause/Resume/Restart lifecycle, and controller/input guards while a
## quit is pending. The process exits through the controller's real
## request_quit() drain, so a clean exit also proves the success path still
## tears down.
##
## Modes (user args after `--`):
##   (none)        full journey
##   stale_start   Start then close in the same frame (stale phase cache)
##   stale_resume  Resume then close in the same frame (stale phase cache)
##   close_start   close before Start
##   close_paused  close while Paused

const MARKER := "GRUS_SHUTDOWN_QUIT_OK"
const MAIN := "Main"
const SmokeHelpers := preload("res://scripts/smoke_helpers.gd")

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

func _playing() -> bool:
	return bool(_main.call("_playing"))

func _button(path: String) -> Button:
	return _main.get_node(path) as Button

func _tile(grid_path: String, kind: String) -> Button:
	for child in _main.get_node(grid_path).get_children():
		if child is Button and str(child.get_meta("kind", "")) == kind:
			return child as Button
	return null

func _report(label: String) -> void:
	if _failures.is_empty():
		print(MARKER)
		return
	print("GRUS_SHUTDOWN_QUIT_FAILED [%s]: %s" % [label, " | ".join(_failures)])
	get_tree().quit(1)

## Closes and checks the authoritative phase once, synchronously: no frame is
## allowed to run between the triggering action and this assertion.
func _close_and_check(label: String, expected_phase: String) -> void:
	_main.notification(Node.NOTIFICATION_WM_CLOSE_REQUEST)
	_note(bool(_main.get("_shutting_down")), "%s: close did not enter pending shutdown" % label)
	_note(not _playing(), "%s: pending shutdown still reports Playing" % label)
	_note(_phase() == expected_phase,
		"%s: phase %s after close, expected %s" % [label, _phase(), expected_phase])

## Scripted drain checks. `stop_and_drain` takes an injectable clock, timeout
## and mix rate so the callback-credit, missed-observation, late-deadline and
## timeout branches are testable without a real device. Callback ages are
## expressed in frames divided by the rate, so the fixture is independent of
## the machine's sample rate.
func _assert_drain_branches() -> void:
	# ~128-frame callbacks: the credit must need well over two resets, and the
	# read count must not change with the sample rate.
	for rate in [44100.0, 48000.0, 96000.0]:
		var calls := [0]
		var local_rate: float = rate
		var clock := func() -> float:
			calls[0] += 1
			if calls[0] == 1:
				return 0.05
			return [110.0, 105.0, 40.0][(calls[0] - 2) % 3] / local_rate
		var observed = await _fx.stop_and_drain(clock, 2000, rate)
		_note(bool(observed), "small-callback drain did not reach the target at %d Hz" % int(rate))
		_note(calls[0] >= 14 and calls[0] <= 18,
			"small-callback drain took %d clock reads at %d Hz, expected 14-18" % [calls[0], int(rate)])

	# Larger per-callback ages reach the frame target in fewer reads: the target
	# is frames, not wall-clock seconds.
	for rate in [44100.0, 96000.0]:
		var calls := [0]
		var local_rate: float = rate
		var clock := func() -> float:
			calls[0] += 1
			if calls[0] == 1:
				return 0.05
			return [700.0, 400.0][(calls[0] - 2) % 2] / local_rate
		var observed = await _fx.stop_and_drain(clock, 2000, rate)
		_note(bool(observed) and calls[0] <= 6,
			"large-frame drain took %d clock reads at %d Hz, expected <= 6" % [calls[0], int(rate)])

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

	# Deadline after the await: the target must not be reported once the
	# resumed iteration is past the deadline.
	var slow := [0]
	var slow_clock := func() -> float:
		slow[0] += 1
		if slow[0] == 1:
			return 0.050
		if slow[0] == 2:
			return 0.030
		OS.delay_msec(25)
		return 0.001
	start = Time.get_ticks_msec()
	var late = await _fx.stop_and_drain(slow_clock, 10, 44100.0)
	var late_ms := Time.get_ticks_msec() - start
	_note(not late, "drain reported success on an iteration that ran past its deadline")
	_note(late_ms >= 10, "late-deadline drain returned before its deadline: %d ms" % late_ms)

func _run_mode(mode: String) -> void:
	match mode:
		"close_start":
			_note(_phase() == "Start", "close_start: fixture is not in Start")
			_close_and_check(mode, "Start")
			_report(mode)
		"close_paused":
			_note(_phase() == "Start", "close_paused: fixture is not in Start")
			_button("HUD/SessionPanel/StartButton").pressed.emit()
			_note(await _wait_phase("Playing"), "close_paused: Start did not enter Playing")
			_note(await _wait_until(_playing), "close_paused: controller did not report Playing")
			_button("HUD/PauseButton").pressed.emit()
			_note(await _wait_phase("Paused"), "close_paused: Pause did not enter Paused")
			_note(await _wait_until(func() -> bool: return not _playing()),
				"close_paused: controller kept reporting Playing")
			_close_and_check(mode, "Paused")
			_report(mode)
		"stale_start":
			_note(_phase() == "Start", "stale_start: fixture is not in Start")
			_note(str(_main.get("_session_phase")) == "Start",
				"stale_start: controller cache was not Start before the action")
			_button("HUD/SessionPanel/StartButton").pressed.emit()
			_note(_phase() == "Playing", "stale_start: Start did not reach Rust Playing")
			_note(str(_main.get("_session_phase")) != "Playing",
				"stale_start: the controller cache refreshed before the close")
			_close_and_check(mode, "Paused")
			_report(mode)
		"stale_resume":
			_note(_phase() == "Start", "stale_resume: fixture is not in Start")
			_button("HUD/SessionPanel/StartButton").pressed.emit()
			_note(await _wait_phase("Playing"), "stale_resume: Start did not enter Playing")
			_note(await _wait_until(_playing), "stale_resume: controller did not report Playing")
			_button("HUD/PauseButton").pressed.emit()
			_note(await _wait_phase("Paused"), "stale_resume: Pause did not enter Paused")
			_note(await _wait_until(func() -> bool: return not _playing()),
				"stale_resume: controller kept reporting Playing")
			_button("HUD/SessionPanel/ResumeButton").pressed.emit()
			_note(_phase() == "Playing", "stale_resume: Resume did not reach Rust Playing")
			_note(str(_main.get("_session_phase")) == "Paused",
				"stale_resume: the controller cache refreshed before the close")
			_close_and_check(mode, "Paused")
			_report(mode)
		_:
			push_error("unknown shutdown test mode '%s'" % mode)
			get_tree().quit(1)

func _run() -> void:
	var args := OS.get_cmdline_user_args()
	var mode := str(args[0]) if args.size() > 0 else ""
	_main = get_node(MAIN) as Node3D
	_fx = _main.get_node("CombatFx") as Node3D
	for _frame in 5:
		await get_tree().process_frame

	if not mode.is_empty():
		await _run_mode(mode)
		return

	await _assert_drain_branches()

	# Normal startup: no match yet, so a build tile must not arm placement.
	_note(_phase() != "Playing", "the fixture starts in Playing (phase %s)" % _phase())
	_note(not _playing(), "the controller reports Playing before Start")
	var barracks := _tile("HUD/CommandPanel/BuildGrid", "Barracks")
	if _note(barracks != null, "the Barracks build tile is missing"):
		barracks.pressed.emit()
		_note(str(_main.get("_placement_kind")) == "", "a build tile armed placement before Start")

	# Normal session controls still work before the quit is committed.
	_note(await _wait_phase("Start"), "restart/start fixture did not report Start")
	_button("HUD/SessionPanel/StartButton").pressed.emit()
	if _note(await _wait_phase("Playing"), "Start did not enter Playing"):
		_note(await _wait_until(_playing), "the controller does not report Playing after Start")
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
	_note(await _wait_until(_playing),
		"the controller does not report Playing after the second Start")

	# Pending quit: freeze the Playing session through the Rust pause seam and
	# reject every controller/GUI action until the drain ends.
	var units_before := get_tree().get_nodes_in_group("unit_views").size()
	var buildings_before := get_tree().get_nodes_in_group("building_views").size()
	_main.notification(Node.NOTIFICATION_WM_CLOSE_REQUEST)
	_note(bool(_main.get("_shutting_down")), "the close request did not enter pending shutdown")
	_note(not _playing(), "pending shutdown still reports Playing")
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
	# Clearing first proves the rejection text came from this press: an
	# earlier "Match is not playing" leftover cannot satisfy the check, and a
	# missing guard would leave a different status behind instead.
	var analyze_status := _main.get_node("HUD/CommandStatus") as Label
	analyze_status.text = ""
	_main.call("_on_analyze_pressed")
	_note(not _playing(), "an Analyze attempt re-enabled Playing")
	_note(analyze_status.text == "Match is not playing",
		"an Analyze attempt during pending shutdown did not report rejection (status %s)" % analyze_status.text)

	var drawer := _main.get_node("HUD/UndergroundPanel") as Control
	var drawer_visible := drawer.visible
	var tab := InputEventKey.new()
	tab.keycode = KEY_TAB
	tab.pressed = true
	_main._input(tab)
	_note(drawer.visible == drawer_visible, "Tab toggled the drawer during pending shutdown")

	# A real GUI click must not reach the minimap either: the controller-style
	# input consumption has to stop the event before the Control's `_gui_input`.
	var minimap := _main.get_node("HUD/Minimap") as Control
	var camera := _main.get_node("Camera3D") as Camera3D
	var camera_before := camera.position
	SmokeHelpers.push_mouse_click(get_tree(), minimap.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	_note(camera.position.is_equal_approx(camera_before),
		"a minimap click moved the camera during pending shutdown")

	_main.notification(Node.NOTIFICATION_WM_CLOSE_REQUEST)
	_note(bool(_main.get("_shutting_down")), "a second close cleared pending shutdown")

	_report("default")
