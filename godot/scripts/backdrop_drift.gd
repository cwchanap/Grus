extends TextureRect

## Slow camera drift over a painted backdrop: a zoom and pan that eases in,
## eases out and then glides back, so a held frame never sits still. The art
## stays at least `zoom_from` larger than its box and pans less than that
## margin, so no edge ever shows. Used by the story intro panels and the
## Start key art. Presentation only, on unscaled process time (the smokes
## raise Engine.time_scale; Movie Maker frames stay in step).

## Seconds for one push in; the glide back takes as long.
@export var period_seconds := 8.0
@export var zoom_from := 1.04
@export var zoom_to := 1.16
## Pan at full push, as a fraction of the box size.
@export var pan := Vector2(0.035, 0.015)

var _seconds := 0.0
var _pan_sign := -1.0

func _ready() -> void:
	restart()
	set_process(is_visible_in_tree())

## Starts a fresh push in; -1 drifts left, 1 drifts right.
func restart(pan_sign := -1.0) -> void:
	_seconds = 0.0
	_pan_sign = pan_sign
	_apply()

func _notification(what: int) -> void:
	if what == NOTIFICATION_VISIBILITY_CHANGED:
		set_process(is_visible_in_tree())

func _process(delta: float) -> void:
	_seconds += delta / Engine.time_scale
	_apply()

func _apply() -> void:
	var view := get_parent_area_size()
	var push := 0.5 - 0.5 * cos(PI * _seconds / period_seconds)
	if size != view:
		size = view
	pivot_offset = view * 0.5
	scale = Vector2.ONE * lerpf(zoom_from, zoom_to, push)
	position = view * Vector2(pan.x * _pan_sign, -pan.y) * push
