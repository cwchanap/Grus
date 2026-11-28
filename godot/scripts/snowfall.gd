extends Control

## Falling snow in two depth layers: many small, faint, slow far flakes and
## fewer large, bright, fast near flakes that drift further sideways, so the
## snow reads as depth over a still painting. Presentation only, shared by
## the story intro and the Start backdrop through weather.tscn. It redraws
## only while visible, on unscaled process time (the smokes raise
## Engine.time_scale; Movie Maker frames stay in step).

const FAR_FLAKES := 110
const NEAR_FLAKES := 36

## [x, y, radius, fall seconds, alpha, sideways drift, sway phase] per flake,
## in 720p units; seeded so every run matches.
var _flakes: Array[PackedFloat32Array] = []
var _seconds := 0.0

func _ready() -> void:
	mouse_filter = MOUSE_FILTER_IGNORE
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	for _flake in FAR_FLAKES:
		_flakes.append(PackedFloat32Array([rng.randf(), rng.randf(), rng.randf_range(0.5, 1.2),
			rng.randf_range(11.0, 18.0), rng.randf_range(0.25, 0.55), 50.0, rng.randf() * TAU]))
	for _flake in NEAR_FLAKES:
		_flakes.append(PackedFloat32Array([rng.randf(), rng.randf(), rng.randf_range(1.5, 2.8),
			rng.randf_range(4.0, 7.0), rng.randf_range(0.6, 0.9), 170.0, rng.randf() * TAU]))
	set_process(is_visible_in_tree())

func _notification(what: int) -> void:
	if what == NOTIFICATION_VISIBILITY_CHANGED:
		set_process(is_visible_in_tree())

func _process(delta: float) -> void:
	_seconds += delta / Engine.time_scale
	queue_redraw()

func _draw() -> void:
	var view := size
	var unit := view.y / 720.0
	var span := view.x * 1.2
	for flake in _flakes:
		var fall := fposmod(flake[1] + _seconds / flake[3], 1.0)
		var y := fall * (view.y + 60.0 * unit) - 30.0 * unit
		var sway := sin(_seconds * 1.3 + flake[6]) * 6.0 * unit
		var x := fposmod(flake[0] * span - fall * flake[5] * unit + sway, span) - view.x * 0.1
		draw_circle(Vector2(x, y), flake[2] * unit, Color(1.0, 1.0, 1.0, flake[4]))
