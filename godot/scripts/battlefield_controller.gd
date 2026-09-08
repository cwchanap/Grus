extends Node3D

const CLICK_RADIUS := 20.0

@onready var camera: Camera3D = $Camera3D
@onready var command_status: Label = $HUD/CommandStatus

var selected_ids: Array[int] = []
var _control_groups: Dictionary = {}
var _feedback_revision := -1

func _ready() -> void:
	_feedback_revision = int(GrusBridge.command_feedback_revision())
	command_status.text = str(GrusBridge.command_feedback())

func _process(_delta: float) -> void:
	var revision := int(GrusBridge.command_feedback_revision())
	if revision != _feedback_revision:
		_feedback_revision = revision
		command_status.text = str(GrusBridge.command_feedback())

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey:
		_handle_key(event as InputEventKey)
		return
	if not event is InputEventMouseButton:
		return
	var mouse_event := event as InputEventMouseButton
	if not mouse_event.pressed:
		return

	match mouse_event.button_index:
		MOUSE_BUTTON_LEFT:
			_select_at(mouse_event.position, mouse_event.shift_pressed)
		MOUSE_BUTTON_RIGHT:
			_issue_move(mouse_event.position)

func _handle_key(event: InputEventKey) -> void:
	if not event.pressed or event.echo:
		return
	if event.keycode < KEY_1 or event.keycode > KEY_9:
		return

	var group := int(event.keycode - KEY_1 + 1)
	if event.ctrl_pressed:
		_control_groups[group] = selected_ids.duplicate()
		return
	if not _control_groups.has(group):
		return

	selected_ids.clear()
	for id in _control_groups[group]:
		selected_ids.append(int(id))
	_apply_selection()

func _select_at(screen_position: Vector2, additive: bool) -> void:
	var nearest: Node3D = null
	var nearest_distance := CLICK_RADIUS

	for unit in _friendly_units():
		if camera.is_position_behind(unit.global_position):
			continue
		var distance := camera.unproject_position(unit.global_position).distance_to(screen_position)
		if distance <= nearest_distance:
			nearest = unit
			nearest_distance = distance

	if not additive:
		selected_ids.clear()
	if nearest != null:
		var id := int(nearest.get_meta("unit_id", -1))
		if id > 0 and not selected_ids.has(id):
			selected_ids.append(id)
	_apply_selection()

func _apply_selection() -> void:
	var live_selected: Array[int] = []
	for unit in _friendly_units():
		var id := int(unit.get_meta("unit_id", -1))
		var selected := selected_ids.has(id)
		unit.call("set_selected", selected)
		if selected:
			live_selected.append(id)
	selected_ids = live_selected

func _issue_move(screen_position: Vector2) -> void:
	if selected_ids.is_empty():
		return
	var target = _ground_target(screen_position)
	if target == null:
		command_status.text = "No ground target"
		return
	if GrusBridge.move_units(PackedInt32Array(selected_ids), target):
		command_status.text = "Move command queued"
	else:
		command_status.text = "Move command rejected"

func _ground_target(screen_position: Vector2):
	var origin := camera.project_ray_origin(screen_position)
	var direction := camera.project_ray_normal(screen_position)
	if absf(direction.y) < 0.0001:
		return null
	var distance := -origin.y / direction.y
	if distance < 0.0:
		return null
	var hit := origin + direction * distance
	return Vector2(hit.x, hit.z)

func _friendly_units() -> Array[Node3D]:
	var result: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == 1:
			result.append(node as Node3D)
	return result
