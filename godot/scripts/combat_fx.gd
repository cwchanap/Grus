extends Node3D

## Transient tracer/hit/death flashes and the pilot sound cues. The generic
## blip remains for combat events without a dedicated pilot sound.

const CUE_THROTTLE_MS := 150
## Best-effort shutdown drain tuning. Two internal mixer blocks (the pinned
## engine mixes in 512-frame blocks) of observed driver-callback time is the
## target, and the deadline bounds a stalled or never-started driver.
const DRAIN_TARGET_FRAMES := 1024
const DRAIN_MIN_CALLBACKS := 2
const DRAIN_TIMEOUT_MS := 500
const CUES := {
	"generic_blip": {"volume_db": -14.0, "max_polyphony": 1},
	"gun_turret_fire": {"stream": preload("res://assets/audio/combat/gun_turret_fire.ogg"), "volume_db": -12.0, "max_polyphony": 2},
	"swarmer_death": {"stream": preload("res://assets/audio/combat/swarmer_death.ogg"), "volume_db": -12.0, "max_polyphony": 2},
	"winter_wind": {"stream": preload("res://assets/audio/ambience/winter_wind_loop.ogg"), "volume_db": -25.0, "max_polyphony": 1, "loop": true},
}

var cue_counts: Dictionary = {}
var _live: Array[Node] = []
var _players: Dictionary = {}
var _last_cue_ms: Dictionary = {}

func _ready() -> void:
	add_to_group("combat_fx")
	for cue_id in CUES:
		var spec: Dictionary = CUES.get(cue_id, {})
		var player := AudioStreamPlayer.new()
		player.stream = spec.get("stream", null) as AudioStream
		if player.stream == null:
			player.stream = _make_blip()
		if bool(spec.get("loop", false)):
			(player.stream as AudioStreamOggVorbis).loop = true
		player.volume_db = float(spec.get("volume_db", 0.0))
		player.max_polyphony = maxi(int(spec.get("max_polyphony", 1)), 1)
		_players[cue_id] = player
		add_child(player)

func _exit_tree() -> void:
	for cue_id in _players:
		var player := _players[cue_id] as AudioStreamPlayer
		player.stop()
		player.stream = null

func play_cue(cue_id: String) -> void:
	var player := _players.get(cue_id) as AudioStreamPlayer
	if player == null:
		return
	var now := Time.get_ticks_msec()
	if _last_cue_ms.has(cue_id) and now - int(_last_cue_ms[cue_id]) < CUE_THROTTLE_MS:
		return
	_last_cue_ms[cue_id] = now
	player.play()
	cue_counts[cue_id] = int(cue_counts.get(cue_id, 0)) + 1

## Plays at most one cue per drain batch, throttled so a fast tick burst
## stays a cue instead of a machine gun.
func play_hit_cue() -> void:
	play_cue("generic_blip")

func set_wind_phase(phase: String) -> void:
	var player := _players["winter_wind"] as AudioStreamPlayer
	match phase:
		"Playing":
			player.stream_paused = false
			if not player.playing:
				player.play()
				cue_counts["winter_wind"] = int(cue_counts.get("winter_wind", 0)) + 1
		"Paused":
			player.stream_paused = true
		_:
			player.stop()
			player.stream_paused = false

func spawn_tracer(from: Vector3, to: Vector3) -> void:
	var span := to - from
	if span.length() < 0.5:
		return
	var mesh := BoxMesh.new()
	mesh.size = Vector3(0.08, 0.08, span.length())
	var view := _spawn_transient(mesh, Color(1.0, 0.9, 0.4), 0.1)
	view.look_at_from_position((from + to) * 0.5, to, Vector3.UP)

func spawn_hit(position: Vector3) -> void:
	_flash(position, Color(1.0, 0.8, 0.2), 0.25, 0.12)

func spawn_death(position: Vector3) -> void:
	_flash(position, Color(0.9, 0.2, 0.15), 0.9, 0.3)

func effect_count() -> int:
	return _live.size()

## Frees every live transient (restart hygiene); the sound player stays.
func clear_all() -> void:
	for node in _live:
		node.queue_free()
	_live.clear()
	for cue_id in _players:
		(_players[cue_id] as AudioStreamPlayer).stop()
	cue_counts.clear()
	_last_cue_ms.clear()

## Best-effort audio quiesce before quitting. Godot exposes no acknowledgement
## that a stopped playback was reclaimed: the mixer thread only queues it for
## deletion on a mixer-block boundary, and the only observable signal is the
## audio driver's per-callback clock. Each observed clock reset is one driver
## callback; the audio time sampled between callbacks is credited until two
## 512-frame mixer blocks have passed. This is a bounded mitigation, not proof:
## callbacks can be missed, a stalled driver cannot be drained, and devices
## whose callbacks are far smaller than a mixer block can still hit the
## deadline. The caller quits either way and reports the fallback.
## `clock`/`timeout_ms`/`mix_rate` are injectable for the focused shutdown
## test. The deadline is frame-polled (not a hard upper bound: a stalled frame
## or the driver-locked clock read can overrun it) and is rechecked after the
## await and clock read, so a resumed iteration past it returns the fallback
## instead of reporting success.
func stop_and_drain(clock: Callable = Callable(), timeout_ms := DRAIN_TIMEOUT_MS, mix_rate := 0.0) -> bool:
	for cue_id in _players:
		(_players[cue_id] as AudioStreamPlayer).stop()
	var read_clock: Callable = clock if clock.is_valid() else AudioServer.get_time_since_last_mix
	var rate := mix_rate if mix_rate > 0.0 else float(AudioServer.get_mix_rate())
	var last: float = read_clock.call()
	var callbacks := 0
	var credited := 0
	var deadline := Time.get_ticks_msec() + timeout_ms
	while true:
		await get_tree().process_frame
		var mix: float = read_clock.call()
		if Time.get_ticks_msec() >= deadline:
			return false
		if mix < last:
			callbacks += 1
			# The first reset only syncs the window: its interval may start
			# before the stop, so it is never credited.
			if callbacks >= DRAIN_MIN_CALLBACKS:
				credited += int(last * rate)
				if credited >= DRAIN_TARGET_FRAMES:
					return true
		last = mix
	return false

func _flash(position: Vector3, color: Color, size: float, lifetime: float) -> void:
	var mesh := SphereMesh.new()
	mesh.radius = size
	mesh.height = size * 2.0
	var view := _spawn_transient(mesh, color, lifetime)
	view.position = position

func _spawn_transient(mesh: Mesh, color: Color, lifetime: float) -> MeshInstance3D:
	var view := MeshInstance3D.new()
	view.mesh = mesh
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = color
	view.material_override = material
	add_child(view)
	_live.append(view)
	# Real-time lifetime: SceneTreeTimer otherwise scales with the sim-speed
	# multiplier, which would make 20x runs flash for a few milliseconds.
	# Weakref capture: clear_all() on restart may free the view first, and a
	# strong freed capture makes Godot log a capture error on every expiry.
	var ref: WeakRef = weakref(view)
	get_tree().create_timer(lifetime, true, false, true).timeout.connect(func() -> void:
		var node: Variant = ref.get_ref()
		if node != null:
			_live.erase(node)
			node.queue_free()
	)
	return view

## One tiny attack/hit blip: a generated 8-bit sawtooth click — no audio
## asset, no audio framework.
func _make_blip() -> AudioStreamWAV:
	var rate := 11025
	var data := PackedByteArray()
	data.resize(int(rate * 0.06))
	for i in data.size():
		data[i] = (i % 16) * 15
	var wav := AudioStreamWAV.new()
	wav.format = AudioStreamWAV.FORMAT_8_BITS
	wav.mix_rate = rate
	wav.stereo = false
	wav.data = data
	return wav
