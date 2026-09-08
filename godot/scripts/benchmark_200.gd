extends Node

const BENCHMARK_SIZE := Vector2i(1920, 1080)
const WARMUP_FRAMES := 60
const SAMPLE_FRAMES := 240

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

func _run() -> void:
	var window := get_window()
	window.content_scale_size = BENCHMARK_SIZE
	window.size = BENCHMARK_SIZE
	await get_tree().process_frame
	_progress("viewport")

	if get_viewport().get_visible_rect().size != Vector2(BENCHMARK_SIZE):
		_fail("benchmark viewport did not resize to 1920x1080")
		return

	var units: Array[Node] = []
	for _frame in range(180):
		await get_tree().physics_frame
		units = get_tree().get_nodes_in_group("unit_views")
		if units.size() == 200:
			break
	if units.size() != 200:
		_fail("expected 200 unit views, found %d" % units.size())
		return
	_progress("unit_views")

	for _frame in range(30):
		await get_tree().process_frame
		var initialized := 0
		for unit in units:
			if int(unit.get_meta("unit_id", -1)) > 0:
				initialized += 1
		if initialized == 200:
			break

	var initial_positions: Dictionary = {}
	for unit in units:
		var id := int(unit.get_meta("unit_id", -1))
		if id <= 0:
			_fail("unit metadata did not initialize before benchmark")
			return
		initial_positions[id] = (unit as Node3D).global_position
	_progress("metadata")

	if not GrusBridge.benchmark_move_all():
		_fail("benchmark movement bridge rejected the 200-unit fixture")
		return

	for _frame in range(8):
		await get_tree().physics_frame

	var moved := 0
	for unit in units:
		var id := int(unit.get_meta("unit_id"))
		if (unit as Node3D).global_position.distance_to(initial_positions[id]) > 0.05:
			moved += 1
	if moved != 200:
		_fail("benchmark expected 200 moving units, observed %d" % moved)
		return
	_progress("moving")

	for _frame in range(WARMUP_FRAMES):
		await get_tree().process_frame
	_progress("warmup", WARMUP_FRAMES)

	var samples: Array[float] = []
	var previous_ticks := Time.get_ticks_usec()
	for frame in range(SAMPLE_FRAMES):
		await get_tree().process_frame
		var current_ticks := Time.get_ticks_usec()
		samples.append(float(current_ticks - previous_ticks) / 1000.0)
		previous_ticks = current_ticks
		if (frame + 1) % 60 == 0:
			_progress("sample", frame + 1)

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

	print("GRUS_BENCHMARK_200 moved=%d frames=%d mean_ms=%.3f p50_ms=%.3f p95_ms=%.3f p99_ms=%.3f max_ms=%.3f mean_fps=%.1f renderer=%s adapter=%s vendor=%s display=%s" % [moved, SAMPLE_FRAMES, mean_ms, _percentile(sorted_samples, 0.50), _percentile(sorted_samples, 0.95), _percentile(sorted_samples, 0.99), sorted_samples[-1], mean_fps, renderer, adapter, vendor, DisplayServer.get_name()])
	get_tree().quit(0)
