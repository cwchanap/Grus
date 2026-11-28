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
## Five 5-second panels make the ticket's ~25 s run. Each panel's art drifts
## on its own (backdrop_drift.gd) and crossfades over the one before it; the
## first fades in from black.
const PANEL_SECONDS := 5.0
const CROSSFADE_SECONDS := 1.0
const CAPTION_FADE_SECONDS := 1.2
const CYAN := Color(0.22, 0.83, 0.9)
const DOT_INK := Color(0.78, 0.88, 0.94)

@onready var _art_clip: Control = $ArtClip
## Two drifting art layers that take turns: the next panel fades in on top
## while the previous one keeps drifting underneath.
@onready var _arts: Array[TextureRect] = [$ArtClip/ArtA, $ArtClip/ArtB]
@onready var _caption: Label = $Caption
@onready var _dots: Control = $Dots
@onready var _hint: RichTextLabel = $Hint
@onready var _skip: Button = $SkipButton

var _index := -1
var _panel_seconds := 0.0

func _ready() -> void:
	visible = false
	set_process(false)
	_skip.pressed.connect(close)
	_dots.draw.connect(_draw_dots)

func seen() -> bool:
	return FileAccess.file_exists(SEEN_FLAG_PATH)

func mark_seen() -> void:
	if FileAccess.open(SEEN_FLAG_PATH, FileAccess.WRITE) == null:
		push_warning("story intro: cannot write %s" % SEEN_FLAG_PATH)

func play() -> void:
	visible = true
	set_process(true)
	_show_panel(0)

func close() -> void:
	if not visible:
		return
	visible = false
	set_process(false)
	_index = -1
	for art in _arts:
		art.texture = null
	closed.emit()

func advance() -> void:
	if _index >= PANELS.size() - 1:
		close()
	else:
		_show_panel(_index + 1)

## The shown panel, 0-based; -1 while closed.
func panel_index() -> int:
	return _index

## The art layer of the shown panel.
func shown_art() -> TextureRect:
	return _arts[maxi(_index, 0) % 2]

func _show_panel(index: int) -> void:
	_index = index
	_panel_seconds = 0.0
	var panel: Dictionary = PANELS[index]
	var art := shown_art()
	art.texture = load(str(panel["art"])) as Texture2D
	# Alternate the drift so consecutive panels do not slide the same way.
	art.restart(-1.0 if index % 2 == 0 else 1.0)
	_art_clip.move_child(art, -1)
	_caption.text = str(panel["caption"])
	var last := index == PANELS.size() - 1
	_hint.text = "[right]Click or Space to %s  [color=#38d3e6]›[/color][/right]" % (
		"begin" if last else "continue")
	_dots.queue_redraw()
	_animate(0.0)

## Unscaled process time: the smokes raise Engine.time_scale.
func _process(delta: float) -> void:
	_panel_seconds += delta / Engine.time_scale
	var elapsed := _panel_seconds
	if elapsed >= PANEL_SECONDS and _index < PANELS.size() - 1:
		_show_panel(_index + 1)
		return
	_animate(elapsed)

func _animate(elapsed: float) -> void:
	shown_art().modulate.a = clampf(elapsed / CROSSFADE_SECONDS, 0.0, 1.0)
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
