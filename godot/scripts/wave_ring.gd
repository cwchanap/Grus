extends Control

## Presentation-only wave countdown ring. The controller reads the one wave
## snapshot and writes seconds_remaining / timer_total_seconds here; the amber
## state is exactly "incoming_zones is nonempty". No countdown math or zone
## logic is duplicated.

var ring_ratio := 0.0
var warning := false

func set_state(ratio: float, is_warning: bool) -> void:
	ratio = clampf(ratio, 0.0, 1.0)
	if is_equal_approx(ratio, ring_ratio) and is_warning == warning:
		return
	ring_ratio = ratio
	warning = is_warning
	queue_redraw()

func _draw() -> void:
	var center := size * 0.5
	var radius := maxf(minf(size.x, size.y) * 0.5 - 4.0, 2.0)
	var width := 5.0
	draw_arc(center, radius, 0.0, TAU, 48, Color(0.17, 0.24, 0.3, 0.95), width, true)
	if ring_ratio <= 0.0:
		return
	var color := Color(0.96, 0.62, 0.2, 0.98) if warning else Color(0.35, 0.86, 0.94, 0.98)
	draw_arc(center, radius, -PI * 0.5, -PI * 0.5 + TAU * ring_ratio, 48, color, width, true)
