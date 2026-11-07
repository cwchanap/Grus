extends Node3D

const CLICK_RADIUS := 20.0
const DRAG_THRESHOLD := 6.0
const MIN_CAMERA_SIZE := 24.0
const MAX_CAMERA_SIZE := 120.0
const ZOOM_STEP := 6.0

@onready var camera: Camera3D = $Camera3D
@onready var command_status: Label = $HUD/CommandStatus
@onready var wave_label: RichTextLabel = $HUD/WaveLabel
@onready var research_bar: ProgressBar = $HUD/ResearchPanel/ResearchBar
@onready var research_label: Label = $HUD/ResearchPanel/ResearchLabel
@onready var research_icon: TextureRect = $HUD/ResearchPanel/ResearchIcon
@onready var analyze_button: Button = $HUD/ResearchPanel/AnalyzeButton
@onready var economy_label: RichTextLabel = $HUD/EconomyPanel/EconomyLabel
@onready var idle_button: Button = $HUD/EconomyPanel/IdleButton
@onready var selection_label: Label = $HUD/SelectionPanel/SelectionLabel
@onready var _build_grid: Container = $HUD/CommandPanel/BuildGrid
@onready var _train_grid: Container = $HUD/CommandPanel/TrainGrid
@onready var _preview_box: MeshInstance3D = $PlacementPreview
@onready var _fx: Node3D = $CombatFx
@onready var session_panel: Panel = $HUD/SessionPanel
@onready var session_label: Label = $HUD/SessionPanel/SessionLabel
@onready var _minimap: Control = $HUD/Minimap
@onready var start_button: Button = $HUD/SessionPanel/StartButton
@onready var resume_button: Button = $HUD/SessionPanel/ResumeButton
@onready var restart_button: Button = $HUD/SessionPanel/RestartButton
@onready var quit_button: Button = $HUD/SessionPanel/QuitButton
@onready var pause_button: Button = $HUD/PauseButton
@onready var _underground_panel: Panel = $HUD/UndergroundPanel
@onready var _underground_header: Label = $HUD/UndergroundPanel/HeaderLabel
@onready var _underground_excavate: Button = $HUD/UndergroundPanel/ExcavateButton
@onready var _underground_kinds: Container = $HUD/UndergroundPanel/KindsBox
@onready var _underground_levels: Container = $HUD/UndergroundPanel/ScrollContainer/LevelsBox

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
var _attack_move_armed := false
## Producer building kinds come from the Rust catalogue via
## catalogue_snapshot — GDScript hardcodes no gameplay data.
var _producer_kinds: Array[String] = []
## Static research tuning, cached once from the catalogue's "research"
## block; the live points/rate/ETA arrive per frame through
## research_snapshot — never a mirrored mutable copy.
var _research_target_points := 0
## Static underground catalogue rows (name/min_level/cost/seconds),
## cached once from the catalogue's "underground" block; every dynamic
## value (depth, dig, slots) arrives per frame through underground_snapshot
## while the panel is visible — never a mirrored mutable copy.
var _ug_rooms: Array[Dictionary] = []
var _ug_slots_per_level := 4
## Tick/phase markers behind the per-tick refresh gate: the snapshot can
## only change inside a fixed tick, and the lock line only on a phase
## change, so anything fresher is a wasted rebuild.
var _ug_last_tick := -1
var _ug_last_phase := ""
## Index into `_ug_rooms` of the armed room kind (-1 = none armed); the
## only selection state — slots derive eligibility from it every refresh.
var _ug_armed_index := -1
var _ug_kind_buttons: Array[Button] = []
var _icon_cache: Dictionary = {}
## Cosmetic button labels only — the authoritative kind wire names ride the
## catalogue entries and bridge commands.
const UNDERGROUND_DISPLAY_NAMES := {
	"Shelter": "Shelter",
	"Greenhouse": "Green",
	"CoalVein": "Coal",
	"GeothermalTap": "Tap",
	"Lab": "Lab",
	"DeepMine": "Mine",
}

func _ready() -> void:
	var research_progress_icon := _icon_for_kind("status", "ResearchProgress")
	research_icon.texture = research_progress_icon
	research_icon.visible = research_progress_icon != null
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var buildings: Dictionary = catalogue.get("buildings", {})
	var producers := {}
	var units: Dictionary = catalogue.get("units", {})
	for kind in units:
		producers[str(units[kind].get("producer", ""))] = true
	_producer_kinds.assign(producers.keys())
	_producer_kinds.sort()
	_feedback_revision = int(GrusBridge.command_feedback_revision())
	command_status.text = str(GrusBridge.command_feedback())
	# The one static research read at init: point target, Analyze price and
	# reward. Every dynamic value (points/rate/ETA, lock states) refreshes
	# per frame from research_snapshot in _refresh_hud.
	var research_catalogue: Dictionary = catalogue.get("research", {})
	_research_target_points = int(research_catalogue.get("target_points", 0))
	var analyze_cost: Dictionary = research_catalogue.get("analyze_cost", {})
	var analyze_crystals := int(analyze_cost.get("crystals", 0))
	var analyze_gain := int(research_catalogue.get("analyze_gain_points", 0))
	research_bar.max_value = maxf(float(_research_target_points), 1.0)
	analyze_button.text = "Analyze %d→+%d" % [analyze_crystals, analyze_gain]
	analyze_button.tooltip_text = "Convert %d Crystals into %d research points" % [analyze_crystals, analyze_gain]
	analyze_button.pressed.connect(_on_analyze_pressed)
	# The static underground catalogue: room rows in wire-name order plus the
	# slot count — the only room-rule/price source, read once here. All
	# dynamic underground state arrives through underground_snapshot while
	# the panel is visible.
	var underground_catalogue: Dictionary = catalogue.get("underground", {})
	var ug_rooms: Dictionary = underground_catalogue.get("rooms", {})
	for kind in underground_catalogue.get("room_kinds", []):
		var ug_entry: Dictionary = ug_rooms.get(str(kind), {})
		if ug_entry.is_empty():
			continue
		ug_entry["name"] = str(kind)
		_ug_rooms.append(ug_entry)
	_ug_slots_per_level = maxi(int(underground_catalogue.get("slots", 4)), 1)
	_underground_excavate.pressed.connect(_on_excavate_pressed)
	# One ordinary toggle Button per room kind, built from the catalogue in
	# wire-name order: clicking arms the kind (the only selection state);
	# tooltips carry the full catalogue cost/time/effect.
	for index in _ug_rooms.size():
		var ug_entry: Dictionary = _ug_rooms[index]
		var kind_button := Button.new()
		kind_button.toggle_mode = true
		kind_button.add_theme_font_size_override("font_size", 14)
		kind_button.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
		kind_button.custom_minimum_size = Vector2(52, 65)
		kind_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		var kind := str(ug_entry.get("name", ""))
		var display_name := _underground_display_name(kind)
		kind_button.text = display_name
		var room_icon := _icon_for_kind("underground", kind)
		if room_icon != null:
			kind_button.icon = room_icon
			kind_button.add_theme_constant_override("icon_max_width", 22)
			kind_button.icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
			kind_button.vertical_icon_alignment = VERTICAL_ALIGNMENT_TOP
			kind_button.add_theme_color_override("icon_disabled_color", Color(1, 1, 1, 0.85))
			kind_button.text = ""
			kind_button.accessibility_name = display_name
			var name_label := Label.new()
			name_label.name = "KindLabel"
			name_label.text = display_name
			name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
			name_label.add_theme_font_size_override("font_size", 11)
			name_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
			name_label.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
			name_label.offset_top = -22
			name_label.offset_bottom = -2
			kind_button.add_child(name_label)
		kind_button.tooltip_text = _ug_kind_tooltip(ug_entry)
		kind_button.toggled.connect(_on_kind_toggled.bind(index))
		_underground_kinds.add_child(kind_button)
		_ug_kind_buttons.append(kind_button)
	var excavate_icon := _icon_for_kind("underground", "Excavate")
	if excavate_icon != null:
		_underground_excavate.icon = excavate_icon
		_underground_excavate.add_theme_constant_override("icon_max_width", 20)
	for button in _build_grid.get_children():
		var kind := str(button.get_meta("kind"))
		var entry: Dictionary = buildings.get(kind, {})
		var building_icon := _icon_for_kind("buildings", kind)
		if building_icon != null:
			button.icon = building_icon
			button.add_theme_constant_override("icon_max_width", 20)
		var costs := PackedStringArray()
		for resource in ["food", "scrap", "coal", "crystals"]:
			var amount := int(entry.get(resource, 0))
			if amount > 0:
				costs.append("%d %s" % [amount, resource.capitalize()])
		button.tooltip_text = "Cost: %s\nBuild: %ds" % [", ".join(costs), int(entry.get("seconds", 0))]
		# The catalogue-derived research threshold rides the button meta; the
		# per-frame lock refresh in _refresh_research_controls compares it
		# against the live point total. No per-kind conditions.
		var unlock_points := int(entry.get("unlock_points", 0))
		button.set_meta("unlock_points", unlock_points)
		if unlock_points > 0:
			button.tooltip_text += "\nUnlocks at %d research" % unlock_points
		button.pressed.connect(_on_build_pressed.bind(kind))
	for button in _train_grid.get_children():
		var unit_icon := _icon_for_kind("units", str(button.get_meta("kind")))
		if unit_icon != null:
			button.icon = unit_icon
			button.add_theme_constant_override("icon_max_width", 20)
		button.pressed.connect(_on_train_pressed.bind(str(button.get_meta("kind"))))
	idle_button.pressed.connect(_on_idle_pressed)
	start_button.pressed.connect(_on_start_pressed)
	resume_button.pressed.connect(_on_resume_pressed)
	pause_button.pressed.connect(_on_pause_pressed)
	restart_button.pressed.connect(_on_restart_pressed)
	quit_button.pressed.connect(_on_quit_pressed)

func _process(_delta: float) -> void:
	var placement_feedback_handled := _consume_placement_results()
	var revision := int(GrusBridge.command_feedback_revision())
	if revision != _feedback_revision:
		_feedback_revision = revision
		if not placement_feedback_handled:
			command_status.text = str(GrusBridge.command_feedback())
	_refresh_session()
	_reconcile_selection()
	_refresh_hud()
	if _underground_panel.visible \
			and (int(GrusBridge.sim_tick()) != _ug_last_tick or _session_phase != _ug_last_phase):
		_refresh_underground_panel()

func _icon_for_kind(group: String, kind: String) -> Texture2D:
	var path := "res://assets/ui/icons/%s/%s.png" % [group, kind.to_snake_case()]
	if not _icon_cache.has(path):
		_icon_cache[path] = ResourceLoader.load(path) as Texture2D if ResourceLoader.exists(path) else null
	return _icon_cache[path] as Texture2D

func _inline_icon(group: String, kind: String, size := 18) -> String:
	var icon := _icon_for_kind(group, kind)
	return "[img=%dx%d]%s[/img] " % [size, size, icon.resource_path] if icon != null else ""

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
	_refresh_session_overlay(snap)
	var result_frame := _session_phase == "Result" and previous_phase != "Result"
	if _session_phase != "Playing" and not result_frame:
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
			session_label.text = "Survive the alien waves"
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
		var attacker: Node3D = null
		var attacker_id := int(event.get("attacker", -1))
		match str(event.get("attacker_kind", "")):
			"unit":
				attacker = _unit_view(attacker_id)
			"building":
				attacker = _building_view(attacker_id)
		if attacker != null:
			_fx.spawn_tracer(attacker.global_position + Vector3(0.0, 0.8, 0.0), hit)
	_fx.spawn_hit(hit)
	if bool(event.get("killed", false)):
		_fx.spawn_death(hit)
	_fx.play_hit_cue()

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

func _refresh_hud() -> void:
	var economy: Dictionary = GrusBridge.economy_snapshot()
	var surface := float(economy.get("surface_efficiency", 1.0))
	var surface_percent := roundi(surface * 100.0)
	# A real deficit must never display as a full 100%: clamp the rounded
	# value down to 99 while the sim reports any efficiency shortfall.
	if surface_percent >= 100 and surface < 1.0:
		surface_percent = 99
	var cold_icon := _inline_icon("status", "SurfaceCold") if surface < 1.0 else ""
	economy_label.text = "%s%d   %s%d   %s%d\n%s%d   Population %d/%d\n%s%d / %d · %ssurface %d%%" % [
		_inline_icon("resources", "Food"),
		int(economy.get("food", 0)),
		_inline_icon("resources", "Scrap"),
		int(economy.get("scrap", 0)),
		_inline_icon("resources", "Coal"),
		int(economy.get("coal", 0)),
		_inline_icon("resources", "Crystals"),
		int(economy.get("crystals", 0)),
		int(economy.get("population_used", 0)),
		int(economy.get("population_cap", 0)),
		_inline_icon("resources", "Heat"),
		int(economy.get("heat_supply", 0)),
		int(economy.get("heat_demand", 0)),
		cold_icon,
		surface_percent,
	]
	idle_button.text = "Idle: %d — Next" % int(economy.get("idle_workers", 0))
	var wave: Dictionary = GrusBridge.wave_snapshot()
	var remaining := maxi(ceili(float(wave.get("seconds_remaining", 0.0))), 0)
	var incoming_names := PackedStringArray()
	for zone in wave.get("incoming_zones", []):
		incoming_names.append(str(zone.get("name", "")))
	var wave_number := int(wave.get("wave", 0))
	var current_label := "Prep" if wave_number == 0 else "Wave %d" % wave_number
	wave_label.text = "%s   %sNext wave %d in %d:%02d   Aliens %d" % [
		current_label,
		_inline_icon("status", "WaveCountdown"),
		int(wave.get("next_wave", 1)),
		int(remaining / 60),
		remaining % 60,
		int(wave.get("live_aliens", 0)),
	]
	if not incoming_names.is_empty():
		wave_label.text += "   %sIncoming: %s" % [
			_inline_icon("status", "IncomingZone"), ", ".join(incoming_names)]
	# One bridge read per frame: hand the same snapshot to the minimap so
	# HUD text and incoming pings can never disagree.
	if _minimap != null and _minimap.has_method("set_wave_snapshot"):
		_minimap.set_wave_snapshot(wave)
	# The one dynamic research read per HUD frame: drives the progress bar,
	# the ETA text, the Analyze enable and every build-button lock — the
	# frontend derives percent and availability from these fields only.
	var research: Dictionary = GrusBridge.research_snapshot()
	var building: Dictionary = {}
	if selected_building_id > 0:
		building = GrusBridge.building_snapshot(selected_building_id)
	_refresh_selection(building)
	_refresh_research_controls(research, building)

## Per-frame research refresh behind the one dynamic snapshot: the bar and
## label track live points/rate/ETA, and every catalogue-bound lock state
## (build buttons, Analyze) re-derives here — never a one-time _ready read.
func _refresh_research_controls(research: Dictionary, building: Dictionary) -> void:
	var points := float(research.get("points", 0.0))
	var target := maxf(float(_research_target_points), 1.0)
	research_bar.value = clampf(points, 0.0, target)
	# Floor the percent so a positive sub-total never displays an early
	# 100%; the target itself still reads 100%.
	var percent := floori(clampf(points, 0.0, target) / target * 100.0)
	var eta := float(research.get("eta_seconds", -1.0))
	var eta_text := "—"
	if eta >= 0.0:
		var total := maxi(ceili(eta), 0)
		eta_text = "%d:%02d" % [total / 60, total % 60]
	research_label.text = "Research %d/%d · %d%% · ETA %s" % [
		floori(points), _research_target_points, percent, eta_text,
	]
	# Analyze is enabled while Playing with the owned, completed Research
	# Center selected — affordability is deliberately NOT checked here:
	# the sim reports InsufficientResources through normal feedback.
	var analyze_context := _playing() and not building.is_empty() \
		and str(building.get("kind", "")) == "ResearchCenter" \
		and int(building.get("team_id", -1)) == 1 \
		and bool(building.get("complete", false))
	analyze_button.disabled = not analyze_context
	# Locked buttons unlock mid-match: the live point total vs each button's
	# catalogue-derived threshold, every frame. Rust still rejects a stale
	# click through the authoritative Locked gate.
	for button in _build_grid.get_children():
		var required := float(button.get_meta("unlock_points", 0))
		button.disabled = required > 0.0 and points < required

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
	if key.keycode == KEY_ESCAPE and _underground_panel.visible:
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
	# pick (worker gather/construction, enemy attack, rally, move).
	if armed and not selected_ids.is_empty():
		var target = _ground_target(screen_position)
		if target != null and GrusBridge.attack_move_units(PackedInt32Array(selected_ids), target):
			command_status.text = "Attack-move queued"
		else:
			command_status.text = "Attack-move rejected"
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

## Enemy picking reuses the existing _nearest_view filter and distance
## tie-break across both view groups.
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

## Building picking tests the projected art rect of the view's active
## Model — the silhouette itself, including height, at any zoom — exactly
## like _resource_at. The old center-radius stays as the fallback while
## metadata or art has not attached, and the closest eligible center wins
## on overlap. Hidden (fogged) views are never picked.
func _building_at(screen_position: Vector2, team_filter := 0) -> Node3D:
	var nearest: Node3D = null
	var nearest_distance := INF
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		if view == null or not view.is_visible_in_tree() \
				or camera.is_position_behind(view.global_position):
			continue
		if team_filter != 0 and int(view.get_meta("team_id", -1)) != team_filter:
			continue
		var center_px := camera.unproject_position(view.global_position)
		var distance := center_px.distance_to(screen_position)
		var model := view.get_node_or_null("Model") as Node3D
		var hit := distance <= CLICK_RADIUS
		if model != null and _model_art_usable(model):
			hit = _projected_art_rect(view, model).has_point(screen_position)
		if hit and distance < nearest_distance:
			nearest = view
			nearest_distance = distance
	return nearest

func _model_art_usable(model: Node3D) -> bool:
	for node in model.find_children("*", "MeshInstance3D", true, false):
		var mesh := node as MeshInstance3D
		if mesh.mesh != null and mesh.get_aabb().size.length() >= 0.001:
			return true
	return false

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
	_attack_move_armed = false
	_ug_armed_index = -1
	for kind_button in _ug_kind_buttons:
		kind_button.set_pressed_no_signal(false)
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
	var bounds := AABB()
	var first := true
	for node in (art_root if art_root != null else view).find_children("*", "MeshInstance3D", true, false):
		var mesh := node as MeshInstance3D
		var local: AABB = view.global_transform.affine_inverse() * mesh.global_transform * mesh.get_aabb()
		bounds = local if first else bounds.merge(local)
		first = false
	if bounds.size.length() < 0.001:
		# No art yet (kind meta arrives a frame late): fall back to the cell.
		bounds = AABB(Vector3(-0.5, 0.0, -0.5), Vector3(1.0, 1.0, 1.0))
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
	_placement_generation += 1
	_placement_kind = kind
	command_status.text = "Placing %s — left-click to place, right-click or Esc to cancel" % kind

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

## One Analyze conversion on the selected Research Center — the same
## queued-command/feedback path as every other order. The button's enabled
## state already encodes the session+selection context; the sim stays the
## final validator for ownership, kind and Crystals.
func _on_analyze_pressed() -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if selected_building_id <= 0:
		command_status.text = "Select the Research Center first"
		return
	if GrusBridge.analyze(selected_building_id):
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
	if _underground_panel.visible:
		_close_underground_panel()
	else:
		_underground_panel.visible = true
		# One immediate refresh so the first shown frame is current; the
		# status line keeps the last command feedback — panel state never
		# clobbers it (the Esc hint lives on the panel's own header).
		_refresh_underground_panel()

func _close_underground_panel() -> void:
	_underground_panel.visible = false

## Dynamic underground refresh, only while the panel is visible and gated
## by the applied-tick counter plus session phase: the snapshot can only
## change inside a fixed tick, so a mid-tick frame never rebuilds one.
## Level rows mirror the snapshot's level count — appended when a dig
## completes, rebuilt only on the depth-shrinking reseed — so a live press
## is never freed out from under a click. Live text (dig countdown, build
## progress, benefit lines, eligibility) refreshes per refresh from the
## read-only snapshot plus the static catalogue. No dig-price or
## room-effects math lives here — the snapshot carries the Rust-computed
## numbers.
func _refresh_underground_panel() -> void:
	_ug_last_tick = int(GrusBridge.sim_tick())
	_ug_last_phase = _session_phase
	var snap: Dictionary = GrusBridge.underground_snapshot()
	_sync_underground_rows(snap)
	_refresh_underground_header(snap)
	_refresh_underground_kinds(snap)
	_refresh_underground_slots(snap)

## Keep one row per snapshot level. Growth only ever appends (a level row
## appears exactly when a dig completes), so existing slot buttons — and
## any press in flight on them — are never freed mid-click. Depth shrinks
## only through a match reseed, the one case that still rebuilds.
func _sync_underground_rows(snap: Dictionary) -> void:
	var levels: Array = snap.get("levels", [])
	var rows := _underground_levels.get_child_count()
	if levels.size() < rows:
		for child in _underground_levels.get_children():
			_underground_levels.remove_child(child)
			child.queue_free()
		rows = 0
	for index in range(rows, levels.size()):
		_add_underground_row(int(levels[index].get("level", index + 1)))

func _add_underground_row(level_number: int) -> void:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 2)
	var level_label := Label.new()
	level_label.text = "L%d" % level_number
	level_label.custom_minimum_size = Vector2(24, 48)
	level_label.add_theme_font_size_override("font_size", 14)
	row.add_child(level_label)
	for slot_index in _ug_slots_per_level:
		var button := Button.new()
		button.add_theme_font_size_override("font_size", 13)
		button.add_theme_color_override("font_disabled_color", Color(0.8, 0.8, 0.8))
		button.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
		button.custom_minimum_size = Vector2(84, 48)
		button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		button.pressed.connect(_on_slot_pressed.bind(level_number, slot_index))
		row.add_child(button)
	_underground_levels.add_child(row)

func _refresh_underground_header(snap: Dictionary) -> void:
	var depth := int(snap.get("depth", 0))
	var target := int(snap.get("target", 1))
	var cost_text := _ug_cost_text(snap.get("next_cost", {}))
	var dig_line := ""
	if bool(snap.get("excavating", false)):
		dig_line = "Digging L%d · %d%% · %ds left" % [
			target, roundi(float(snap.get("progress", 0.0)) * 100.0),
			maxi(ceili(float(snap.get("remaining", 0.0))), 0)]
	else:
		dig_line = "Dig L%d · %s · %ds" % [target, cost_text, int(snap.get("next_seconds", 0))]
	var lock_line := ""
	if _session_phase == "Paused":
		lock_line = "\nPaused — inspection only"
	elif not _playing():
		lock_line = "\n%s — inspection only" % _session_phase
	_underground_header.text = "Underground — depth %d · Esc closes\n%s%s" % [
		depth, dig_line, lock_line]
	if bool(snap.get("excavating", false)):
		_underground_excavate.disabled = true
		_underground_excavate.text = "Digging L%d · %d%%" % [
			target, roundi(float(snap.get("progress", 0.0)) * 100.0)]
	else:
		_underground_excavate.disabled = not _playing()
		# The header line always carries the full price/time; the button keeps
		# the terse form so deeper (longer) prices can never clip.
		_underground_excavate.text = "Excavate L%d" % target

## A kind Button becomes selectable once the completed depth reaches its
## catalogue min_level; the pressed look marks the armed kind.
func _refresh_underground_kinds(snap: Dictionary) -> void:
	var depth := int(snap.get("depth", 0))
	for index in _ug_kind_buttons.size():
		var button := _ug_kind_buttons[index]
		var min_level := int(_ug_rooms[index].get("min_level", 1))
		button.disabled = depth < min_level
		var name_label := button.get_node_or_null("KindLabel") as Label
		if name_label != null:
			var color_name := "font_disabled_color" if button.disabled else "font_color"
			name_label.add_theme_color_override("font_color", button.get_theme_color(color_name))

## Formats a cost dictionary carried by a snapshot or catalogue entry;
## GDScript renders, Rust computes.
func _ug_cost_text(cost: Dictionary) -> String:
	var parts := PackedStringArray()
	for resource in ["food", "scrap", "coal", "crystals"]:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%d %s" % [amount, resource.capitalize()])
	return " + ".join(parts)

func _underground_display_name(kind: String) -> String:
	return str(UNDERGROUND_DISPLAY_NAMES.get(kind, kind))

## The full kind tooltip: catalogue name, cost, build time and the
## Rust-computed effect line — straight off the static catalogue row,
## never retyped here.
func _ug_kind_tooltip(entry: Dictionary) -> String:
	return "%s — Requires L%d\nCost: %s · Build: %ds\n%s" % [
		str(entry.get("name", "")), int(entry.get("min_level", 1)), _ug_cost_text(entry),
		int(entry.get("seconds", 0)), str(entry.get("effect", ""))]

## Per-refresh slot update against the live snapshot and the armed kind:
## empty slots act only with a kind armed, only on rows at or above its
## min_level, and only while Playing; shallower rows stay disabled and say
## why. Occupied/constructing slots are never actionable.
func _refresh_underground_slots(snap: Dictionary) -> void:
	var levels: Array = snap.get("levels", [])
	var armed := _ug_armed_index >= 0 and _ug_armed_index < _ug_rooms.size()
	var armed_entry: Dictionary = _ug_rooms[_ug_armed_index] if armed else {}
	var armed_min := int(armed_entry.get("min_level", 1)) if armed else 0
	for row_index in mini(_underground_levels.get_child_count(), levels.size()):
		var row: Node = _underground_levels.get_child(row_index)
		var level_number := int(levels[row_index].get("level", 0))
		var slots: Array = levels[row_index].get("slots", [])
		for slot_index in mini(_ug_slots_per_level, slots.size()):
			var button := row.get_child(slot_index + 1) as Button
			var slot: Dictionary = slots[slot_index]
			var kind := str(slot.get("kind", ""))
			if kind == "":
				button.icon = null
				if armed and level_number < armed_min:
					button.text = "Requires\nL%d" % armed_min
					button.tooltip_text = "%s requires L%d" % [
						str(armed_entry.get("name", "")), armed_min]
					button.disabled = true
				elif armed:
					button.text = "Empty"
					button.tooltip_text = _ug_kind_tooltip(armed_entry)
					button.disabled = not _playing()
				else:
					button.text = "Empty"
					button.tooltip_text = "Arm a room kind above"
					button.disabled = true
				continue
			var display := _underground_display_name(kind)
			button.icon = _icon_for_kind("underground", kind)
			if button.icon != null:
				button.add_theme_constant_override("icon_max_width", 20)
				button.add_theme_color_override("icon_disabled_color", Color(1, 1, 1, 0.85))
			if bool(slot.get("complete", false)):
				button.text = "%s\n%s" % [display, str(slot.get("benefit", ""))]
				button.tooltip_text = "%s — complete: %s" % [kind, str(slot.get("benefit", ""))]
			else:
				var progress := roundi(float(slot.get("progress", 0.0)) * 100.0)
				button.text = "%s\n%d%%" % [display, progress]
				button.tooltip_text = "%s — building, %ds left" % [
					kind, maxi(ceili(float(slot.get("remaining", 0.0))), 0)]
			button.disabled = true

## Arming is the panel's only selection state: pressing a kind button arms
## it (exclusively), pressing the armed one again disarms; slot eligibility
## recomputes from the catalogue + snapshot on the next refresh.
func _on_kind_toggled(pressed: bool, index: int) -> void:
	if pressed:
		_ug_armed_index = index
		for other in _ug_kind_buttons.size():
			_ug_kind_buttons[other].set_pressed_no_signal(other == index)
		command_status.text = "%s armed — click an eligible slot" % str(_ug_rooms[index].get("name", ""))
	else:
		_ug_armed_index = -1
	_refresh_underground_panel()

## One paid excavation — the same queued-command/feedback path as every
## other order; the sim owns the single-dig rule and affordability.
func _on_excavate_pressed() -> void:
	if not _playing():
		command_status.text = "Match is not playing"
		return
	if GrusBridge.excavate():
		command_status.text = "Excavate queued"
	else:
		command_status.text = "Excavate rejected"

## An eligible empty-slot click queues the armed kind at this 1-based level
## and 0-based slot; disabled buttons never fire, and Rust revalidates
## everything authoritatively.
func _on_slot_pressed(level: int, slot: int) -> void:
	if not _playing() or _ug_armed_index < 0 or _ug_armed_index >= _ug_rooms.size():
		return
	var kind := str(_ug_rooms[_ug_armed_index].get("name", ""))
	if kind == "":
		return
	if GrusBridge.build_room(level, slot, kind):
		command_status.text = "%s queued (L%d slot %d)" % [
			_underground_display_name(kind), level, slot + 1]
	else:
		command_status.text = "Room command rejected"
