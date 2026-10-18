extends Node

## HPA-474 mixed-workload benchmark. After a benchmark reset it waits for the
## full 200-unit load (buildings/resources may already show extra AI
## foundations), waits for the fog revision to advance so the sim is provably
## ticking, then records one frame-time sample per rendered frame plus the
## live unit count for a real-time duration. A liveness checkpoint between
## fixed frame intervals fails a sample that shows neither unit movement nor
## combat activity. Real combat deaths are expected: units are never respawned
## and health is never reset.

const BENCHMARK_SIZE := Vector2i(1920, 1080)
const DEFAULT_SAMPLE_SECONDS := 10.0
const WARMUP_SECONDS := 2.0
const VIEW_WAIT_FRAMES := 180
const VISIBILITY_WAIT_FRAMES := 600
const MOVEMENT_CHECK_INTERVAL := 20
const MOVEMENT_EPSILON := 0.05

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

## Sample duration: `GRUS_BENCHMARK_SECONDS` when set to a positive finite
## value, else the 10 s default. A present but invalid value fails the run —
## silently sampling a different duration than requested would corrupt the
## measurement contract.
func _read_sample_seconds() -> float:
	var raw := OS.get_environment("GRUS_BENCHMARK_SECONDS")
	if raw.is_empty():
		return DEFAULT_SAMPLE_SECONDS
	var seconds := raw.to_float()
	if not is_finite(seconds) or seconds <= 0.0:
		_fail("GRUS_BENCHMARK_SECONDS must be a positive finite number of seconds, got '%s'" % raw)
		return -1.0
	return seconds

## One liveness window: drains benchmark combat events through the bridge and
## compares still-existing unit positions against the previous checkpoint.
## Returns false (after failing) when the window shows neither meaningful
## movement nor combat activity — a rendering-alive but simulation-dead load.
func _liveness_checkpoint(checkpoint_positions: Dictionary, window_end_frame: int) -> bool:
	var events: Array = GrusBridge.drain_combat_events()
	var moved := 0
	for unit in get_tree().get_nodes_in_group("unit_views"):
		var id := int(unit.get_meta("unit_id", -1))
		if id <= 0:
			continue
		var now: Vector3 = (unit as Node3D).global_position
		if checkpoint_positions.has(id) and now.distance_to(checkpoint_positions[id]) > MOVEMENT_EPSILON:
			moved += 1
		checkpoint_positions[id] = now
	if moved == 0 and events.is_empty():
		_fail("benchmark liveness window ending at frame %d showed neither unit movement nor combat activity" % window_end_frame)
		return false
	_progress("active", moved)
	return true

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
	if not GrusBridge.reset_benchmark_fixture():
		_fail("benchmark reset bridge rejected the benchmark fixture")
		return
	_progress("benchmark_reset")

	# Start shape: exactly 200 unique unit views; buildings >= 2 (the AI may
	# place extra foundations before views attach); at least the authored 18
	# resource sources. The workload itself was already accepted during the
	# benchmark reset — there is no separate start call.
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
				and get_tree().get_nodes_in_group("building_views").size() >= 2 \
				and get_tree().get_nodes_in_group("resource_views").size() >= 18:
			break
	if units.size() != 200 or unique_ids.size() != 200:
		_fail("expected 200 unique unit views, found %d nodes / %d ids" % [units.size(), unique_ids.size()])
		return
	if get_tree().get_nodes_in_group("building_views").size() < 2:
		_fail("benchmark fixture must show at least 2 building views, found %d" % get_tree().get_nodes_in_group("building_views").size())
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

	var warmup_deadline := Time.get_ticks_usec() + int(roundf(WARMUP_SECONDS * 1_000_000.0))
	while Time.get_ticks_usec() < warmup_deadline:
		await get_tree().process_frame
	_progress("warmup")

	# Checkpoint baseline at the start of the sample so each liveness window
	# compares against positions from inside the measured load.
	var checkpoint_positions: Dictionary = {}
	for unit in units:
		checkpoint_positions[int(unit.get_meta("unit_id"))] = (unit as Node3D).global_position

	var samples: Array[float] = []
	var min_live_units := 0
	var live_unit_sum := 0
	var start_ticks := Time.get_ticks_usec()
	var deadline := start_ticks + int(roundf(sample_seconds * 1_000_000.0))
	var previous_ticks := start_ticks
	var frame := 0
	while true:
		await get_tree().process_frame
		var now := Time.get_ticks_usec()
		samples.append(float(now - previous_ticks) / 1000.0)
		previous_ticks = now
		var live := get_tree().get_nodes_in_group("unit_views").size()
		if frame == 0 or live < min_live_units:
			min_live_units = live
		live_unit_sum += live
		frame += 1
		if frame % MOVEMENT_CHECK_INTERVAL == 0 and not _liveness_checkpoint(checkpoint_positions, frame):
			return
		if now >= deadline:
			break
	# A trailing partial window still owes one liveness check.
	if samples.size() % MOVEMENT_CHECK_INTERVAL != 0 and not _liveness_checkpoint(checkpoint_positions, samples.size()):
		return

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

	print("GRUS_BENCHMARK_200 start_units=200 min_live_units=%d mean_live_units=%.1f end_units=%d duration_s=%.2f frames=%d mean_ms=%.3f p50_ms=%.3f p95_ms=%.3f p99_ms=%.3f max_ms=%.3f mean_fps=%.1f visibility_revision=%d renderer=%s adapter=%s vendor=%s display=%s" % [
		min_live_units,
		float(live_unit_sum) / samples.size(),
		get_tree().get_nodes_in_group("unit_views").size(),
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
