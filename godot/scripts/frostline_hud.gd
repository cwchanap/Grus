extends CanvasLayer

## Presentation-only HUD view attached directly to the existing HUD
## CanvasLayer. The battlefield controller stays the one owner of selection,
## session, snapshots and command feedback: it reads the read-only bridge
## snapshots once per frame and hands them to these render calls. This script
## caches no snapshot and never issues a command; the one static catalogue
## read in _ready exists only to render fixed display text and tooltips.

@onready var wave_label: RichTextLabel = $WaveLabel
@onready var research_bar: ProgressBar = $ResearchPanel/ResearchBar
@onready var research_label: Label = $ResearchPanel/ResearchLabel
@onready var research_icon: TextureRect = $ResearchPanel/ResearchIcon
@onready var analyze_button: Button = $ResearchPanel/AnalyzeButton
@onready var economy_label: RichTextLabel = $EconomyPanel/EconomyLabel
@onready var idle_button: Button = $EconomyPanel/IdleButton
@onready var selection_label: Label = $SelectionPanel/SelectionLabel
@onready var session_panel: Panel = $SessionPanel
@onready var session_label: Label = $SessionPanel/SessionLabel
@onready var start_button: Button = $SessionPanel/StartButton
@onready var resume_button: Button = $SessionPanel/ResumeButton
@onready var restart_button: Button = $SessionPanel/RestartButton
@onready var quit_button: Button = $SessionPanel/QuitButton
@onready var pause_button: Button = $PauseButton
@onready var _build_grid: Container = $CommandPanel/BuildGrid
@onready var _train_grid: Container = $CommandPanel/TrainGrid
@onready var _minimap: Control = $Minimap
## Static research tuning, cached once from the catalogue's "research" block
## for the bar's maximum; live points/rate/ETA arrive per frame through
## refresh_research_controls — never a mirrored mutable copy.
var _research_target_points := 0
var _icon_cache: Dictionary = {}

func _ready() -> void:
	var research_progress_icon := _icon_for_kind("status", "ResearchProgress")
	research_icon.texture = research_progress_icon
	research_icon.visible = research_progress_icon != null
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var buildings: Dictionary = catalogue.get("buildings", {})
	var units: Dictionary = catalogue.get("units", {})
	var research_catalogue: Dictionary = catalogue.get("research", {})
	_research_target_points = int(research_catalogue.get("target_points", 0))
	var analyze_cost: Dictionary = research_catalogue.get("analyze_cost", {})
	var analyze_crystals := int(analyze_cost.get("crystals", 0))
	var analyze_gain := int(research_catalogue.get("analyze_gain_points", 0))
	research_bar.max_value = maxf(float(_research_target_points), 1.0)
	analyze_button.text = "Analyze %d→+%d" % [analyze_crystals, analyze_gain]
	analyze_button.tooltip_text = "Convert %d Crystals into %d research points" % [analyze_crystals, analyze_gain]
	analyze_button.icon = _icon_for_kind("status", "ResearchRate")
	analyze_button.add_theme_constant_override("icon_max_width", 20)
	pause_button.icon = _icon_for_kind("actions", "Pause")
	pause_button.add_theme_constant_override("icon_max_width", 30)
	pause_button.text = ""
	idle_button.icon = _icon_for_kind("status", "IdleWorkers")
	idle_button.add_theme_constant_override("icon_max_width", 22)
	for button in _build_grid.get_children():
		var kind := str(button.get_meta("kind"))
		var entry: Dictionary = buildings.get(kind, {})
		var building_icon := _icon_for_kind("buildings", kind)
		if building_icon != null:
			button.icon = building_icon
			button.add_theme_constant_override("icon_max_width", 28)
		button.set_meta("cost", entry)
		_set_tile_cost(button, entry)
		var costs := PackedStringArray()
		for resource in ["food", "scrap", "coal", "crystals"]:
			var amount := int(entry.get(resource, 0))
			if amount > 0:
				costs.append("%d %s" % [amount, resource.capitalize()])
		button.tooltip_text = "Cost: %s\nBuild: %ds" % [", ".join(costs), int(entry.get("seconds", 0))]
		# The catalogue-derived research threshold rides the button meta; the
		# per-frame lock refresh compares it against the live point total. No
		# per-kind conditions.
		var unlock_points := int(entry.get("unlock_points", 0))
		button.set_meta("unlock_points", unlock_points)
		if unlock_points > 0:
			button.tooltip_text += "\nUnlocks at %d research" % unlock_points
	for button in _train_grid.get_children():
		var kind := str(button.get_meta("kind"))
		var entry: Dictionary = units.get(kind, {})
		var unit_icon := _icon_for_kind("units", kind)
		if unit_icon != null:
			button.icon = unit_icon
			button.add_theme_constant_override("icon_max_width", 28)
		button.set_meta("cost", entry)
		_set_tile_cost(button, entry)

## The HPA-695 action-tile scene draws its catalogue price under the icon;
## the markup keeps the painted resource vocabulary without a second cost
## table. The tile stays a plain Button and the tooltip keeps the full text.
func _set_tile_cost(button: Button, cost: Dictionary) -> void:
	var label := button.get_node_or_null("Cost") as RichTextLabel
	if label == null:
		return
	var parts := PackedStringArray()
	for resource in ["food", "scrap", "coal", "crystals"]:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%s%d" % [_inline_icon("resources", resource.capitalize(), 12).strip_edges(), amount])
	label.text = "[center]%s[/center]" % " ".join(parts)

## Resource bar plus the idle-worker jump label, straight off the controller's
## once-per-frame economy snapshot.
func refresh_economy(economy: Dictionary) -> void:
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
	_refresh_affordability(economy)

## Display-only affordability hint, derived from the catalogue prices already
## on each tile and the live economy snapshot. The tile stays enabled: Rust
## remains the final validator and must surface InsufficientResources.
func _refresh_affordability(economy: Dictionary) -> void:
	for grid in [_build_grid, _train_grid]:
		for button in grid.get_children():
			var cost: Dictionary = button.get_meta("cost", {})
			var affordable := true
			for resource in ["food", "scrap", "coal", "crystals"]:
				if int(cost.get(resource, 0)) > int(economy.get(resource, 0)):
					affordable = false
					break
			var variation: StringName = &"UnaffordableTile" if not affordable else &"Tile"
			if button.theme_type_variation != variation:
				button.theme_type_variation = variation

## Wave line and the minimap pings: one controller read per frame feeds both,
## so HUD text and incoming pings can never disagree.
func refresh_wave(wave: Dictionary) -> void:
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
	if _minimap != null and _minimap.has_method("set_wave_snapshot"):
		_minimap.set_wave_snapshot(wave)

## Per-frame research refresh behind the one dynamic snapshot: the bar and
## label track live points/rate/ETA, and every catalogue-bound lock state
## (build buttons, Analyze) re-derives here — never a one-time _ready read.
## The controller owns the session phase and passes its Playing verdict.
func refresh_research_controls(research: Dictionary, building: Dictionary, playing: bool) -> void:
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
	var analyze_context := playing and not building.is_empty() \
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

## Renders the controller-owned selection; unit kinds come from the same
## per-frame view map the controller uses for command context.
func refresh_selection(selected_ids: Array[int], selected_building_id: int, building: Dictionary, unit_kinds: Dictionary) -> void:
	if not selected_ids.is_empty():
		var labels := PackedStringArray()
		for id in selected_ids:
			labels.append("%s #%d" % [str(unit_kinds.get(id, "")), id])
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

## Session overlay visibility and wording from the controller's settled phase;
## the buttons themselves stay connected and dispatched by the controller.
func refresh_session(phase: String, winner_team: int) -> void:
	var playing := phase == "Playing"
	session_panel.visible = not playing
	pause_button.visible = playing
	# Restart/Quit belong to Result only.
	match phase:
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
			session_label.text = "Victory!" if winner_team == 1 else "Defeat"
			start_button.visible = false
			resume_button.visible = false
			restart_button.visible = true
			quit_button.visible = true
		_:
			# Playing: panel hidden.
			pass

func _icon_for_kind(group: String, kind: String) -> Texture2D:
	var path := "res://assets/ui/icons/%s/%s.png" % [group, kind.to_snake_case()]
	if not _icon_cache.has(path):
		_icon_cache[path] = ResourceLoader.load(path) as Texture2D if ResourceLoader.exists(path) else null
	return _icon_cache[path] as Texture2D

func _inline_icon(group: String, kind: String, size := 18) -> String:
	var icon := _icon_for_kind(group, kind)
	return "[img=%dx%d]%s[/img] " % [size, size, icon.resource_path] if icon != null else ""
