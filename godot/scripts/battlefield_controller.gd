extends Node3D

const CLICK_RADIUS := 20.0
const DRAG_THRESHOLD := 6.0
const MIN_CAMERA_SIZE := 24.0
const MAX_CAMERA_SIZE := 120.0
const ZOOM_STEP := 6.0

@onready var camera: Camera3D = $Camera3D
@onready var command_status: Label = $HUD/CommandStatus
@onready var economy_label: Label = $HUD/EconomyPanel/EconomyLabel
@onready var idle_button: Button = $HUD/EconomyPanel/IdleButton
@onready var selection_label: Label = $HUD/SelectionPanel/SelectionLabel
@onready var age_button: Button = $HUD/CommandPanel/TrainGrid/AgeButton
@onready var _build_grid: Container = $HUD/CommandPanel/BuildGrid
@onready var _train_grid: Container = $HUD/CommandPanel/TrainGrid
@onready var _preview_box: MeshInstance3D = $PlacementPreview
@onready var _fx: Node3D = $CombatFx
@onready var session_panel: Panel = $HUD/SessionPanel
@onready var session_label: Label = $HUD/SessionPanel/SessionLabel
@onready var start_button: Button = $HUD/SessionPanel/StartButton
@onready var resume_button: Button = $HUD/SessionPanel/ResumeButton
@onready var restart_button: Button = $HUD/SessionPanel/RestartButton
@onready var quit_button: Button = $HUD/SessionPanel/QuitButton
@onready var pause_button: Button = $HUD/PauseButton

var selected_ids: Array[int] = []
var selected_building_id := -1
var _control_groups: Dictionary = {}
var _feedback_revision := -1
var _left_press_position := Vector2.ZERO
var _left_pressed := false
var _placement_kind := ""
var _idle_cursor := 0
var _session_phase := ""
var _attack_move_armed := false
## Producer building kinds and the Age-up cost come from the Rust catalogue
## via catalogue_snapshot — GDScript hardcodes no gameplay data.
var _producer_kinds: Array[String] = []

func _ready() -> void:
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var producers := {}
	var units: Dictionary = catalogue.get("units", {})
	for kind in units:
		producers[str(units[kind].get("producer", ""))] = true
	_producer_kinds.assign(producers.keys())
	_producer_kinds.sort()
	var age_up: Dictionary = catalogue.get("age_up", {})
	age_button.text = "Advance Age (%df %dg)" % [
		int(age_up.get("food", 0)),
		int(age_up.get("gold", 0)),
	]
	_feedback_revision = int(GrusBridge.command_feedback_revision())
	command_status.text = str(GrusBridge.command_feedback())
	for button in _build_grid.get_children():
		button.pressed.connect(_on_build_pressed.bind(str(button.get_meta("kind"))))
	for button in _train_grid.get_children():
		if button.has_meta("kind"):
			button.pressed.connect(_on_train_pressed.bind(str(button.get_meta("kind"))))
		else:
			button.pressed.connect(_on_age_pressed)
	idle_button.pressed.connect(_on_idle_pressed)
	start_button.pressed.connect(_on_start_pressed)
	resume_button.pressed.connect(_on_resume_pressed)
	pause_button.pressed.connect(_on_pause_pressed)
	restart_button.pressed.connect(_on_restart_pressed)
	quit_button.pressed.connect(_on_quit_pressed)

func _process(_delta: float) -> void:
	var revision := int(GrusBridge.command_feedback_revision())
	if revision != _feedback_revision:
		_feedback_revision = revision
		command_status.text = str(GrusBridge.command_feedback())
	_refresh_session()
	_refresh_hud()

## Reads the session once per frame: drives the overlay and drains
## current-tick combat events only while Playing — the bridge keeps the last
## Playing tick's events readable across a pause/Result, and those stale
## events must never replay as fresh effects.
func _refresh_session() -> void:
	var snap: Dictionary = GrusBridge.session_snapshot()
	_session_phase = str(snap.get("phase", ""))
	_refresh_session_overlay(snap)
	if _session_phase != "Playing":
		return
	var events: Array = GrusBridge.drain_combat_events()
	for event in events:
		_dispatch_combat_event(event)

func _refresh_session_overlay(snap: Dictionary) -> void:
	var playing := _session_phase == "Playing"
	session_panel.visible = not playing
	pause_button.visible = playing
	# Restart/Quit belong to Result only.
	match _session_phase:
		"Start":
			session_label.text = "Skirmish ready"
			start_button.visible = true
			resume_button.visible = false
			restart_button.visible = false
			quit_button.visible = false
		"Paused":
			session_label.text = "Paused"
			start_button.visible = false
			resume_button.visible = true
			restart_button.visible = false
			quit_button.visible = false
		"Result":
			session_label.text = "Victory!" if int(snap.get("winner_team", -1)) == 1 else "Defeat"
			start_button.visible = false
			resume_button.visible = false
			restart_button.visible = true
			quit_button.visible = true
		_:
			# Playing: panel hidden.
			pass

## Gameplay orders only ever leave Godot while the session is Playing.
func _playing() -> bool:
	return _session_phase == "Playing"

## Cosmetics only: reads the event payload, never touches combat state.
func _dispatch_combat_event(event: Dictionary) -> void:
	var hit := Vector3(float(event.get("x", 0.0)), 1.0, float(event.get("y", 0.0)))
	if bool(event.get("ranged", false)):
		var attacker := _unit_view(int(event.get("attacker", -1)))
		if attacker != null:
			_fx.spawn_tracer(attacker.global_position + Vector3(0.0, 0.8, 0.0), hit)
	_fx.spawn_hit(hit)
	if bool(event.get("killed", false)):
		_fx.spawn_death(hit)
		_prune_destroyed_selection(str(event.get("target_kind", "")), int(event.get("target_id", -1)))
	_fx.play_hit_cue()

## A kill event names the destroyed stable id: drop it from the selection and
## every control group so later orders never reference a dead object, then
## reconcile the live selection state.
func _prune_destroyed_selection(target_kind: String, target_id: int) -> void:
	if target_kind == "unit":
		selected_ids.erase(target_id)
		for group in _control_groups:
			_control_groups[group].erase(target_id)
	elif target_kind == "building" and selected_building_id == target_id:
		selected_building_id = -1
	_apply_selection()

func _refresh_hud() -> void:
	var economy: Dictionary = GrusBridge.economy_snapshot()
	economy_label.text = "Food %d   Wood %d   Gold %d\nPopulation %d/%d   Age %d" % [
		int(economy.get("food", 0)),
		int(economy.get("wood", 0)),
		int(economy.get("gold", 0)),
		int(economy.get("population_used", 0)),
		int(economy.get("population_cap", 0)),
		int(economy.get("age", 1)),
	]
	idle_button.text = "Idle: %d — Next" % int(economy.get("idle_workers", 0))
	var building: Dictionary = {}
	if selected_building_id > 0:
		building = GrusBridge.building_snapshot(selected_building_id)
	age_button.disabled = int(economy.get("age", 1)) >= 2 \
		or str(building.get("kind", "")) != "TownCenter"
	_refresh_selection(building)

func _refresh_selection(building: Dictionary) -> void:
	if not selected_ids.is_empty():
		var labels := PackedStringArray()
		for id in selected_ids:
			labels.append("%s #%d" % [_unit_kind(id), id])
		selection_label.text = "Selected: " + ", ".join(labels)
		return
	if selected_building_id > 0 and not building.is_empty():
		var text := "%s #%d (team %d)" % [
			str(building.get("kind", "?")),
			selected_building_id,
			int(building.get("team_id", 0)),
		]
		if bool(building.get("complete", false)):
			text += "\nConstruction complete"
		else:
			text += "\nConstruction %d%%" % roundi(float(building.get("construction_progress", 0.0)) * 100.0)
		var queue_label := str(building.get("queue_label", ""))
		if queue_label != "":
			text += "\nTraining %s — %d%%" % [queue_label, roundi(float(building.get("queue_progress", 0.0)) * 100.0)]
		var blocked := int(building.get("blocked_reason", 0))
		if blocked != 0:
			text += "\nBlocked (code %d)" % blocked
		if int(building.get("rally_x", -1)) >= 0:
			text += "\nRally: (%d, %d)" % [int(building.get("rally_x", -1)), int(building.get("rally_y", -1))]
		else:
			text += "\nRally: none"
		selection_label.text = text
		return
	selection_label.text = "Nothing selected"

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey:
		_handle_key(event as InputEventKey)
		return
	if event is InputEventMouseMotion:
		var motion := event as InputEventMouseMotion
		if _placement_kind != "":
			_update_placement_preview(motion.position)
			return
		if motion.button_mask & MOUSE_BUTTON_MASK_MIDDLE:
			_pan_camera(motion.relative)
		return
	if not event is InputEventMouseButton:
		return
	var mouse_event := event as InputEventMouseButton

	if mouse_event.pressed and mouse_event.button_index == MOUSE_BUTTON_WHEEL_UP:
		camera.size = maxf(MIN_CAMERA_SIZE, camera.size - ZOOM_STEP)
		return
	if mouse_event.pressed and mouse_event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
		camera.size = minf(MAX_CAMERA_SIZE, camera.size + ZOOM_STEP)
		return

	if mouse_event.button_index == MOUSE_BUTTON_LEFT:
		if _placement_kind != "":
			if mouse_event.pressed:
				_try_placement(mouse_event.position)
			return
		if mouse_event.pressed:
			_left_pressed = true
			_left_press_position = mouse_event.position
			_select_at(mouse_event.position, mouse_event.shift_pressed)
		elif _left_pressed:
			_left_pressed = false
			if _left_press_position.distance_to(mouse_event.position) >= DRAG_THRESHOLD:
				_select_box(_left_press_position, mouse_event.position, mouse_event.shift_pressed)
		return

	if not mouse_event.pressed:
		return
	if mouse_event.button_index == MOUSE_BUTTON_RIGHT:
		_issue_context_command(mouse_event.position)

func _handle_key(event: InputEventKey) -> void:
	if not event.pressed or event.echo:
		return
	if event.keycode == KEY_ESCAPE:
		if _placement_kind != "":
			_cancel_placement()
		elif _session_phase == "Playing":
			GrusBridge.set_paused(true)
		elif _session_phase == "Paused":
			GrusBridge.set_paused(false)
		return
	if event.keycode == KEY_S:
		_issue_stop()
		return
	if event.keycode == KEY_A:
		if _playing() and not selected_ids.is_empty():
			_attack_move_armed = true
			command_status.text = "Attack-move: right-click a ground target"
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
	selected_building_id = -1
	_apply_selection()

func _pan_camera(relative: Vector2) -> void:
	var viewport_height := maxf(get_viewport().get_visible_rect().size.y, 1.0)
	var world_per_pixel := camera.size / viewport_height
	camera.position.x -= relative.x * world_per_pixel
	camera.position.z -= relative.y * world_per_pixel

func _select_at(screen_position: Vector2, additive: bool) -> void:
	var nearest := _nearest_view("unit_views", screen_position, 1)
	if nearest != null:
		if not additive:
			selected_ids.clear()
		selected_building_id = -1
		var id := int(nearest.get_meta("unit_id", -1))
		if id > 0 and not selected_ids.has(id):
			selected_ids.append(id)
		_apply_selection()
		return
	var building := _nearest_view("building_views", screen_position, 1)
	if building != null:
		selected_ids.clear()
		selected_building_id = int(building.get_meta("building_id", -1))
		_apply_selection()
		return
	if not additive:
		selected_ids.clear()
	selected_building_id = -1
	_apply_selection()

func _select_box(start: Vector2, finish: Vector2, additive: bool) -> void:
	var top_left := Vector2(minf(start.x, finish.x), minf(start.y, finish.y))
	var size := Vector2(absf(finish.x - start.x), absf(finish.y - start.y))
	var selection_rect := Rect2(top_left, size)

	if not additive:
		selected_ids.clear()
	selected_building_id = -1
	for unit in _friendly_units():
		if camera.is_position_behind(unit.global_position):
			continue
		if not selection_rect.has_point(camera.unproject_position(unit.global_position)):
			continue
		var id := int(unit.get_meta("unit_id", -1))
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
	if selected_ids.is_empty():
		_attack_move_armed = false

func _issue_context_command(screen_position: Vector2) -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if _placement_kind != "":
		_cancel_placement()
		return
	var armed := _attack_move_armed
	_attack_move_armed = false
	# An armed attack-move is a modal order: it outranks every contextual
	# pick (villager gather/construction, enemy attack, rally, move).
	if armed and not selected_ids.is_empty():
		var target = _ground_target(screen_position)
		if target != null and GrusBridge.attack_move_units(PackedInt32Array(selected_ids), target):
			command_status.text = "Attack-move queued"
		else:
			command_status.text = "Attack-move rejected"
		return
	var villagers := _selected_villager_ids()
	if not villagers.is_empty():
		var resource_id := _gatherable_resource_id(screen_position)
		if resource_id > 0:
			if GrusBridge.gather(PackedInt32Array(villagers), resource_id):
				command_status.text = "Gather command queued"
			else:
				command_status.text = "Gather command rejected"
			return
		var building := _nearest_view("building_views", screen_position)
		if building != null and int(building.get_meta("team_id", -1)) == 1:
			var building_id := int(building.get_meta("building_id", -1))
			if not _building_complete(building_id):
				if GrusBridge.resume_construction(villagers[0], building_id):
					command_status.text = "Construction resume queued"
				else:
					command_status.text = "Construction resume rejected"
				return
	# A right-click on an enemy unit/building attacks it with the whole
	# selection; the sim rejects noncombatants per unit.
	var enemy := _nearest_enemy_view(screen_position)
	if enemy != null and not selected_ids.is_empty():
		var target_kind := "unit" if enemy.is_in_group("unit_views") else "building"
		var target_id := int(enemy.get_meta("unit_id", enemy.get_meta("building_id", -1)))
		if GrusBridge.attack_units(PackedInt32Array(selected_ids), target_kind, target_id):
			command_status.text = "Attack command queued"
		else:
			command_status.text = "Attack command rejected"
		return
	if _producer_kinds.has(_selected_producer_kind()):
		var target = _ground_target(screen_position)
		if target != null:
			if GrusBridge.set_rally(selected_building_id, floori(target.x), floori(target.y)):
				command_status.text = "Rally point queued"
			else:
				command_status.text = "Rally point rejected"
			return
	_issue_move(screen_position)

func _issue_move(screen_position: Vector2) -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
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

func _issue_stop() -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if selected_ids.is_empty():
		return
	if GrusBridge.stop_units(PackedInt32Array(selected_ids)):
		command_status.text = "Stop command queued"
	else:
		command_status.text = "Stop command rejected"

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

func _nearest_view(group: String, screen_position: Vector2, team_filter := 0) -> Node3D:
	var nearest: Node3D = null
	var nearest_distance := CLICK_RADIUS
	for node in get_tree().get_nodes_in_group(group):
		if team_filter != 0 and int(node.get_meta("team_id", -1)) != team_filter:
			continue
		var view := node as Node3D
		if view == null or camera.is_position_behind(view.global_position):
			continue
		var distance := _view_distance(view, screen_position)
		if distance <= nearest_distance:
			nearest = view
			nearest_distance = distance
	return nearest

## Enemy picking reuses the existing _nearest_view filter and distance
## tie-break across both view groups.
func _nearest_enemy_view(screen_position: Vector2) -> Node3D:
	var unit := _nearest_view("unit_views", screen_position, 2)
	var building := _nearest_view("building_views", screen_position, 2)
	if unit == null:
		return building
	if building == null:
		return unit
	return unit if _view_distance(unit, screen_position) <= _view_distance(building, screen_position) else building

func _view_distance(view: Node3D, screen_position: Vector2) -> float:
	return camera.unproject_position(view.global_position).distance_to(screen_position)

func _friendly_units() -> Array[Node3D]:
	var result: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == 1:
			result.append(node as Node3D)
	return result

func _unit_kind(unit_id: int) -> String:
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("unit_id", -1)) == unit_id:
			return str(node.get_meta("unit_kind", ""))
	return ""

func _unit_view(unit_id: int) -> Node3D:
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("unit_id", -1)) == unit_id:
			return node as Node3D
	return null

func _on_start_pressed() -> void:
	GrusBridge.start_match()

func _on_pause_pressed() -> void:
	GrusBridge.set_paused(true)

func _on_resume_pressed() -> void:
	GrusBridge.set_paused(false)

func _on_restart_pressed() -> void:
	_restart_match()

func _on_quit_pressed() -> void:
	get_tree().quit(0)

## Restart clears every transient Godot-side fact while the bridge reseeds:
## selection, control groups, placement, attack-move arming, and live combat
## effects. No dead stable ids survive the reset.
func _restart_match() -> void:
	GrusBridge.restart_match()
	selected_ids.clear()
	selected_building_id = -1
	_control_groups.clear()
	_idle_cursor = 0
	_attack_move_armed = false
	_cancel_placement()
	_fx.clear_all()

func _selected_villager_ids() -> Array[int]:
	var villagers: Array[int] = []
	for id in selected_ids:
		if _unit_kind(id) == "Villager":
			villagers.append(id)
	villagers.sort()
	return villagers

func _lowest_selected_villager_id() -> int:
	var villagers := _selected_villager_ids()
	return villagers[0] if not villagers.is_empty() else 0

func _gatherable_resource_id(screen_position: Vector2) -> int:
	var resource := _nearest_view("resource_views", screen_position)
	var building := _nearest_view("building_views", screen_position)
	if building != null and (int(building.get_meta("team_id", -1)) != 1 \
			or not building.has_meta("resource_id")):
		building = null
	# A Farm body under the cursor outranks a neighbouring source when it is
	# the nearer view — the building's footprint centre is the click target.
	if building != null and (resource == null \
			or _view_distance(building, screen_position) < _view_distance(resource, screen_position)):
		return int(building.get_meta("resource_id", -1))
	if resource != null:
		return int(resource.get_meta("resource_id", -1))
	return -1

func _building_complete(building_id: int) -> bool:
	if building_id <= 0:
		return true
	var snapshot: Dictionary = GrusBridge.building_snapshot(building_id)
	if snapshot.is_empty():
		return true
	return bool(snapshot.get("complete", true))

func _selected_producer_kind() -> String:
	if selected_building_id <= 0:
		return ""
	return str(GrusBridge.building_snapshot(selected_building_id).get("kind", ""))

func _on_build_pressed(kind: String) -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	_placement_kind = kind
	command_status.text = "Placing %s — left-click to place, right-click or Esc to cancel" % kind

func _cancel_placement() -> void:
	_placement_kind = ""
	_preview_box.visible = false

func _update_placement_preview(screen_position: Vector2) -> void:
	var target = _ground_target(screen_position)
	if target == null:
		_preview_box.visible = false
		return
	var preview: Dictionary = GrusBridge.placement_preview(
		_lowest_selected_villager_id(), _placement_kind, floori(target.x), floori(target.y))
	if preview.is_empty():
		_preview_box.visible = false
		return
	var width := float(preview.get("width", 1))
	var height := float(preview.get("height", 1))
	var anchor_x := float(preview.get("anchor_x", 0))
	var anchor_y := float(preview.get("anchor_y", 0))
	_preview_box.visible = true
	_preview_box.scale = Vector3(width, 1.0, height)
	_preview_box.position = Vector3(anchor_x + width / 2.0, 0.2, anchor_y + height / 2.0)
	var material := _preview_box.material_override as StandardMaterial3D
	if bool(preview.get("valid", false)):
		material.albedo_color = Color(0.3, 0.85, 0.3, 0.4)
	else:
		material.albedo_color = Color(0.9, 0.25, 0.2, 0.4)

func _try_placement(screen_position: Vector2) -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	var target = _ground_target(screen_position)
	if target == null:
		return
	var builder := _lowest_selected_villager_id()
	if builder <= 0:
		command_status.text = "Select a villager to build"
		return
	var anchor_x := floori(target.x)
	var anchor_y := floori(target.y)
	var preview: Dictionary = GrusBridge.placement_preview(builder, _placement_kind, anchor_x, anchor_y)
	if not bool(preview.get("valid", false)):
		command_status.text = "Placement blocked (code %d)" % int(preview.get("reject_code", -1))
		return
	if GrusBridge.place_building(builder, _placement_kind, anchor_x, anchor_y):
		command_status.text = "Placement queued"
	else:
		command_status.text = "Placement rejected"
	_cancel_placement()

func _on_train_pressed(kind: String) -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if selected_building_id <= 0:
		command_status.text = "Select a production building first"
		return
	if GrusBridge.enqueue_unit(selected_building_id, kind):
		command_status.text = "%s training queued" % kind
	else:
		command_status.text = "Training rejected"

func _on_age_pressed() -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if selected_building_id <= 0:
		command_status.text = "Select the Town Center to advance"
		return
	if GrusBridge.enqueue_age_up(selected_building_id):
		command_status.text = "Age advancement queued"
	else:
		command_status.text = "Age advancement rejected"

func _on_idle_pressed() -> void:
	var economy: Dictionary = GrusBridge.economy_snapshot()
	var ids: PackedInt32Array = economy.get("idle_worker_ids", PackedInt32Array())
	if ids.is_empty():
		command_status.text = "No idle workers"
		return
	if _idle_cursor >= ids.size():
		_idle_cursor = 0
	var worker_id := int(ids[_idle_cursor])
	_idle_cursor += 1
	selected_ids.clear()
	selected_ids.append(worker_id)
	selected_building_id = -1
	_apply_selection()
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("unit_id", -1)) != worker_id:
			continue
		var view := node as Node3D
		camera.position.x = view.global_position.x
		camera.position.z = view.global_position.z
		break
	command_status.text = "Selected idle villager #%d" % worker_id
