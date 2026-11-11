extends Node3D

const CLICK_RADIUS := 20.0
const DRAG_THRESHOLD := 6.0
const MIN_CAMERA_SIZE := 24.0
const MAX_CAMERA_SIZE := 120.0
const ZOOM_STEP := 6.0

@onready var camera: Camera3D = $Camera3D
@onready var command_status: Label = $HUD/CommandStatus
## Rendering lives on the HUD CanvasLayer's own script; it holds no gameplay
## state, so the controller stays the single owner of every snapshot read.
@onready var _hud = $HUD
@onready var analyze_button: Button = $HUD/ResearchPanel/AnalyzeButton
@onready var idle_button: Button = $HUD/EconomyPanel/IdleButton
@onready var _build_grid: Container = $HUD/CommandPanel/BuildGrid
@onready var _train_grid: Container = $HUD/CommandPanel/TrainGrid
@onready var _order_grid: Container = $HUD/CommandPanel/OrderGrid
@onready var _preview_box: MeshInstance3D = $PlacementPreview
@onready var _fx: Node3D = $CombatFx
@onready var start_button: Button = $HUD/SessionPanel/StartButton
@onready var resume_button: Button = $HUD/SessionPanel/ResumeButton
@onready var restart_button: Button = $HUD/SessionPanel/RestartButton
@onready var quit_button: Button = $HUD/SessionPanel/QuitButton
@onready var pause_button: Button = $HUD/PauseButton
## The drawer script on the existing panel owns its visual arming state and
## emits the two action intents; Tab/Esc, the Playing gate, dispatch and the
## command status stay here.
@onready var _ug_drawer = $HUD/UndergroundPanel
## The Excavate button is static scene content, so the controller owns its
## pressed wiring like every other static Button: it connects straight to the
## drawer's existing intent signal.
@onready var _underground_excavate: Button = $HUD/UndergroundPanel/ExcavateButton

var selected_ids: Array[int] = []
var selected_building_id := -1
var _control_groups: Dictionary = {}
var _feedback_revision := -1
var _left_press_position := Vector2.ZERO
var _left_pressed := false
var _placement_kind := ""
var _placement_generation := 0
var _pending_placement_requests: Array[Dictionary] = []
var _idle_cursor := 0
var _session_phase := ""
var _unit_kinds: Dictionary = {}
var _previous_unit_kinds: Dictionary = {}
var _building_kinds: Dictionary = {}
## One controller-owned modal order: "", "attack_move", "move", "gather",
## "rally". Build placement stays a separate `_placement_kind` because it has
## its own preview/queued-placement lifecycle.
var _armed_order := ""
## Producer building kinds come from the Rust catalogue via
## catalogue_snapshot — GDScript hardcodes no gameplay data.
var _producer_kinds: Array[String] = []
## Tick/phase markers behind the per-tick refresh gate: the snapshot can
## only change inside a fixed tick, and the lock line only on a phase
## change, so anything fresher is a wasted rebuild.
var _ug_last_tick := -1
var _ug_last_phase := ""

func _ready() -> void:
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var producers := {}
	var units: Dictionary = catalogue.get("units", {})
	for kind in units:
		producers[str(units[kind].get("producer", ""))] = true
	_producer_kinds.assign(producers.keys())
	_producer_kinds.sort()
	_feedback_revision = int(GrusBridge.command_feedback_revision())
	command_status.text = str(GrusBridge.command_feedback())
	_ug_drawer.excavate_requested.connect(_on_excavate_requested)
	_ug_drawer.room_requested.connect(_on_room_requested)
	_underground_excavate.pressed.connect(_ug_drawer.excavate_requested.emit)
	# The drawer creates its kind toggles in its own _ready (children ready
	# before this node), so observe them here for the controller-owned hint —
	# no third drawer signal and no duplicated armed state.
	for button in _ug_drawer.get_node("KindsBox").get_children():
		button.toggled.connect(_on_underground_kind_toggled.bind(str(button.get_meta("kind"))))
	for button in _build_grid.get_children():
		button.pressed.connect(_on_build_pressed.bind(str(button.get_meta("kind"))))
	for button in _train_grid.get_children():
		button.pressed.connect(_on_train_pressed.bind(str(button.get_meta("kind"))))
	for button in _order_grid.get_children():
		if button is Button:
			button.pressed.connect(_on_order_pressed.bind(str(button.get_meta("order", ""))))
	analyze_button.pressed.connect(_on_analyze_pressed)
	idle_button.pressed.connect(_on_idle_pressed)
	start_button.pressed.connect(_on_start_pressed)
	resume_button.pressed.connect(_on_resume_pressed)
	pause_button.pressed.connect(_on_pause_pressed)
	restart_button.pressed.connect(_on_restart_pressed)
	quit_button.pressed.connect(_on_quit_pressed)

func _process(_delta: float) -> void:
	_unit_kinds = _view_kinds("unit_views", "unit_id", "unit_kind")
	_building_kinds = _view_kinds("building_views", "building_id", "building_kind")
	var placement_feedback_handled := _consume_placement_results()
	var revision := int(GrusBridge.command_feedback_revision())
	if revision != _feedback_revision:
		_feedback_revision = revision
		if not placement_feedback_handled:
			command_status.text = str(GrusBridge.command_feedback())
	_refresh_session()
	_previous_unit_kinds = _unit_kinds
	_reconcile_selection()
	_refresh_hud()
	if _ug_drawer.visible \
			and (int(GrusBridge.sim_tick()) != _ug_last_tick or _session_phase != _ug_last_phase):
		_refresh_underground_panel()

func _view_kinds(group: String, id_key: String, kind_key: String) -> Dictionary:
	var kinds := {}
	for view in get_tree().get_nodes_in_group(group):
		kinds[int(view.get_meta(id_key, -1))] = str(view.get_meta(kind_key, ""))
	return kinds

func _consume_placement_results() -> bool:
	var unresolved: Array[Dictionary] = []
	var handled_feedback := false
	for request in _pending_placement_requests:
		var result: Dictionary = GrusBridge.take_placement_command_result(int(request.get("id", -1)))
		if not bool(result.get("complete", false)):
			unresolved.append(request)
			continue
		if int(request.get("generation", -1)) != _placement_generation \
				or str(request.get("kind", "")) != _placement_kind:
			# A dropped result must mask only its own echo: the shared channel
			# keeps the last command's text per tick, so identical text is this
			# stale acceptance while a different text is unrelated feedback
			# (a move/route rejection) that still applies.
			if str(result.get("feedback", "")) == str(GrusBridge.command_feedback()):
				handled_feedback = true
			continue
		handled_feedback = true
		command_status.text = str(result.get("feedback", "Placement rejected"))
		if bool(result.get("accepted", false)) \
				and _placement_kind != "Wall" and _placement_kind != "Gate":
			_cancel_placement()
	_pending_placement_requests = unresolved
	return handled_feedback

## Reads the session once per frame: drives the overlay and drains
## current-tick combat events while Playing — plus once on the first Result
## frame, whose buffer still holds the settle tick's killing blow (strike
## records the hit and resolves the match in the same fixed tick). Paused
## leftovers stay unreadable: those stale events must never replay as fresh
## effects.
func _refresh_session() -> void:
	var snap: Dictionary = GrusBridge.session_snapshot()
	var previous_phase := _session_phase
	_session_phase = str(snap.get("phase", ""))
	if _session_phase != "Playing":
		_armed_order = ""
	_fx.set_wind_phase(_session_phase)
	_hud.refresh_session(_session_phase, int(snap.get("winner_team", -1)))
	var result_frame := _session_phase == "Result" and previous_phase != "Result"
	if _session_phase != "Playing" and not result_frame:
		return
	var events: Array = GrusBridge.drain_combat_events()
	for event in events:
		_dispatch_combat_event(event)

## Gameplay orders only ever leave Godot while the session is Playing.
func _playing() -> bool:
	return _session_phase == "Playing"

## Cosmetics only: reads the event payload, never touches combat state.
func _dispatch_combat_event(event: Dictionary) -> void:
	var hit := Vector3(float(event.get("x", 0.0)), 1.0, float(event.get("y", 0.0)))
	if bool(event.get("ranged", false)):
		var attacker: Node3D = null
		var attacker_id := int(event.get("attacker", -1))
		match str(event.get("attacker_kind", "")):
			"unit":
				attacker = _unit_view(attacker_id)
			"building":
				attacker = _building_view(attacker_id)
		if attacker != null:
			var attacker_kind := str(event.get("attacker_kind", ""))
			_fx.spawn_tracer(_attacker_muzzle_position(attacker, attacker_kind), hit)
	_fx.spawn_hit(hit)
	if bool(event.get("killed", false)):
		_fx.spawn_death(hit)
		if str(event.get("target_kind", "")) == "unit":
			var target_id := int(event.get("target_id", -1))
			var target_kind := str(_unit_kinds.get(target_id, _previous_unit_kinds.get(target_id, "")))
			if target_kind == "Swarmer":
				_fx.play_cue("swarmer_death")
	var attacker_id := int(event.get("attacker", 0))
	if str(event.get("attacker_kind", "")) == "building" \
			and str(_building_kinds.get(attacker_id, "")) == "GunTurret":
		_fx.play_cue("gun_turret_fire")
	elif attacker_id > 0:
		_fx.play_hit_cue()

func _attacker_muzzle_position(attacker: Node3D, attacker_kind: String) -> Vector3:
	var height := 0.8
	if attacker_kind == "building":
		# Heights follow the weapons authored in scripts/build_winter_assets.py.
		match str(attacker.get_meta("building_kind", "")):
			"GunTurret":
				height = 0.62
			"FlameTower":
				height = 0.9
			"CrystalLance":
				height = 1.8
	return attacker.global_position + Vector3(0.0, height, 0.0)

## Selection truth is the live view tree, never the cosmetic event buffer:
## step_combat clears CombatEvents on every non-Result tick, so a kill can
## be wiped before this drain ever sees it. Once per frame, drop stable ids
## with no live view from the selection and every control group — then
## reconcile rings and attack-move arming only when something changed.
func _reconcile_selection() -> void:
	var live_ids := {}
	for unit in _friendly_units():
		live_ids[int(unit.get_meta("unit_id", -1))] = true
	var changed := false
	for i in range(selected_ids.size() - 1, -1, -1):
		if not live_ids.has(selected_ids[i]):
			selected_ids.remove_at(i)
			changed = true
	for group in _control_groups:
		var ids: Array = _control_groups[group]
		for i in range(ids.size() - 1, -1, -1):
			if not live_ids.has(int(ids[i])):
				ids.remove_at(i)
				changed = true
	if selected_building_id > 0:
		var view := _building_view(selected_building_id)
		if view == null or not view.is_visible_in_tree():
			# A dead OR fog-hidden enemy building is dropped from the
			# selection the moment it stops being presented.
			selected_building_id = -1
			changed = true
	if changed:
		_apply_selection()

## One read of each HUD snapshot per frame, handed straight to the HUD view;
## the controller keeps the selection/session context the render needs and
## never keeps a second copy of the rendered values.
func _refresh_hud() -> void:
	_hud.refresh_economy(GrusBridge.economy_snapshot())
	_hud.refresh_wave(GrusBridge.wave_snapshot())
	var research: Dictionary = GrusBridge.research_snapshot()
	var building: Dictionary = {}
	var building_health := -1.0
	if selected_building_id > 0:
		building = GrusBridge.building_snapshot(selected_building_id)
		var building_view := _building_view(selected_building_id)
		if building_view != null:
			building_health = float(building_view.call("health_ratio"))
	var unit_task: Dictionary = {}
	var unit_health := -1.0
	if selected_ids.size() == 1:
		var unit_id := selected_ids[0]
		unit_task = GrusBridge.unit_task_snapshot(unit_id)
		var unit_view := _unit_view(unit_id)
		if unit_view != null:
			unit_health = float(unit_view.call("health_ratio"))
	_hud.refresh_selection(selected_ids, selected_building_id, building, _unit_kinds,
		unit_task, unit_health, building_health)
	_hud.refresh_orders(_armed_order, _playing())
	_hud.refresh_research_controls(research, _playing(), _research_center_id() > 0)

## Tab and Esc-while-open route through `_input` (not `_unhandled_input`)
## and mark the event handled: `ui_focus_next` would otherwise consume Tab
## once a panel button holds keyboard focus, and the open panel owns Esc
## ahead of the pause shortcut. Every Tab event — press, echo and release
## — is handled, since holding Tab repeats echo events that would
## otherwise walk GUI focus onto the panel's buttons. Every other key
## keeps its existing `_unhandled_input` route.
func _input(event: InputEvent) -> void:
	if not event is InputEventKey:
		return
	var key := event as InputEventKey
	if key.keycode == KEY_TAB:
		if key.pressed and not key.echo:
			_toggle_underground_panel()
		get_viewport().set_input_as_handled()
		return
	if not key.pressed or key.echo:
		return
	if key.keycode == KEY_ESCAPE and _ug_drawer.visible:
		_close_underground_panel()
		get_viewport().set_input_as_handled()

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
		elif _armed_order != "":
			_armed_order = ""
			command_status.text = "Order canceled"
		elif _session_phase == "Playing":
			GrusBridge.set_paused(true)
		elif _session_phase == "Paused":
			GrusBridge.set_paused(false)
		return
	if event.keycode == KEY_S:
		_armed_order = ""
		_issue_stop()
		return
	if event.keycode == KEY_A:
		if _playing() and not selected_ids.is_empty():
			if _placement_kind != "":
				_cancel_placement()
			_armed_order = "attack_move"
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
	var building := _building_at(screen_position, 1)
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
	_clear_invalid_armed_order()

## One modal order outlives a selection only while its context still exists:
## units for move/attack-move/gather (workers for Gather), a selected
## producer for Rally. Lost selection/context clears it, never a hidden fallback.
func _clear_invalid_armed_order() -> void:
	match _armed_order:
		"":
			return
		"rally":
			if not _producer_kinds.has(_selected_producer_kind()):
				_armed_order = ""
		"gather":
			if _selected_worker_ids().is_empty():
				_armed_order = ""
		_:
			if selected_ids.is_empty():
				_armed_order = ""

func _issue_context_command(screen_position: Vector2) -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if _placement_kind != "":
		_cancel_placement()
		return
	# An armed order is modal: it outranks every contextual pick and is
	# consumed by this one right-click, even when its target is invalid.
	if _armed_order != "":
		var armed := _armed_order
		_armed_order = ""
		_dispatch_armed_order(armed, screen_position)
		return
	var workers := _selected_worker_ids()
	if not workers.is_empty():
		var resource_id := _gatherable_resource_id(screen_position)
		if resource_id > 0:
			if GrusBridge.gather(PackedInt32Array(workers), resource_id):
				command_status.text = "Gather command queued"
			else:
				command_status.text = "Gather command rejected"
			return
		var building := _building_at(screen_position)
		if building != null and int(building.get_meta("team_id", -1)) == 1:
			var building_id := int(building.get_meta("building_id", -1))
			if not _building_complete(building_id):
				if GrusBridge.resume_construction(workers[0], building_id):
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

## The one modal-order dispatch table. Each branch reuses the existing bridge
## operation and never falls through to a generic contextual pick.
func _dispatch_armed_order(order: String, screen_position: Vector2) -> void:
	match order:
		"attack_move":
			var target = _ground_target(screen_position)
			if target == null or selected_ids.is_empty():
				command_status.text = "Attack-move rejected"
				return
			if GrusBridge.attack_move_units(PackedInt32Array(selected_ids), target):
				command_status.text = "Attack-move queued"
			else:
				command_status.text = "Attack-move rejected"
		"move":
			_issue_move(screen_position)
		"gather":
			var workers := _selected_worker_ids()
			var resource_id := _gatherable_resource_id(screen_position)
			if workers.is_empty() or resource_id <= 0:
				command_status.text = "Gather command rejected"
				return
			if GrusBridge.gather(PackedInt32Array(workers), resource_id):
				command_status.text = "Gather command queued"
			else:
				command_status.text = "Gather command rejected"
		"rally":
			var target = _ground_target(screen_position)
			if target == null or not _producer_kinds.has(_selected_producer_kind()):
				command_status.text = "Rally point rejected"
				return
			if GrusBridge.set_rally(selected_building_id, floori(target.x), floori(target.y)):
				command_status.text = "Rally point queued"
			else:
				command_status.text = "Rally point rejected"
		_:
			command_status.text = "Order canceled"

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

func _nearest_view(group: String, screen_position: Vector2, team_filter := 0, radius := CLICK_RADIUS) -> Node3D:
	var nearest: Node3D = null
	var nearest_distance := radius
	for node in get_tree().get_nodes_in_group(group):
		if not node.is_visible_in_tree():
			# Fog is a gameplay contract, not a rendering detail: a hidden view
			# (hidden enemy, unexplored resource) must never be picked.
			continue
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

## Compare the nearest eligible unit with the silhouette-picked building.
func _nearest_enemy_view(screen_position: Vector2) -> Node3D:
	var unit := _nearest_view("unit_views", screen_position, 2)
	var building := _building_at(screen_position, 2)
	if unit == null:
		return building
	if building == null:
		return unit
	return unit if _view_distance(unit, screen_position) <= _view_distance(building, screen_position) else building

func _view_distance(view: Node3D, screen_position: Vector2) -> float:
	return camera.unproject_position(view.global_position).distance_to(screen_position)

## Building picking tests the projected silhouette of the view's active
## Model (health bar excluded) via ray-triangle intersection, with closest-hit
## depth sorting along the camera ray so foreground geometry wins over background
## structures. The center-radius floor stays as the fallback when no silhouette
## is hit (e.g. for small buildings at wide zoom or unstyled views). Hidden
## (fogged) views are never picked.
func _building_at(screen_position: Vector2, team_filter := 0) -> Node3D:
	var ray_origin := camera.project_ray_origin(screen_position)
	var ray_direction := camera.project_ray_normal(screen_position)
	var nearest_hit_view: Node3D = null
	var nearest_hit_depth := INF
	var fallback: Node3D = null
	var fallback_distance := CLICK_RADIUS
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		if view == null or not view.is_visible_in_tree() \
				or camera.is_position_behind(view.global_position):
			continue
		if team_filter != 0 and int(view.get_meta("team_id", -1)) != team_filter:
			continue
		var center_px := camera.unproject_position(view.global_position)
		var distance := center_px.distance_to(screen_position)
		if distance <= fallback_distance:
			fallback = view
			fallback_distance = distance
		var model := view.get_node_or_null("Model") as Node3D
		if model == null:
			continue
		var rect := _projected_art_rect(view, model)
		if not rect.has_point(screen_position):
			continue
		var depth := _model_ray_hit_depth(model, ray_origin, ray_direction)
		if depth >= 0.0 and depth < nearest_hit_depth:
			nearest_hit_view = view
			nearest_hit_depth = depth
	return nearest_hit_view if nearest_hit_view != null else fallback

func _model_ray_hit_depth(model: Node3D, ray_origin: Vector3, ray_dir: Vector3) -> float:
	var min_depth := INF
	for node in model.find_children("*", "MeshInstance3D", true, false):
		var mesh_instance := node as MeshInstance3D
		if mesh_instance == null or mesh_instance.mesh == null:
			continue
		var triangles := mesh_instance.mesh.generate_triangle_mesh()
		if triangles == null:
			continue
		var inverse := mesh_instance.global_transform.affine_inverse()
		var hit := triangles.intersect_ray(inverse * ray_origin, (inverse.basis * ray_dir).normalized())
		if not hit.is_empty():
			var world_hit: Vector3 = mesh_instance.global_transform * hit["position"]
			min_depth = minf(min_depth, (world_hit - ray_origin).dot(ray_dir))
	return min_depth if min_depth < INF else -1.0

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

func _building_view(building_id: int) -> Node3D:
	for node in get_tree().get_nodes_in_group("building_views"):
		if int(node.get_meta("building_id", -1)) == building_id:
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
## selection, control groups, placement, attack-move arming, the armed
## underground room kind, and live combat effects. No dead stable ids
## survive the reset.
func _restart_match() -> void:
	GrusBridge.restart_match()
	selected_ids.clear()
	selected_building_id = -1
	_control_groups.clear()
	_idle_cursor = 0
	_armed_order = ""
	_ug_drawer.reset()
	_pending_placement_requests.clear()
	_cancel_placement()
	_fx.clear_all()

func _selected_worker_ids() -> Array[int]:
	var workers: Array[int] = []
	for id in selected_ids:
		if _unit_kind(id) == "Worker":
			workers.append(id)
	workers.sort()
	return workers

func _lowest_selected_worker_id() -> int:
	var workers := _selected_worker_ids()
	return workers[0] if not workers.is_empty() else 0

func _gatherable_resource_id(screen_position: Vector2) -> int:
	# Only standalone finite sources are gatherable; buildings never carry
	# resource metadata.
	var resource := _resource_at(screen_position)
	if resource != null:
		return int(resource.get_meta("resource_id", -1))
	return -1

## A resource is gathered by clicking anywhere on its projected art bounds
## — the silhouette itself, including height, at any window size or zoom —
## or inside the projected-cell radius floor that keeps zoomed-out picking
## forgiving. Nearest center wins on overlap, like _nearest_view.
func _resource_at(screen_position: Vector2) -> Node3D:
	var nearest: Node3D = null
	var nearest_distance := INF
	for node in get_tree().get_nodes_in_group("resource_views"):
		var view := node as Node3D
		if view == null or not view.is_visible_in_tree() \
				or camera.is_position_behind(view.global_position):
			continue
		var center_px := camera.unproject_position(view.global_position)
		var distance := center_px.distance_to(screen_position)
		var hit := distance <= _resource_pick_radius() \
				or _projected_art_rect(view).has_point(screen_position)
		if hit and distance < nearest_distance:
			nearest = view
			nearest_distance = distance
	return nearest

func _resource_pick_radius() -> float:
	var viewport_height := maxf(get_viewport().get_visible_rect().size.y, 1.0)
	var pixels_per_unit := viewport_height / maxf(camera.size, 0.001)
	return maxf(CLICK_RADIUS, pixels_per_unit * sqrt(0.5) + 1.0)

func _projected_art_rect(view: Node3D, art_root: Node3D = null) -> Rect2:
	var bounds := _view_art_bounds(view, art_root)
	var rect := Rect2()
	var first := true
	for x in [bounds.position.x, bounds.end.x]:
		for y in [bounds.position.y, bounds.end.y]:
			for z in [bounds.position.z, bounds.end.z]:
				var point := camera.unproject_position(view.global_transform * Vector3(x, y, z))
				rect = Rect2(point, Vector2.ZERO) if first else rect.expand(point)
				first = false
	return rect

## art_root scopes the mesh scan — passing a building view's active Model
## keeps its sibling HealthBar out of the pick silhouette, while a missing
## root still walks the whole view (resource models are the view).
func _view_art_bounds(view: Node3D, art_root: Node3D = null) -> AABB:
	if art_root != null and art_root.has_meta("_cached_art_bounds"):
		return art_root.get_meta("_cached_art_bounds") as AABB
	var bounds := AABB()
	var first := true
	for node in (art_root if art_root != null else view).find_children("*", "MeshInstance3D", true, false):
		var mesh := node as MeshInstance3D
		if mesh == null or mesh.mesh == null:
			continue
		var local: AABB = view.global_transform.affine_inverse() * mesh.global_transform * mesh.get_aabb()
		bounds = local if first else bounds.merge(local)
		first = false
	if bounds.size.length() < 0.001:
		# No art yet (kind meta arrives a frame late): fall back to the cell.
		bounds = AABB(Vector3(-0.5, 0.0, -0.5), Vector3(1.0, 1.0, 1.0))
	if art_root != null:
		art_root.set_meta("_cached_art_bounds", bounds)
	return bounds

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
	# Placement and a modal order are mutually exclusive targeting modes.
	_armed_order = ""
	_placement_generation += 1
	_placement_kind = kind
	command_status.text = "Placing %s — left-click to place, right-click or Esc to cancel" % kind

## A real order-tile click arms the one modal order after checking its
## context; Stop runs immediately and Cancel clears placement/order without
## enqueueing a gameplay command.
func _on_order_pressed(order: String) -> void:
	if order == "cancel":
		if _placement_kind != "":
			_cancel_placement()
			command_status.text = "Placement canceled"
		elif _armed_order != "":
			_armed_order = ""
			command_status.text = "Order canceled"
		return
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if order == "stop":
		_armed_order = ""
		_issue_stop()
		return
	if order in ["move", "attack_move"] and selected_ids.is_empty():
		command_status.text = "Select units first"
		return
	if order == "gather" and _selected_worker_ids().is_empty():
		command_status.text = "Select workers to gather"
		return
	if order == "rally" and not _producer_kinds.has(_selected_producer_kind()):
		command_status.text = "Select a production building first"
		return
	# Entering a targeting order leaves placement first; the reverse entry
	# (`_on_build_pressed`) clears a modal order. The two never coexist.
	if _placement_kind != "":
		_cancel_placement()
	_armed_order = order
	match order:
		"move":
			command_status.text = "Move: right-click a ground target"
		"attack_move":
			command_status.text = "Attack-move: right-click a ground target"
		"gather":
			command_status.text = "Gather: right-click a resource"
		"rally":
			command_status.text = "Rally: right-click a ground target"

func _cancel_placement() -> void:
	_placement_generation += 1
	_placement_kind = ""
	_preview_box.visible = false

func _update_placement_preview(screen_position: Vector2) -> void:
	var target = _ground_target(screen_position)
	if target == null:
		_preview_box.visible = false
		return
	var preview: Dictionary = GrusBridge.placement_preview(
		_lowest_selected_worker_id(), _placement_kind, floori(target.x), floori(target.y))
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
	var builder := _lowest_selected_worker_id()
	if builder <= 0:
		command_status.text = "Select a worker to build"
		return
	for request in _pending_placement_requests:
		if int(request.get("generation", -1)) == _placement_generation:
			command_status.text = "Previous placement still resolving"
			return
	var anchor_x := floori(target.x)
	var anchor_y := floori(target.y)
	var preview: Dictionary = GrusBridge.placement_preview(builder, _placement_kind, anchor_x, anchor_y)
	if not bool(preview.get("valid", false)):
		command_status.text = "Placement blocked (code %d)" % int(preview.get("reject_code", -1))
		return
	var request_id := int(GrusBridge.next_placement_request_id())
	var kind := _placement_kind
	if GrusBridge.place_building_request(builder, kind, anchor_x, anchor_y, request_id):
		_pending_placement_requests.append({
			"id": request_id,
			"generation": _placement_generation,
			"kind": kind,
		})
		command_status.text = "Placement queued"
	else:
		command_status.text = "Placement rejected"

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

## The live friendly Research Center view id, resolved from the existing
## building_views metadata (kind/team/id) — never a hardcoded building id.
func _research_center_id() -> int:
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		if view == null or not view.is_visible_in_tree():
			continue
		if int(view.get_meta("team_id", -1)) != 1 \
				or str(view.get_meta("building_kind", "")) != "ResearchCenter":
			continue
		var id := int(view.get_meta("building_id", -1))
		if id > 0:
			return id
	return -1

## One Analyze conversion on the live Team-1 Research Center, independent of
## the current selection — the same queued-command/feedback path as every
## other order. The button's enabled state encodes Playing + core presence;
## the sim stays the final validator for ownership, completion and Crystals.
func _on_analyze_pressed() -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	var core_id := _research_center_id()
	if core_id <= 0:
		command_status.text = "Research Center unavailable"
		return
	if GrusBridge.analyze(core_id):
		command_status.text = "Analyze queued"
	else:
		command_status.text = "Analyze rejected"

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
	command_status.text = "Selected idle worker #%d" % worker_id

## Tab toggles the underground panel; the panel itself consumes mouse input
## (Control STOP filter) so clicks never leak into ground selection,
## placement or unit commands.
func _toggle_underground_panel() -> void:
	if _ug_drawer.visible:
		_close_underground_panel()
	else:
		_ug_drawer.visible = true
		# One immediate refresh so the first shown frame is current; the
		# status line keeps the last command feedback — panel state never
		# clobbers it (the Esc hint lives on the panel's own header).
		_refresh_underground_panel()

func _close_underground_panel() -> void:
	_ug_drawer.visible = false

## Dynamic underground refresh, only while the panel is visible and gated by
## the applied-tick counter plus session phase: the snapshot can only change
## inside a fixed tick, so a mid-tick frame never hands the drawer a repeat.
func _refresh_underground_panel() -> void:
	_ug_last_tick = int(GrusBridge.sim_tick())
	_ug_last_phase = _session_phase
	_ug_drawer.refresh(GrusBridge.underground_snapshot(), _session_phase)

## The drawer owns arming and emits no arm signal; the controller observes
## the already-created room-kind toggles only to keep its own status hint.
func _on_underground_kind_toggled(pressed: bool, kind: String) -> void:
	if pressed:
		command_status.text = "%s armed — click an eligible slot" % kind

## One paid excavation intent from the drawer — the same queued-command/
## feedback path as every other order; the sim owns the single-dig rule and
## affordability.
func _on_excavate_requested() -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if GrusBridge.excavate():
		command_status.text = "Excavate queued"
	else:
		command_status.text = "Excavate rejected"

## An eligible empty-slot intent carries the drawer-resolved kind with its
## 1-based level and 0-based slot; Rust revalidates everything authoritatively.
func _on_room_requested(level: int, slot: int, kind: String) -> void:
	if not _playing() or kind == "":
		return
	if GrusBridge.build_room(level, slot, kind):
		command_status.text = "%s queued (L%d slot %d)" % [
			_ug_drawer.display_name(kind), level, slot + 1]
	else:
		command_status.text = "Room command rejected"
