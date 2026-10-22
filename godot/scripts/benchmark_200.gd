extends Node

## HPA-474 mixed-workload benchmark. After a benchmark reset it waits for the
## full 200-unit load, waits for the fog revision to advance so the sim is provably
## ticking, then records one frame-time sample per rendered frame plus
## live and visible-in-tree unit counts sampled once per liveness window for a
## real-time duration. A liveness checkpoint between fixed frame intervals fails a
## sample that shows neither unit movement nor combat activity. Real combat
## deaths are expected: units are never respawned and health is never reset.

const BENCHMARK_SIZE := Vector2i(1920, 1080)
const DEFAULT_SAMPLE_SECONDS := 10.0
const WARMUP_SECONDS := 2.0
const VIEW_WAIT_FRAMES := 180
const VISIBILITY_WAIT_FRAMES := 600
const MOVEMENT_CHECK_INTERVAL := 20
const MOVEMENT_EPSILON := 0.05

var _last_combat_event_total := 0
## Live-unit and visible-in-tree samples, one of each per liveness window —
## the per-frame group scans these replaced would land in every measured
## frame time. `_sample_window_seconds` holds each window's measured
## duration so the emitted means are elapsed-time weighted — a trailing
## partial window must not count like a full one.
var _live_unit_samples: Array[int] = []
var _visible_in_tree_samples: Array[int] = []
var _sample_window_seconds: Array[float] = []
var _last_sample_ticks := 0

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

func _progress(stage: String, frames := 0) -> void:
	print("GRUS_BENCHMARK_STAGE stage=%s frames=%d elapsed_ms=%d" % [stage, frames, Time.get_ticks_msec()])

func _percentile(sorted_samples: Array[float], fraction: float) -> float:
	var index := clampi(int(ceil((sorted_samples.size() - 1) * fraction)), 0, sorted_samples.size() - 1)
	return sorted_samples[index]

## Sample duration: `GRUS_BENCHMARK_SECONDS`, or the 10 s default when the
## variable is unset. Any present value — including an explicitly empty
## one — must be a complete positive finite number: `to_float()` alone would accept a
## leading number and silently drop trailing garbage like "10seconds", and
## silently sampling a different duration than requested would corrupt the
## measurement contract.
func _read_sample_seconds() -> float:
	if not OS.has_environment("GRUS_BENCHMARK_SECONDS"):
		return DEFAULT_SAMPLE_SECONDS
	var raw := OS.get_environment("GRUS_BENCHMARK_SECONDS")
	if not raw.is_valid_float():
		_fail("GRUS_BENCHMARK_SECONDS must be a positive finite number of seconds, got '%s'" % raw)
		return -1.0
	var seconds := raw.to_float()
	if not is_finite(seconds) or seconds <= 0.0:
		_fail("GRUS_BENCHMARK_SECONDS must be a positive finite number of seconds, got '%s'" % raw)
		return -1.0
	return seconds

## Unit views that moved more than MOVEMENT_EPSILON since `baseline`. The
## scan walks the live group only: a view freed since the baseline was
## taken (real combat deaths in this workload) is simply absent, never a
## 'previously freed' access. With `rebase` the baseline adopts the current
## positions for the next window.
func _movement_since(baseline: Dictionary, rebase: bool) -> int:
	var moved := 0
	for unit in get_tree().get_nodes_in_group("unit_views"):
		var id := int(unit.get_meta("unit_id", -1))
		if id <= 0:
			continue
		var now: Vector3 = (unit as Node3D).global_position
		if baseline.has(id) and now.distance_to(baseline[id]) > MOVEMENT_EPSILON:
			moved += 1
		if rebase:
			baseline[id] = now
	return moved

## Live unit views right now: dead views leave the group entirely while
## fog-hidden enemies stay in it with `visible = false`, so membership is
## the live count — the visible-in-tree subset is the render-load proxy,
## not this one.
func _live_unit_count() -> int:
	return get_tree().get_nodes_in_group("unit_views").size()

## Unit views flagged visible in the scene tree right now: fog-hidden
## enemies stay in the group with `visible = false` and are never submitted
## to the renderer, so they do not count as measured load. This is the
## visibility flag only, not a drawn count — cull-mask/layer filtering and
## frustum culling are not accounted for.
func _visible_in_tree_count() -> int:
	var count := 0
	for unit in get_tree().get_nodes_in_group("unit_views"):
		if unit.is_visible_in_tree():
			count += 1
	return count

## One liveness verdict for a measured window: diffs the bridge's
## monotonic combat-event total and fails when the window shows neither
## meaningful movement nor combat activity — a rendering-alive but
## simulation-dead load. The counter is non-consuming: the instanced main
## scene's battlefield controller already drains `drain_combat_events()`
## every rendered frame for cosmetics (a missing session reads as
## Playing), so a second drain here would always see an empty buffer and
## would steal cosmetic events.
func _liveness_verdict(moved: int, window_end_frame: int) -> bool:
	var events := int(GrusBridge.combat_event_total())
	if moved == 0 and events <= _last_combat_event_total:
		_fail("benchmark liveness window ending at frame %d showed neither unit movement nor combat activity" % window_end_frame)
		return false
	_last_combat_event_total = events
	_progress("active", moved)
	return true

## Appends one live and one visible-in-tree sample for the liveness window
## ending at `now_ticks`, storing the measured window duration alongside so
## each count weighs into the means by elapsed time.
func _record_unit_samples(now_ticks: int) -> void:
	_sample_window_seconds.append(float(now_ticks - _last_sample_ticks) / 1_000_000.0)
	_last_sample_ticks = now_ticks
	_live_unit_samples.append(_live_unit_count())
	_visible_in_tree_samples.append(_visible_in_tree_count())

## One full liveness window: records the live and visible-in-tree samples,
## diffs positions against the checkpoint and rebases it for the next
## window.
func _liveness_checkpoint(checkpoint_positions: Dictionary, window_end_frame: int, now_ticks: int) -> bool:
	_record_unit_samples(now_ticks)
	return _liveness_verdict(_movement_since(checkpoint_positions, true), window_end_frame)

func _run() -> void:
	var sample_seconds := _read_sample_seconds()
	if sample_seconds <= 0.0:
		return

	var window := get_window()
	window.content_scale_size = BENCHMARK_SIZE
	window.size = BENCHMARK_SIZE
	await get_tree().process_frame
	_progress("viewport")

	if get_viewport().get_visible_rect().size != Vector2(BENCHMARK_SIZE):
		_fail("benchmark viewport did not resize to 1920x1080")
		return

	if not GrusBridge.has_method("reset_benchmark_fixture"):
		_fail("benchmark reset bridge is missing")
		return
	if not GrusBridge.has_method("combat_event_total"):
		_fail("benchmark combat-activity counter is missing")
		return
	if not GrusBridge.reset_benchmark_fixture():
		_fail("benchmark reset bridge rejected the benchmark fixture")
		return
	_progress("benchmark_reset")

	# Start shape: exactly 200 unique unit views; exactly the authored 2
	# buildings; at least the authored 18 resource sources. The workload
	# itself was already accepted during the benchmark reset — there is no
	# separate start call.
	var units: Array[Node] = []
	var unique_ids: Dictionary = {}
	for _frame in range(VIEW_WAIT_FRAMES):
		await get_tree().physics_frame
		units = get_tree().get_nodes_in_group("unit_views")
		unique_ids.clear()
		for unit in units:
			var id := int(unit.get_meta("unit_id", -1))
			if id > 0:
				unique_ids[id] = true
		if units.size() == 200 and unique_ids.size() == 200 \
				and get_tree().get_nodes_in_group("building_views").size() == 2 \
				and get_tree().get_nodes_in_group("resource_views").size() >= 18:
			break
	if units.size() != 200 or unique_ids.size() != 200:
		_fail("expected 200 unique unit views, found %d nodes / %d ids" % [units.size(), unique_ids.size()])
		return
	if get_tree().get_nodes_in_group("building_views").size() != 2:
		_fail("benchmark fixture must show exactly 2 building views, found %d" % get_tree().get_nodes_in_group("building_views").size())
		return
	if get_tree().get_nodes_in_group("resource_views").size() < 18:
		_fail("benchmark fixture must show at least 18 resource views, found %d" % get_tree().get_nodes_in_group("resource_views").size())
		return
	_progress("unit_views")

	# The visibility revision must advance before timing: a frozen sim never
	# re-fogs, so timing a stuck world would measure a dead render loop.
	var revision_baseline: int = GrusBridge.visibility_revision()
	var revision: int = revision_baseline
	for _frame in range(VISIBILITY_WAIT_FRAMES):
		await get_tree().process_frame
		revision = GrusBridge.visibility_revision()
		if revision > revision_baseline:
			break
	if revision <= revision_baseline:
		_fail("visibility revision never advanced past %d" % revision_baseline)
		return
	_progress("visibility", revision)

	# Live and visible-in-tree counts must be able to disagree: fog has
	# provably ticked (revision advanced), no combat deaths have landed
	# yet, and the enemy army still sits under fog — so equality here
	# means a "live" metric is really counting visible views again.
	var start_live := _live_unit_count()
	var start_visible_in_tree := _visible_in_tree_count()
	if start_visible_in_tree >= start_live:
		_fail("expected fog-hidden live units after the first fog pass, got live=%d visible_in_tree=%d" % [start_live, start_visible_in_tree])
		return

	var warmup_deadline := Time.get_ticks_usec() + int(roundf(WARMUP_SECONDS * 1_000_000.0))
	while Time.get_ticks_usec() < warmup_deadline:
		await get_tree().process_frame
	_progress("warmup")

	# Checkpoint baseline at the start of the sample so each liveness window
	# compares against positions from inside the measured load. Rebuilt
	# from the live group: the `units` array was captured before the fog
	# wait and the warm-up, so views freed since (real combat deaths) are
	# absent here instead of faulting on access.
	var checkpoint_positions: Dictionary = {}
	for unit in get_tree().get_nodes_in_group("unit_views"):
		var id := int(unit.get_meta("unit_id", -1))
		if id > 0:
			checkpoint_positions[id] = (unit as Node3D).global_position
	# One baseline behind the checkpoint, so a trailing partial window
	# still diffs a full measurement window instead of a handful of frames.
	var older_checkpoint_positions := checkpoint_positions.duplicate()
	_last_combat_event_total = int(GrusBridge.combat_event_total())

	var samples: Array[float] = []
	var start_ticks := Time.get_ticks_usec()
	_last_sample_ticks = start_ticks
	var deadline := start_ticks + int(roundf(sample_seconds * 1_000_000.0))
	var previous_ticks := start_ticks
	var frame := 0
	while true:
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		samples.append(float(now - previous_ticks) / 1000.0)
		previous_ticks = now
		frame += 1
		if frame % MOVEMENT_CHECK_INTERVAL == 0:
			older_checkpoint_positions = checkpoint_positions.duplicate()
			if not _liveness_checkpoint(checkpoint_positions, frame, now):
				return
		if now >= deadline:
			break
	# A trailing partial window still owes one liveness check — against the
	# previous full-window baseline, since a handful of leftover frames can
	# span less than a sim tick and would fail a healthy run.
	if samples.size() % MOVEMENT_CHECK_INTERVAL != 0:
		_record_unit_samples(previous_ticks)
		if not _liveness_verdict(_movement_since(older_checkpoint_positions, false), samples.size()):
			return

	if _live_unit_samples.is_empty():
		_record_unit_samples(previous_ticks)
	var min_live_units := 0
	var min_visible_in_tree_units := 0
	var live_unit_seconds := 0.0
	var visible_in_tree_unit_seconds := 0.0
	var sampled_seconds := 0.0
	for index in _live_unit_samples.size():
		var live_count := _live_unit_samples[index]
		var visible_count := _visible_in_tree_samples[index]
		if index == 0 or live_count < min_live_units:
			min_live_units = live_count
		if index == 0 or visible_count < min_visible_in_tree_units:
			min_visible_in_tree_units = visible_count
		var window_seconds := _sample_window_seconds[index]
		live_unit_seconds += live_count * window_seconds
		visible_in_tree_unit_seconds += visible_count * window_seconds
		sampled_seconds += window_seconds
	var mean_live_units := live_unit_seconds / sampled_seconds if sampled_seconds > 0.0 else float(_live_unit_samples[0])
	var mean_visible_in_tree_units := visible_in_tree_unit_seconds / sampled_seconds if sampled_seconds > 0.0 else float(_visible_in_tree_samples[0])

	var sorted_samples := samples.duplicate()
	sorted_samples.sort()
	var total_ms := 0.0
	for sample in samples:
		total_ms += sample
	var mean_ms := total_ms / samples.size()
	var mean_fps := 1000.0 / mean_ms if mean_ms > 0.0 else 0.0
	var renderer := str(ProjectSettings.get_setting("rendering/renderer/rendering_method", "unknown"))
	var adapter := RenderingServer.get_video_adapter_name()
	var vendor := RenderingServer.get_video_adapter_vendor()

	print("GRUS_BENCHMARK_200 start_units=200 min_live_units=%d mean_live_units=%.1f end_units=%d min_visible_in_tree_units=%d mean_visible_in_tree_units=%.1f end_visible_in_tree_units=%d duration_s=%.2f frames=%d mean_ms=%.3f p50_ms=%.3f p95_ms=%.3f p99_ms=%.3f max_ms=%.3f mean_fps=%.1f visibility_revision=%d renderer=%s adapter=%s vendor=%s display=%s" % [
		min_live_units,
		mean_live_units,
		_live_unit_count(),
		min_visible_in_tree_units,
		mean_visible_in_tree_units,
		_visible_in_tree_count(),
		float(previous_ticks - start_ticks) / 1_000_000.0,
		samples.size(),
		mean_ms,
		_percentile(sorted_samples, 0.50),
		_percentile(sorted_samples, 0.95),
		_percentile(sorted_samples, 0.99),
		sorted_samples[-1],
		mean_fps,
		GrusBridge.visibility_revision(),
		renderer,
		adapter,
		vendor,
		DisplayServer.get_name(),
	])
	get_tree().quit(0)
