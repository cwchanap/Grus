extends Node3D

const HudHelpers := preload("res://scripts/hud_helpers.gd")
## Static access to building_view.gd's shared sprite alpha cache and
## art-geometry predicate — the one cache used by both health-bar seating
## and camera-ray silhouette picking.
const BuildingView := preload("res://scripts/building_view.gd")
const MinimapView := preload("res://scripts/minimap.gd")

const CLICK_RADIUS := 20.0
const DRAG_THRESHOLD := 6.0
const MIN_CAMERA_SIZE := 24.0
const MAX_CAMERA_SIZE := 120.0
const ZOOM_STEP := 6.0
## Arrow keys and the screen edge pan at this many camera heights per second,
## so the apparent speed stays constant at every zoom level. WASD is not
## used: S (stop) and A (attack-move) are unit orders.
const PAN_SPEED := 0.9
const EDGE_PAN_MARGIN := 3.0
## Normalized (u, v) muzzle coordinates on painted combat-tower sprites
## (top-down texture space, matching BuildingView.sprite_local_from_pixel).
const PAINTED_MUZZLE_UV := {
	"GunTurret": Vector2(0.40, 0.50),
	"FlameTower": Vector2(0.68, 0.44),
	"CrystalLance": Vector2(0.50, 0.14),
}

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
## emits `room_requested`; Tab/Esc, the Playing gate, dispatch and the
## command status stay here.
@onready var _ug_drawer = $HUD/UndergroundPanel
## The Excavate button is static scene content, so the controller connects its
## pressed signal straight to `_on_excavate_requested` like every other static
## Button.
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
## This generation's click-time refusal, kept beside the shared status
## label: the feedback mirror can overwrite `command_status` with an
## unrelated route failure on any later tick, but an armed click's
## pre-dispatch refusal is this placement's own verdict and must survive
## that mirror until the next arm.
var _last_placement_refusal := {}
## Latest completed placement ticket verdict ({id, generation, kind, accepted,
## feedback}), retained across _consume_placement_results so an unrelated route
## failure overwriting `command_status` never hides a rejected ticket from
## downstream observers.
var _last_placement_verdict := {}
var _idle_cursor := 0
var _session_phase := ""
var _session_winner := -2
var _research_center_view: Node3D = null
const QUIT_FALLBACK_SECONDS := 1.0

var _unit_kinds: Dictionary = {}
var _previous_unit_kinds: Dictionary = {}
var _building_kinds: Dictionary = {}
var _unit_views: Dictionary = {}
var _building_views: Dictionary = {}
var _friendly_unit_views: Array[Node3D] = []
var _views_indexed := false
## One controller-owned modal order: "", "attack_move", "move", "gather",
## "rally". Build placement stays a separate `_placement_kind` because it has
## its own preview/queued-placement lifecycle.
var _armed_order := ""
## Producer building kinds come from the Rust catalogue via
## catalogue_snapshot — GDScript hardcodes no gameplay data.
var _producer_kinds: Array[String] = []
## The selected building's snapshot, fetched once per frame by _refresh_hud
## and shared with _selected_producer_kind; a mid-frame selection change
## refetches once so clicks never read the previous building.
var _selected_building_snap: Dictionary = {}
var _selected_building_snap_id := -1
## Tick/phase markers behind the per-tick refresh gate: the snapshot can
## only change inside a fixed tick, and the lock line only on a phase
## change, so anything fresher is a wasted rebuild.
var _ug_last_tick := -1
var _ug_last_phase := ""
## One-way flag for the single quit path; it also freezes session refreshes so
## no cue or the ambience restarts during the audio drain.
var _shutting_down := false
var _quit_finished := false
var _previous_auto_accept_quit := true

func _ready() -> void:
	# Quit must go through request_quit so the audio drain runs; the OS close
	# request is handled below instead of SceneTree's implicit quit.
	var tree := get_tree()
	_previous_auto_accept_quit = tree.auto_accept_quit
	tree.auto_accept_quit = false
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var units: Dictionary = catalogue.get("units", {})
	_producer_kinds = HudHelpers.producer_kinds(units)
	_feedback_revision = int(GrusBridge.command_feedback_revision())
	command_status.text = str(GrusBridge.command_feedback())
	_ug_drawer.room_requested.connect(_on_room_requested)
	_underground_excavate.pressed.connect(_on_excavate_requested)
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

func _exit_tree() -> void:
	var tree := get_tree()
	if tree != null:
		tree.auto_accept_quit = _previous_auto_accept_quit

func _process(delta: float) -> void:
	_scroll_camera(delta)
	_index_views()
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

func _index_views() -> void:
	var unit_kinds := {}
	var unit_views := {}
	var friendly_units: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group("unit_views"):
		var id := int(node.get_meta("unit_id", -1))
		unit_kinds[id] = str(node.get_meta("unit_kind", ""))
		var view := node as Node3D
		if view != null:
			unit_views[id] = view
			if int(view.get_meta("team_id", -1)) == 1:
				friendly_units.append(view)
	var building_kinds := {}
	var building_views := {}
	for node in get_tree().get_nodes_in_group("building_views"):
		var id := int(node.get_meta("building_id", -1))
		building_kinds[id] = str(node.get_meta("building_kind", ""))
		var view := node as Node3D
		if view != null:
			building_views[id] = view
	_unit_kinds = unit_kinds
	_unit_views = unit_views
	_friendly_unit_views = friendly_units
	_building_kinds = building_kinds
	_building_views = building_views
	_views_indexed = true

func _consume_placement_results() -> bool:
	var unresolved: Array[Dictionary] = []
	var handled_feedback := false
	for request in _pending_placement_requests:
		var result: Dictionary = GrusBridge.take_placement_command_result(int(request.get("id", -1)))
		if not bool(result.get("complete", false)):
			unresolved.append(request)
			continue
		var request_id := int(request.get("id", -1))
		var request_generation := int(request.get("generation", -1))
		var request_kind := str(request.get("kind", ""))
		var accepted := bool(result.get("accepted", false))
		var feedback := str(result.get("feedback", "Placement rejected"))
		_last_placement_verdict = {
			"id": request_id,
			"generation": request_generation,
			"kind": request_kind,
			"accepted": accepted,
			"feedback": feedback,
		}
		if request_generation != _placement_generation \
				or request_kind != _placement_kind:
			# A dropped result must mask only its own echo: the shared channel
			# keeps the last command's text per tick, so identical text is this
			# stale acceptance while a different text is unrelated feedback
			# (a move/route rejection) that still applies.
			if feedback == str(GrusBridge.command_feedback()):
				handled_feedback = true
			continue
		handled_feedback = true
		command_status.text = feedback
		if accepted and _placement_kind != "Wall" and _placement_kind != "Gate":
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
	if _shutting_down:
		return
	var snap: Dictionary = GrusBridge.session_snapshot()
	var previous_phase := _session_phase
	_session_phase = str(snap.get("phase", ""))
	if _session_phase == "Result":
		# Result owns the screen; the drawer is inspection-only and must not
		# sit behind the mouse-ignoring outcome composition.
		_close_underground_panel()
	if _session_phase != "Playing":
		_armed_order = ""
	var winner_team := int(snap.get("winner_team", -1))
	if _session_phase != previous_phase:
		_fx.set_wind_phase(_session_phase)
	if _session_phase != previous_phase or winner_team != _session_winner:
		_session_winner = winner_team
		_hud.refresh_session(_session_phase, winner_team)
	var result_frame := _session_phase == "Result" and previous_phase != "Result"
	if _session_phase != "Playing" and not result_frame:
		return
	var events: Array = GrusBridge.drain_combat_events()
	for event in events:
		_dispatch_combat_event(event)

## Gameplay orders only ever leave Godot while the session is Playing and no
## quit is pending.
func _playing() -> bool:
	return not _shutting_down and _session_phase == "Playing"

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
		var building_kind := str(attacker.get_meta("building_kind", ""))
		# Painted combat towers anchor their muzzle on the sprite plane via
		# sprite_local_from_pixel so tracers never originate behind the quad;
		# unstyled views fall back to world-Y offsets.
		if PAINTED_MUZZLE_UV.has(building_kind):
			var model := attacker.get_node_or_null("Model") as Node3D
			if model != null:
				var sprites := model.find_children("*", "Sprite3D", true, false)
				if not sprites.is_empty():
					var sprite := sprites[0] as Sprite3D
					if sprite != null and sprite.texture != null:
						var uv: Vector2 = PAINTED_MUZZLE_UV[building_kind]
						var pixel: Vector2 = uv * sprite.texture.get_size()
						return sprite.global_transform * BuildingView.sprite_local_from_pixel(sprite, pixel)
		match building_kind:
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
	_selected_building_snap = building
	_selected_building_snap_id = selected_building_id
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
	_hud.refresh_build_placement(_placement_kind)
	_hud.refresh_research_controls(research, _playing(), _research_center_id() > 0)

## Tab and Esc-while-open route through `_input` (not `_unhandled_input`)
## and mark the event handled: `ui_focus_next` would otherwise consume Tab
## once a panel button holds keyboard focus, and the open panel owns Esc
## ahead of the pause shortcut. Every Tab event — press, echo and release
## — is handled, since holding Tab repeats echo events that would
## otherwise walk GUI focus onto the panel's buttons. Every other key
## keeps its existing `_unhandled_input` route.
func _input(event: InputEvent) -> void:
	if _shutting_down:
		get_viewport().set_input_as_handled()
		return
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
	if _shutting_down:
		get_viewport().set_input_as_handled()
		return
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

## Arrow keys and a mouse resting on the window edge pan the camera. The edge
## only counts while the window is focused, the pointer is inside the viewport
## and no drag is in progress. A headless display server reports the pointer
## parked at (0, 0), so it never edge-scrolls and smokes keep a still camera.
func _scroll_camera(delta: float) -> void:
	if _shutting_down:
		return
	var direction := Vector2.ZERO
	direction.x = float(Input.is_physical_key_pressed(KEY_RIGHT)) \
			- float(Input.is_physical_key_pressed(KEY_LEFT))
	direction.y = float(Input.is_physical_key_pressed(KEY_DOWN)) \
			- float(Input.is_physical_key_pressed(KEY_UP))
	if direction == Vector2.ZERO and not _left_pressed \
			and DisplayServer.get_name() != "headless" and DisplayServer.window_is_focused():
		direction = _edge_direction(get_viewport().get_mouse_position())
	if direction == Vector2.ZERO:
		return
	var step := camera.size * PAN_SPEED * delta
	camera.position.x += direction.x * step
	camera.position.z += direction.y * step
	_clamp_camera_to_map()
	camera.reset_physics_interpolation()

## Keep the ground point under the viewport center on the map, so a resting
## edge or a held key cannot scroll the battlefield out of view.
func _clamp_camera_to_map() -> void:
	var forward := -camera.global_transform.basis.z.normalized()
	if absf(forward.y) < 0.0001:
		return
	var hit := camera.global_position + forward * (-camera.global_position.y / forward.y)
	camera.position.x += clampf(hit.x, 0.0, float(MinimapView.MAP_WIDTH)) - hit.x
	camera.position.z += clampf(hit.z, 0.0, float(MinimapView.MAP_HEIGHT)) - hit.z

func _edge_direction(mouse: Vector2) -> Vector2:
	var rect := get_viewport().get_visible_rect()
	if not rect.has_point(mouse):
		return Vector2.ZERO
	var direction := Vector2.ZERO
	if mouse.x <= rect.position.x + EDGE_PAN_MARGIN:
		direction.x = -1.0
	elif mouse.x >= rect.end.x - EDGE_PAN_MARGIN:
		direction.x = 1.0
	if mouse.y <= rect.position.y + EDGE_PAN_MARGIN:
		direction.y = -1.0
	elif mouse.y >= rect.end.y - EDGE_PAN_MARGIN:
		direction.y = 1.0
	return direction

func _pan_camera(relative: Vector2) -> void:
	var viewport_height := maxf(get_viewport().get_visible_rect().size.y, 1.0)
	var world_per_pixel := camera.size / viewport_height
	camera.position.x -= relative.x * world_per_pixel
	camera.position.z -= relative.y * world_per_pixel

func _select_at(screen_position: Vector2, additive: bool) -> void:
	var all_building_hit := _building_hit_at(screen_position, 0)
	var unit_pick := _unit_pick_view(screen_position, 1, all_building_hit)
	var nearest := unit_pick.get("view") as Node3D
	if nearest != null:
		if not additive:
			selected_ids.clear()
		selected_building_id = -1
		var id := int(nearest.get_meta("unit_id", -1))
		if id > 0 and not selected_ids.has(id):
			selected_ids.append(id)
		_apply_selection()
		return
	var building := _select_building_from_hit(screen_position, all_building_hit, 1)
	# An enemy body covering the click hides a friendly building behind it —
	# left-clicking through that cover selects nothing, not the building.
	if building != null and not bool(unit_pick.get("covered", false)):
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
		if unit.has_method("set_selected"):
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
	# A right-click on an enemy unit/building attacks it with the whole
	# selection; the sim rejects noncombatants per unit. The pick consumes
	# the click only when the selection holds a combatant — any selected
	# unit that is not a Worker, the kind split _selected_worker_ids already
	# single-sources — and, when the click names an actionable worker
	# target, only when the enemy pick is a direct silhouette hit: an alien
	# in front of a site is still attacked over resuming it, but a 20 px
	# fallback pick can no longer swallow a gather/resume click it merely
	# neighbours. The fallback still fires when no worker target exists,
	# and an enemy hidden behind the site's opaque sprite still falls
	# through.
	var workers := _selected_worker_ids()
	var resource: Node3D = null
	var resource_id := -1
	var resumable: Node3D = null
	var all_building_hit: Dictionary = {}
	if not workers.is_empty():
		resource = _resource_at(screen_position)
		if resource != null:
			resource_id = int(resource.get_meta("resource_id", -1))
		if resource_id <= 0:
			all_building_hit = _building_hit_at(screen_position, 0)
			var site := all_building_hit.get("view") as Node3D
			if site != null and int(site.get_meta("team_id", -1)) == 1 \
					and not _building_complete(int(site.get_meta("building_id", -1))):
				resumable = site
	if workers.size() != selected_ids.size():
		var enemy_pick := _enemy_pick_at(screen_position, all_building_hit)
		var enemy := enemy_pick.get("view") as Node3D
		var enemy_wins := false
		if enemy != null:
			if bool(enemy_pick.get("direct", false)):
				enemy_wins = true
				# A visible resource nearer on the click ray than the enemy
				# behind it keeps Gather priority for mixed selections; an
				# enemy in front of the resource still takes Attack.
				if resource_id > 0 and resource != null:
					var ray_origin := camera.project_ray_origin(screen_position)
					var ray_direction := camera.project_ray_normal(screen_position)
					var resource_depth := _model_ray_hit_depth(resource, ray_origin, ray_direction)
					var enemy_depth := float(enemy_pick.get("depth", INF))
					if resource_depth >= 0.0 and resource_depth < enemy_depth:
						enemy_wins = false
			elif resource_id <= 0 and resumable == null:
				enemy_wins = true
		if enemy_wins:
			var target_kind := "unit" if enemy.is_in_group("unit_views") else "building"
			var target_id := int(enemy.get_meta("unit_id", enemy.get_meta("building_id", -1)))
			if GrusBridge.attack_units(PackedInt32Array(selected_ids), target_kind, target_id):
				command_status.text = "Attack command queued"
			else:
				command_status.text = "Attack command rejected"
			return
	if resource_id > 0:
		_issue_gather(workers, resource_id)
		return
	if resumable != null:
		var resumable_id := int(resumable.get_meta("building_id", -1))
		if GrusBridge.resume_construction(workers[0], resumable_id):
			command_status.text = "Construction resume queued"
		else:
			command_status.text = "Construction resume rejected"
		return
	if _producer_kinds.has(_selected_producer_kind()):
		var target = _ground_target(screen_position)
		if target != null:
			_issue_rally(target)
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
			_issue_gather(workers, resource_id)
		"rally":
			var target = _ground_target(screen_position)
			if target == null or not _producer_kinds.has(_selected_producer_kind()):
				command_status.text = "Rally point rejected"
				return
			_issue_rally(target)
		_:
			command_status.text = "Order canceled"

func _issue_gather(workers: Array[int], resource_id: int) -> void:
	if GrusBridge.gather(PackedInt32Array(workers), resource_id):
		command_status.text = "Gather command queued"
	else:
		command_status.text = "Gather command rejected"

func _issue_rally(target: Vector2) -> void:
	if GrusBridge.set_rally(selected_building_id, floori(target.x), floori(target.y)):
		command_status.text = "Rally point queued"
	else:
		command_status.text = "Rally point rejected"

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

## Unit pick eligibility is the unit's own silhouette first: a click on the
## visible body (torso, head) selects the unit even beyond the feet-projection
## radius, which stays only as the affordance for near-miss clicks on empty
## ground. The silhouette search runs across teams before the caller's team
## rule — a visible body of the other team covers the click, so neither the
## occluded unit's mesh nor the 20 px fallback may pick through it. When a
## foreground building occludes the mesh-hit unit the team rule is moot —
## the body never covered the click — so the pick falls back to
## `_nearest_view` in either team order and a visible unit within 20 px
## beside the building is still considered before giving the click to the
## building.
func _unit_pick_at(screen_position: Vector2, team_filter: int, building_hit: Dictionary = {}) -> Node3D:
	return _unit_pick_view(screen_position, team_filter, building_hit).get("view") as Node3D

## `_unit_pick_at` plus pick provenance: `direct` is true only for a real
## Model mesh hit that survived the foreground-building check, false for the
## 20 px feet-radius fallback — so a caller weighing the pick against an
## actionable worker order can tell a body click from a near miss. A click
## covered by a foreground unit of another team yields neither, while a
## building-hidden body of another team blocks neither — the occlusion
## check runs before the team rule.
func _unit_pick_view(screen_position: Vector2, team_filter: int, building_hit: Dictionary = {}) -> Dictionary:
	var mesh_hit := _unit_mesh_hit_at(screen_position)
	var mesh_view := mesh_hit.get("view") as Node3D
	if mesh_view != null:
		var building_depth := float(building_hit.get("depth", INF))
		var mesh_depth := float(mesh_hit.get("depth", INF))
		var occluded := building_depth < INF and building_depth < mesh_depth
		if not occluded:
			if team_filter != 0 and int(mesh_view.get_meta("team_id", -1)) != team_filter:
				# A wrong-team body still covers the click: callers must
				# suppress whatever it hides (a nearer opaque building hit
				# would already have taken the occluded branch above).
				return {"view": null, "direct": false, "covered": true, "depth": INF}
			return {"view": mesh_view, "direct": true, "depth": mesh_depth}
	return {"view": _nearest_view("unit_views", screen_position, team_filter, CLICK_RADIUS, building_hit), "direct": false, "depth": INF}

## The frontmost visible unit body on the pick ray across every team — the
## team rule is the caller's, because a click covered by another team's mesh
## is covered ground, not a pick of whatever sits behind it.
func _unit_mesh_hit_at(screen_position: Vector2) -> Dictionary:
	var ray_origin := camera.project_ray_origin(screen_position)
	var ray_direction := camera.project_ray_normal(screen_position)
	var nearest_hit_view: Node3D = null
	var nearest_hit_depth := INF
	for node in get_tree().get_nodes_in_group("unit_views"):
		var view := node as Node3D
		if view == null or not view.is_visible_in_tree() \
				or camera.is_position_behind(view.global_position):
			continue
		var model := view.get_node_or_null("Model") as Node3D
		if model == null:
			continue
		if not _projected_art_rect(view, model).has_point(screen_position):
			continue
		var depth := _model_ray_hit_depth(model, ray_origin, ray_direction)
		if depth >= 0.0 and depth < nearest_hit_depth:
			nearest_hit_view = view
			nearest_hit_depth = depth
	return {"view": nearest_hit_view, "depth": nearest_hit_depth}

func _nearest_view(group: String, screen_position: Vector2, team_filter := 0, radius := CLICK_RADIUS, occluding_hit: Dictionary = {}) -> Node3D:
	var nearest: Node3D = null
	var nearest_distance := radius
	var check_occlusion := not occluding_hit.is_empty()
	var ray_origin := camera.project_ray_origin(screen_position) if check_occlusion else Vector3.ZERO
	var ray_direction := camera.project_ray_normal(screen_position) if check_occlusion else Vector3.ZERO
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
			if check_occlusion and _building_beats_unit(occluding_hit, view, ray_origin, ray_direction):
				continue
			nearest = view
			nearest_distance = distance
	return nearest

## Resolve the building for `team_filter` after occlusion by any team's
## opaque foreground silhouette: a real foreground hit blocks buildings
## behind it (even of `team_filter`) and suppresses fallback-only hits,
## while a fallback-only hit leaves the team-filtered 20 px fallback eligible.
func _select_building_from_hit(screen_position: Vector2, all_building_hit: Dictionary, team_filter: int) -> Node3D:
	if float(all_building_hit.get("depth", INF)) < INF:
		var hit_view := all_building_hit.get("view") as Node3D
		if hit_view != null and int(hit_view.get_meta("team_id", -1)) == team_filter:
			return hit_view
		return null
	if all_building_hit.has("fallback_by_team"):
		var fallbacks: Dictionary = all_building_hit.get("fallback_by_team", {})
		return fallbacks.get(team_filter, null) as Node3D
	return _building_at(screen_position, team_filter)

## Compare the nearest eligible enemy with the silhouette-picked building:
## an overlapping enemy loses to a real opaque building hit in front of its
## body reference — of either team, because a friendly roof hides the body
## pixels the mesh picker accepts — while only an unoccluded enemy building
## can be the returned attack target. Fallback-only building hits never
## outrank a unit and never pierce an opaque foreground roof.
func _nearest_enemy_view(screen_position: Vector2) -> Node3D:
	return _enemy_pick_at(screen_position).get("view") as Node3D

## `_nearest_enemy_view` plus pick provenance under the same arbitration
## rules: `direct` is true only when the returned enemy carried a real
## silhouette hit — an unoccluded unit mesh or a real enemy-building face —
## while the 20 px fallback reports false so a gather/resume click can
## refuse to yield to a near miss.
func _enemy_pick_at(screen_position: Vector2, precomputed_building_hit: Dictionary = {}) -> Dictionary:
	var all_building_hit := precomputed_building_hit if not precomputed_building_hit.is_empty() else _building_hit_at(screen_position, 0)
	var unit_pick := _unit_pick_view(screen_position, 2, all_building_hit)
	var unit := unit_pick.get("view") as Node3D
	var enemy_building := _select_building_from_hit(screen_position, all_building_hit, 2)
	var building_depth := float(all_building_hit.get("depth", INF))
	var building_direct := enemy_building != null and building_depth < INF
	if unit == null:
		# A friendly body covering the click hides every enemy building
		# behind it — no attack through the cover. A nearer opaque hit
		# would have occluded the unit inside _unit_pick_view already.
		if bool(unit_pick.get("covered", false)):
			return {"view": null, "direct": false, "depth": INF}
		return {"view": enemy_building, "direct": building_direct, "depth": building_depth if building_direct else INF}
	return {"view": unit, "direct": bool(unit_pick.get("direct", false)), "depth": float(unit_pick.get("depth", INF))}

## A building only steals an overlapping unit's pick on a real opaque
## silhouette hit strictly in front of the unit's visible geometry: when the
## pick ray meets the unit's own Model (head, torso) that mesh depth decides,
## so a finished Gate's overhead beam cannot take a click the Worker's body
## occludes; a ray that misses the unit's mesh falls back to the small
## ground-level body reference (+0.15 y), so the shallow plinth still never
## takes a Worker's click and a tall unit behind a painted roof never steals
## the roof click. A fallback-only hit (depth INF) never outranks the unit.
func _building_beats_unit(building_hit: Dictionary, unit: Node3D, ray_origin: Vector3, ray_direction: Vector3) -> bool:
	var building_depth: float = building_hit.get("depth", INF)
	if building_depth >= INF:
		return false
	var unit_model := unit.get_node_or_null("Model") as Node3D
	if unit_model != null:
		var unit_hit := _model_ray_hit_depth(unit_model, ray_origin, ray_direction)
		if unit_hit >= 0.0:
			return building_depth < unit_hit
	var unit_depth := (unit.global_position + Vector3(0.0, 0.15, 0.0) - ray_origin).dot(ray_direction)
	return building_depth < unit_depth

func _view_distance(view: Node3D, screen_position: Vector2) -> float:
	return camera.unproject_position(view.global_position).distance_to(screen_position)

## Building picking tests the projected silhouette of the view's active
## Model (health bar excluded) via ray-triangle intersection, with closest-hit
## depth sorting along the camera ray so foreground geometry wins over background
## structures. The center-radius floor stays as the fallback when no silhouette
## is hit (e.g. for small buildings at wide zoom or unstyled views). Hidden
## (fogged) views are never picked. Returns {view, depth}; depth is INF for a
## 20px fallback-only hit so unit overlap comparisons can never let a
## fallback outrank an eligible unit.
func _building_hit_at(screen_position: Vector2, team_filter := 0) -> Dictionary:
	var ray_origin := camera.project_ray_origin(screen_position)
	var ray_direction := camera.project_ray_normal(screen_position)
	var nearest_hit_view: Node3D = null
	var nearest_hit_depth := INF
	var fallback: Node3D = null
	var fallback_distance := CLICK_RADIUS
	var fallback_by_team := {}
	var fallback_dist_by_team := {}
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		if view == null or not view.is_visible_in_tree() \
				or camera.is_position_behind(view.global_position):
			continue
		var view_team := int(view.get_meta("team_id", -1))
		if team_filter != 0 and view_team != team_filter:
			continue
		var center_px := camera.unproject_position(view.global_position)
		var distance := center_px.distance_to(screen_position)
		if distance <= fallback_distance:
			fallback = view
			fallback_distance = distance
		if team_filter == 0 and distance <= float(fallback_dist_by_team.get(view_team, CLICK_RADIUS)):
			fallback_by_team[view_team] = view
			fallback_dist_by_team[view_team] = distance
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
	if nearest_hit_view != null:
		return {"view": nearest_hit_view, "depth": nearest_hit_depth}
	var result := {"view": fallback, "depth": INF}
	if team_filter == 0:
		result["fallback_by_team"] = fallback_by_team
	return result

func _building_at(screen_position: Vector2, team_filter := 0) -> Node3D:
	return _building_hit_at(screen_position, team_filter).get("view") as Node3D

func _model_ray_hit_depth(model: Node3D, ray_origin: Vector3, ray_dir: Vector3) -> float:
	var min_depth := INF
	for node in model.find_children("*", "GeometryInstance3D", true, false):
		var art: GeometryInstance3D = BuildingView.art_geometry(node)
		# Silhouette picking tests rendered geometry only: a hidden mesh
		# still carries triangles and must not select or cover a click. The
		# view root's own visibility stays the caller's contract — every
		# caller already checked it before scanning the model.
		if art == null or not BuildingView.art_rendered(art, model):
			continue
		# Both paths yield a TriangleMesh with intersect_ray: painted sprites
		# generate their quad directly, meshes go through Mesh.generate_triangle_mesh.
		var triangles: Variant = null
		if art is SpriteBase3D:
			triangles = (art as SpriteBase3D).generate_triangle_mesh()
		else:
			var mesh_instance := art as MeshInstance3D
			if mesh_instance.mesh != null:
				triangles = mesh_instance.mesh.generate_triangle_mesh()
		if triangles == null:
			continue
		var inverse := art.global_transform.affine_inverse()
		var hit: Variant = triangles.intersect_ray(inverse * ray_origin, (inverse.basis * ray_dir).normalized())
		if hit.is_empty():
			continue
		if art is SpriteBase3D:
			# Painted silhouette: the quad triangle only counts when the
			# texel under the hit is opaque — transparent corners must not
			# pick the building. Hit-local point → image row (top-down
			# flip) → the same shared BitMap the health bar used.
			var sprite := art as SpriteBase3D
			var pixel: Vector2 = BuildingView.sprite_pixel_from_local(sprite, hit["position"])
			if not BuildingView.pixel_opaque(sprite.texture, pixel):
				continue
		var world_hit: Vector3 = art.global_transform * hit["position"]
		min_depth = minf(min_depth, (world_hit - ray_origin).dot(ray_dir))
	return min_depth if min_depth < INF else -1.0

func _friendly_units() -> Array[Node3D]:
	if _views_indexed:
		var live: Array[Node3D] = []
		for view in _friendly_unit_views:
			if is_instance_valid(view):
				live.append(view)
		return live
	var result: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) == 1:
			result.append(node as Node3D)
	return result

func _unit_kind(unit_id: int) -> String:
	if _views_indexed:
		return str(_unit_kinds.get(unit_id, ""))
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("unit_id", -1)) == unit_id:
			return str(node.get_meta("unit_kind", ""))
	return ""

func _unit_view(unit_id: int) -> Node3D:
	if _views_indexed:
		var cached := _unit_views.get(unit_id, null) as Node3D
		return cached if is_instance_valid(cached) else null
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("unit_id", -1)) == unit_id:
			return node as Node3D
	return null

func _building_view(building_id: int) -> Node3D:
	if _views_indexed:
		var cached := _building_views.get(building_id, null) as Node3D
		return cached if is_instance_valid(cached) else null
	for node in get_tree().get_nodes_in_group("building_views"):
		if int(node.get_meta("building_id", -1)) == building_id:
			return node as Node3D
	return null

func _on_start_pressed() -> void:
	if _shutting_down:
		return
	GrusBridge.start_match()

func _on_pause_pressed() -> void:
	if _shutting_down:
		return
	GrusBridge.set_paused(true)

func _on_resume_pressed() -> void:
	if _shutting_down:
		return
	GrusBridge.set_paused(false)

func _on_restart_pressed() -> void:
	if _shutting_down:
		return
	_restart_match()

func _on_quit_pressed() -> void:
	request_quit()

## The single quit path (OS window close and the Result button): stop the
## ambience/cues and give the audio mixer a bounded best-effort window before
## the engine tears down (see CombatFx.stop_and_drain). A Playing match is
## frozen through the existing Rust pause seam so the final frames cannot
## mutate the authoritative session; engine and audio processing keep running.
func request_quit() -> void:
	if _shutting_down:
		return
	_shutting_down = true
	var tree := get_tree()
	if tree == null:
		return
	# Unconditional: Rust leaves Start/Paused/Result and a session-free
	# benchmark unchanged, and this also covers a Start/Resume whose phase the
	# frame cache has not seen yet.
	GrusBridge.set_paused(true)
	# Guarantee quit and auto_accept_quit restoration even if the drain coroutine fails.
	var fallback := tree.create_timer(QUIT_FALLBACK_SECONDS, true, false, true)
	fallback.timeout.connect(_finish_quit.bind(tree), CONNECT_ONE_SHOT)
	if is_instance_valid(_fx):
		var drained = await _fx.stop_and_drain()
		if not drained:
			push_warning("audio shutdown drain hit its deadline; playbacks may still be live")
	_finish_quit(tree)

func _finish_quit(tree: SceneTree) -> void:
	if _quit_finished:
		return
	_quit_finished = true
	if tree != null:
		tree.auto_accept_quit = _previous_auto_accept_quit
		tree.quit(0)

func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:
		request_quit()

## Restart clears every transient Godot-side fact while the bridge reseeds:
## selection, control groups, placement, attack-move arming, the armed
## underground room kind, and live combat effects. No dead stable ids
## survive the reset.
func _restart_match() -> void:
	if _shutting_down:
		return
	GrusBridge.restart_match()
	selected_ids.clear()
	selected_building_id = -1
	_selected_building_snap = {}
	_selected_building_snap_id = -1
	_research_center_view = null
	_unit_views = {}
	_building_views = {}
	_friendly_unit_views = []
	_views_indexed = false
	_control_groups.clear()
	_idle_cursor = 0
	_armed_order = ""
	_ug_drawer.reset()
	_pending_placement_requests.clear()
	_last_placement_refusal = {}
	_last_placement_verdict = {}
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

## art_root scopes the art scan — passing a building view's active Model
## keeps its sibling HealthBar out of the pick silhouette, while a missing
## root still walks the whole view (resource models are the view). Meshes
## and painted sprites both count; both expose get_aabb().
func _view_art_bounds(view: Node3D, art_root: Node3D = null) -> AABB:
	if art_root != null and art_root.has_meta("_cached_art_bounds"):
		return art_root.get_meta("_cached_art_bounds") as AABB
	var bounds := AABB()
	var first := true
	var deferred_art := false
	for node in (art_root if art_root != null else view).find_children("*", "GeometryInstance3D", true, false):
		var art: GeometryInstance3D = BuildingView.art_geometry(node)
		if art == null or not BuildingView.art_rendered(art, art_root if art_root != null else view):
			continue
		# Sprite layout and AABB both update inside deferred flushes: a
		# query between a footprint change and the next redraw returns the
		# pre-resize quad, so sprite bounds are recomputed every call —
		# caching would freeze the stale silhouette until the next resize.
		if art is SpriteBase3D:
			deferred_art = true
		var local: AABB = view.global_transform.affine_inverse() * art.global_transform * art.get_aabb()
		bounds = local if first else bounds.merge(local)
		first = false
	if bounds.size.length() < 0.001:
		# No art yet (kind meta arrives a frame late, or Sprite3D's deferred
		# _im_update has not computed its AABB yet): fall back to the cell
		# without caching so real art bounds replace it on the next query.
		return AABB(Vector3(-0.5, 0.0, -0.5), Vector3(1.0, 1.0, 1.0))
	if art_root != null and not deferred_art:
		art_root.set_meta("_cached_art_bounds", bounds)
	return bounds

func _building_complete(building_id: int) -> bool:
	if building_id <= 0:
		return true
	var snapshot: Dictionary = GrusBridge.building_snapshot(building_id)
	if snapshot.is_empty():
		return true
	return bool(snapshot.get("complete", true))

## The selected building's kind only while it can take rally/train context:
## friendly (team 1), construction complete, and alive in the shared
## per-frame snapshot. An enemy, hidden or unfinished producer reports "".
func _selected_producer_kind() -> String:
	if selected_building_id <= 0:
		return ""
	if _selected_building_snap_id != selected_building_id:
		_selected_building_snap = GrusBridge.building_snapshot(selected_building_id)
		_selected_building_snap_id = selected_building_id
	if int(_selected_building_snap.get("team_id", -1)) != 1 \
			or not bool(_selected_building_snap.get("complete", false)):
		return ""
	return str(_selected_building_snap.get("kind", ""))

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
## context — clicking the armed tile again disarms it; Stop runs
## immediately and Cancel clears placement/order without enqueueing a
## gameplay command.
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
	if _armed_order == order:
		_armed_order = ""
		command_status.text = "Order canceled"
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
		var refusal := "Placement blocked (code %d)" % int(preview.get("reject_code", -1))
		_last_placement_refusal = {"generation": _placement_generation, "text": refusal}
		command_status.text = refusal
		return
	var request_id := int(GrusBridge.next_placement_request_id())
	var kind := _placement_kind
	if GrusBridge.place_building_request(builder, kind, anchor_x, anchor_y, request_id):
		_last_placement_refusal = {}
		_last_placement_verdict = {}
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
## building_views metadata (kind/team/id) and cached until the view becomes
## invalid or hidden — never a hardcoded building id and never a per-frame scan.
func _research_center_id() -> int:
	if is_instance_valid(_research_center_view) \
			and _research_center_view.is_inside_tree() \
			and _research_center_view.is_visible_in_tree() \
			and int(_research_center_view.get_meta("team_id", -1)) == 1 \
			and str(_research_center_view.get_meta("building_kind", "")) == "ResearchCenter":
		var cached_id := int(_research_center_view.get_meta("building_id", -1))
		if cached_id > 0:
			return cached_id
	_research_center_view = null
	# Skip the group scan entirely when this frame's kind map holds no
	# Research Center (destroyed, or a fixture without one): otherwise the
	# rescan would repeat every frame until one appears.
	var any_center := false
	for id in _building_kinds:
		if _building_kinds[id] == "ResearchCenter":
			any_center = true
			break
	if not any_center:
		return -1
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		if view == null or not view.is_visible_in_tree():
			continue
		if int(view.get_meta("team_id", -1)) != 1 \
				or str(view.get_meta("building_kind", "")) != "ResearchCenter":
			continue
		var id := int(view.get_meta("building_id", -1))
		if id > 0:
			_research_center_view = view
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
	if _shutting_down:
		return
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
	var view := _unit_view(worker_id)
	if view != null:
		camera.position.x = view.global_position.x
		camera.position.z = view.global_position.z
	command_status.text = "Selected idle worker #%d" % worker_id

## Tab toggles the underground panel; the panel itself consumes mouse input
## (Control STOP filter) so clicks never leak into ground selection,
## placement or unit commands. Result never reopens it: the outcome screen
## owns the frame.
func _toggle_underground_panel() -> void:
	if _session_phase == "Result":
		return
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
