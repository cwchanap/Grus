extends Node3D

## One tiny transient combat-effect helper: tracer/hit/death flashes plus a
## single generated blip. Every effect is a one-shot mesh that autofrees
## after its lifetime; no persistent effect framework, no audio asset.

var _live: Array[Node] = []
var _sound: AudioStreamPlayer
var _last_cue_ms := 0

func _ready() -> void:
	add_to_group("combat_fx")
	_sound = AudioStreamPlayer.new()
	_sound.stream = _make_blip()
	_sound.volume_db = -14.0
	add_child(_sound)

## Plays at most one cue per drain batch, throttled so a fast tick burst
## stays a cue instead of a machine gun.
func play_hit_cue() -> void:
	var now := Time.get_ticks_msec()
	if now - _last_cue_ms < 150:
		return
	_last_cue_ms = now
	_sound.play()

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
