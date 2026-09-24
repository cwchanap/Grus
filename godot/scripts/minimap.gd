extends Control

## Minimap: a 128x96 ImageTexture of the Team-1 fog states (1 px per cell),
## refreshed from the bridge only when the sim visibility revision changes.
## Overlay drawing reads the live scene views: friendly unit/building
## markers always, enemy markers only while their view is visible in tree,
## explored standalone resource markers, plus the camera viewport. Clicks
## recenter the existing camera; they never issue gameplay commands and the
## minimap owns no gameplay state.

const MAP_WIDTH := 128
const MAP_HEIGHT := 96

const COLOR_UNEXPLORED := Color(0.05, 0.06, 0.05)
const COLOR_EXPLORED := Color(0.14, 0.2, 0.13)
const COLOR_VISIBLE := Color(0.26, 0.36, 0.24)
const FRIENDLY_COLOR := Color(0.3, 0.6, 1.0)
const ENEMY_COLOR := Color(1.0, 0.35, 0.3)
const RESOURCE_COLOR := Color(0.95, 0.8, 0.3)
const CAMERA_COLOR := Color(1, 1, 1, 0.85)

## Live marker counts for smoke/telemetry reads.
var friendly_marker_count := 0
var enemy_marker_count := 0
var resource_marker_count := 0

var _revision := -1
var _image: Image
var _texture: ImageTexture
var _friendly_points := PackedVector2Array()
var _enemy_points := PackedVector2Array()
var _resource_points := PackedVector2Array()

@onready var _camera: Camera3D = get_node("../../Camera3D")

func _ready() -> void:
	_image = Image.create(MAP_WIDTH, MAP_HEIGHT, false, Image.FORMAT_RGB8)
	_texture = ImageTexture.create_from_image(_image)

func _process(_delta: float) -> void:
	var revision := int(GrusBridge.visibility_revision())
	if revision != _revision:
		_revision = revision
		_refresh_base(revision)
	_refresh_markers()
	queue_redraw()

func _refresh_base(revision: int) -> void:
	if revision < 0:
		# No visibility state (visibility-free benchmark): plain terrain.
		_image.fill(COLOR_VISIBLE)
		_texture.update(_image)
		return
	var snap: Dictionary = GrusBridge.visibility_snapshot()
	if int(snap.get("revision", -1)) != revision:
		return
	var width := int(snap.get("width", 0))
	var states: PackedInt32Array = snap.get("states", PackedInt32Array())
	if width != MAP_WIDTH or states.size() != MAP_WIDTH * MAP_HEIGHT:
		return
	var x := 0
	var y := 0
	for value in states:
		match value:
			2:
				_image.set_pixel(x, y, COLOR_VISIBLE)
			1:
				_image.set_pixel(x, y, COLOR_EXPLORED)
			_:
				_image.set_pixel(x, y, COLOR_UNEXPLORED)
		x += 1
		if x >= width:
			x = 0
			y += 1
	_texture.update(_image)

func _refresh_markers() -> void:
	_friendly_points.clear()
	_enemy_points.clear()
	_resource_points.clear()
	for node in get_tree().get_nodes_in_group("unit_views"):
		_add_marker(node)
	for node in get_tree().get_nodes_in_group("building_views"):
		_add_marker(node)
	for node in get_tree().get_nodes_in_group("resource_views"):
		var view := node as Node3D
		if view != null and view.is_visible_in_tree():
			_resource_points.append(_point_of(view))
	friendly_marker_count = _friendly_points.size()
	enemy_marker_count = _enemy_points.size()
	resource_marker_count = _resource_points.size()

## Friendly views always mark; enemy views only while presented (a hidden
## enemy view must never become a minimap marker).
func _add_marker(node: Node) -> void:
	var view := node as Node3D
	if view == null:
		return
	var team := int(node.get_meta("team_id", -1))
	if team == 1:
		_friendly_points.append(_point_of(view))
	elif view.is_visible_in_tree():
		_enemy_points.append(_point_of(view))

func _point_of(view: Node3D) -> Vector2:
	return Vector2(
		clampf(view.global_position.x, 0.0, MAP_WIDTH - 1.0),
		clampf(view.global_position.z, 0.0, MAP_HEIGHT - 1.0))

func _draw() -> void:
	if _texture != null:
		draw_texture_rect(_texture, Rect2(Vector2.ZERO, Vector2(MAP_WIDTH, MAP_HEIGHT)), false)
	for point in _resource_points:
		draw_circle(point, 1.5, RESOURCE_COLOR)
	for point in _enemy_points:
		draw_circle(point, 2.0, ENEMY_COLOR)
	for point in _friendly_points:
		draw_circle(point, 2.0, FRIENDLY_COLOR)
	var corners := _camera_ground_corners()
	if corners.size() == 4:
		draw_polyline(corners + PackedVector2Array([corners[0]]), CAMERA_COLOR, 1.0)

## The camera frustum's intersection with the ground plane, minimap-space.
func _camera_ground_corners() -> PackedVector2Array:
	var viewport_size := get_viewport().get_visible_rect().size
	var corners := PackedVector2Array()
	for screen_point in [Vector2.ZERO, Vector2(viewport_size.x, 0.0), viewport_size, Vector2(0.0, viewport_size.y)]:
		var origin := _camera.project_ray_origin(screen_point)
		var direction := _camera.project_ray_normal(screen_point)
		if absf(direction.y) < 0.0001:
			continue
		var distance := -origin.y / direction.y
		if distance < 0.0:
			continue
		var hit := origin + direction * distance
		corners.append(Vector2(
			clampf(hit.x, 0.0, MAP_WIDTH - 1.0),
			clampf(hit.z, 0.0, MAP_HEIGHT - 1.0)))
	return corners

func _gui_input(event: InputEvent) -> void:
	var click := event as InputEventMouseButton
	if click == null or click.button_index != MOUSE_BUTTON_LEFT or not click.pressed:
		return
	var cell := Vector2i(
		clampi(int(click.position.x), 0, MAP_WIDTH - 1),
		clampi(int(click.position.y), 0, MAP_HEIGHT - 1))
	# The camera is pitched down, so its position is not its viewed ground
	# point: shift the camera by the difference between the clicked cell and
	# where the viewport-center ray currently hits the ground. The center
	# ray is computed from the logical transform — `project_ray_*` read the
	# physics-interpolated transform, which lags a fresh jump by a frame.
	var forward := -_camera.global_transform.basis.z.normalized()
	if absf(forward.y) >= 0.0001:
		var hit := _camera.global_position + forward * (-_camera.global_position.y / forward.y)
		_camera.position.x += (cell.x + 0.5) - hit.x
		_camera.position.z += (cell.y + 0.5) - hit.z
		_camera.reset_physics_interpolation()
	accept_event()
