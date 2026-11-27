extends Control

## HPA-1208 story intro: five painted panels over the Start screen. Presentation
## only: it reads no snapshot and issues no command, so the sim stays frozen in
## Start underneath. The controller decides when it plays (first launch, or
## STORY on Start) and owns the wind loop; this node owns the slideshow, its
## input and the seen flag.
##
## Panels 1-4 move on by themselves; click or Space moves on sooner. The last
## panel waits, and its click lands on Start, as do SKIP and Esc. While open the
## intro swallows every key and click, so nothing reaches the HUD or the world.

signal closed

const SEEN_FLAG_PATH := "user://intro_seen"
## Art paths, not preloads: the 1920x1080 panels load only while the intro
## plays. Panel 4 reuses the Start key art.
const PANELS := [
	{"art": "res://assets/ui/frostline/intro/panel_1.png",
		"caption": "The sky went grey, and the winter never ended."},
	{"art": "res://assets/ui/frostline/intro/panel_2.png",
		"caption": "Then they came down with the snow."},
	{"art": "res://assets/ui/frostline/intro/panel_3.png",
		"caption": "One by one, the cities went silent."},
	{"art": "res://assets/ui/frostline/screens/start_key_art.png",
		"caption": "Grus is the last city still burning."},
	{"art": "res://assets/ui/frostline/intro/panel_5.png",
		"caption": "Its Research Center is building the one weapon that can end them. Hold the walls. Keep the fires lit. Buy it time."},
]
## Five 5-second panels make the ticket's ~25 s run; the slow pan and zoom
## runs longer, so it is still moving when a panel changes.
const PANEL_SECONDS := 5.0
const MOTION_SECONDS := 8.0
const ART_FADE_SECONDS := 0.6
const CAPTION_FADE_SECONDS := 1.2
## The art is drawn at least this much larger than the screen, and pans less
## than that margin, so the zoom never shows an edge.
const ZOOM_FROM := 1.03
const ZOOM_TO := 1.09
const PAN := Vector2(0.012, 0.008)
const SNOW_FLAKES := 90
const CYAN := Color(0.22, 0.83, 0.9)
const DOT_INK := Color(0.78, 0.88, 0.94)

@onready var _art: TextureRect = $ArtClip/Art
@onready var _snow: Control = $Snow
@onready var _caption: Label = $Caption
@onready var _dots: Control = $Dots
@onready var _hint: RichTextLabel = $Hint
@onready var _skip: Button = $SkipButton

var _index := -1
var _panel_started_ms := 0
var _played_ms := 0
## [x, y, radius, fall seconds, alpha] per flake, seeded so every run matches.
var _flakes: Array[PackedFloat32Array] = []

func _ready() -> void:
	visible = false
	set_process(false)
	_skip.pressed.connect(close)
	_snow.draw.connect(_draw_snow)
	_dots.draw.connect(_draw_dots)
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	for _flake in SNOW_FLAKES:
		_flakes.append(PackedFloat32Array([rng.randf(), rng.randf(),
			rng.randf_range(0.5, 1.8), rng.randf_range(6.0, 14.0), rng.randf_range(0.35, 0.85)]))

func seen() -> bool:
	return FileAccess.file_exists(SEEN_FLAG_PATH)

func mark_seen() -> void:
	if FileAccess.open(SEEN_FLAG_PATH, FileAccess.WRITE) == null:
		push_warning("story intro: cannot write %s" % SEEN_FLAG_PATH)

func play() -> void:
	visible = true
	set_process(true)
	_played_ms = Time.get_ticks_msec()
	_show_panel(0)

func close() -> void:
	if not visible:
		return
	visible = false
	set_process(false)
	_index = -1
	_art.texture = null
	closed.emit()

func advance() -> void:
	if _index >= PANELS.size() - 1:
		close()
	else:
		_show_panel(_index + 1)

## The shown panel, 0-based; -1 while closed.
func panel_index() -> int:
	return _index

func _show_panel(index: int) -> void:
	_index = index
	_panel_started_ms = Time.get_ticks_msec()
	var panel: Dictionary = PANELS[index]
	_art.texture = load(str(panel["art"])) as Texture2D
	_caption.text = str(panel["caption"])
	var last := index == PANELS.size() - 1
	_hint.text = "[right]Click or Space to %s  [color=#38d3e6]›[/color][/right]" % (
		"begin" if last else "continue")
	_dots.queue_redraw()
	_animate(0.0)

## Real time, not scaled process delta: the smokes raise Engine.time_scale.
func _process(_delta: float) -> void:
	var elapsed := (Time.get_ticks_msec() - _panel_started_ms) / 1000.0
	if elapsed >= PANEL_SECONDS and _index < PANELS.size() - 1:
		_show_panel(_index + 1)
		return
	_animate(elapsed)
	_snow.queue_redraw()

func _animate(elapsed: float) -> void:
	var motion := ease(clampf(elapsed / MOTION_SECONDS, 0.0, 1.0), 0.6)
	var view := size
	_art.size = view
	_art.pivot_offset = view * 0.5
	_art.scale = Vector2.ONE * lerpf(ZOOM_FROM, ZOOM_TO, motion)
	# Alternate the drift so consecutive panels do not slide the same way.
	var pan := Vector2(-PAN.x if _index % 2 == 0 else PAN.x, -PAN.y)
	_art.position = view * pan * motion
	_art.modulate.a = clampf(elapsed / ART_FADE_SECONDS, 0.0, 1.0)
	_caption.modulate.a = clampf(elapsed / CAPTION_FADE_SECONDS, 0.0, 1.0)

func _gui_input(event: InputEvent) -> void:
	var click := event as InputEventMouseButton
	if click != null and click.pressed and click.button_index == MOUSE_BUTTON_LEFT:
		advance()
	accept_event()

## Runs before the controller's own _input (deeper nodes first), so Tab, Esc
## and the order keys never reach the HUD underneath.
func _input(event: InputEvent) -> void:
	if not visible or not event is InputEventKey:
		return
	get_viewport().set_input_as_handled()
	var key := event as InputEventKey
	if not key.pressed or key.echo:
		return
	match key.keycode:
		KEY_ESCAPE:
			close()
		KEY_SPACE, KEY_ENTER, KEY_KP_ENTER:
			advance()

func _draw_snow() -> void:
	var view := _snow.size
	var unit := view.y / 720.0
	var seconds := (Time.get_ticks_msec() - _played_ms) / 1000.0
	var span := view.x * 1.1
	for flake in _flakes:
		var fall := fposmod(flake[1] + seconds / flake[3], 1.0)
		var y := fall * (view.y + 60.0 * unit) - 30.0 * unit
		var x := fposmod(flake[0] * span - fall * 60.0 * unit, span) - view.x * 0.05
		_snow.draw_circle(Vector2(x, y), flake[2] * unit, Color(1.0, 1.0, 1.0, flake[4]))

func _draw_dots() -> void:
	var gap := 22.0
	var radius := 6.5
	var start := Vector2(_dots.size.x * 0.5 - gap * (PANELS.size() - 1) * 0.5, _dots.size.y * 0.5)
	for slot in PANELS.size():
		var center := start + Vector2(gap * slot, 0.0)
		var diamond := _diamond(center, radius)
		if slot == _index:
			_dots.draw_colored_polygon(_diamond(center, radius + 4.0), Color(CYAN, 0.25))
			_dots.draw_colored_polygon(diamond, CYAN)
		elif slot < _index:
			_dots.draw_colored_polygon(diamond, Color(DOT_INK, 0.45))
		var outline := diamond.duplicate()
		outline.append(diamond[0])
		_dots.draw_polyline(outline, CYAN if slot == _index else Color(DOT_INK, 0.55), 1.5, true)

static func _diamond(center: Vector2, radius: float) -> PackedVector2Array:
	return PackedVector2Array([center + Vector2(0.0, -radius), center + Vector2(radius, 0.0),
		center + Vector2(0.0, radius), center + Vector2(-radius, 0.0)])
