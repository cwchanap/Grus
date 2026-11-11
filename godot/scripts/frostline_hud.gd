extends CanvasLayer

## Presentation-only HUD view attached directly to the existing HUD
## CanvasLayer. The battlefield controller stays the one owner of selection,
## session, snapshots and command feedback: it reads the read-only bridge
## snapshots once per frame and hands them to these render calls. This script
## caches no snapshot and never issues a command; the one static catalogue
## read in _ready exists only to render fixed display text and tooltips.

@onready var wave_label: RichTextLabel = $WaveLabel
@onready var wave_ring: Control = $WaveRing
@onready var wave_clock: TextureRect = $WaveRing/ClockIcon
@onready var incoming_glow: Control = $IncomingGlow
@onready var research_bar: ProgressBar = $ResearchPanel/ResearchBar
@onready var research_label: Label = $ResearchPanel/ResearchLabel
@onready var research_icon: TextureRect = $ResearchPanel/ResearchIcon
@onready var analyze_button: Button = $ResearchPanel/AnalyzeButton
@onready var economy_label: RichTextLabel = $EconomyPanel/EconomyLabel
@onready var heat_bar: ProgressBar = $EconomyPanel/HeatBar
@onready var cold_chip: RichTextLabel = $EconomyPanel/ColdChip
@onready var idle_button: Button = $EconomyPanel/IdleButton
@onready var selection_label: Label = $SelectionPanel/SelectionLabel
@onready var selection_name: Label = $SelectionPanel/SelectionName
@onready var selection_panel: Panel = $SelectionPanel
@onready var portrait_icon: TextureRect = $SelectionPanel/PortraitIcon
@onready var health_bar: ProgressBar = $SelectionPanel/HealthBar
@onready var health_label: Label = $SelectionPanel/HealthLabel
@onready var carry_bar: ProgressBar = $SelectionPanel/CarryBar
@onready var carry_label: RichTextLabel = $SelectionPanel/CarryLabel
@onready var queue_sockets: Container = $SelectionPanel/QueueSockets
@onready var session_panel: Panel = $SessionPanel
@onready var session_label: Label = $SessionPanel/SessionLabel
@onready var start_button: Button = $SessionPanel/StartButton
@onready var resume_button: Button = $SessionPanel/ResumeButton
@onready var restart_button: Button = $SessionPanel/RestartButton
@onready var quit_button: Button = $SessionPanel/QuitButton
@onready var pause_button: Button = $PauseButton
@onready var _build_grid: Container = $CommandPanel/BuildGrid
@onready var _train_grid: Container = $CommandPanel/TrainGrid
@onready var _order_grid: Container = $CommandPanel/OrderGrid
@onready var _minimap: Control = $Minimap
## Static research tuning, cached once from the catalogue's "research" block
## for the bar's maximum; live points/rate/ETA arrive per frame through
## refresh_research_controls — never a mirrored mutable copy.
var _research_target_points := 0
var _icon_cache: Dictionary = {}
## Static catalogue rows (price/time/HP/combat/producer), read once for
## display text and tooltips; live values keep arriving per frame.
var _unit_entries: Dictionary = {}
var _building_entries: Dictionary = {}
var _carry_limit := 0
var _producer_kinds: Dictionary = {}
## Five queue sockets plus one overflow chip, built once and reused; the
## live FIFO arrives through the selected building's read-only snapshot.
const QUEUE_SOCKET_COUNT := 5
var _socket_icons: Array[TextureRect] = []
var _socket_times: Array[Label] = []
var _queue_overflow: Label
## Last producer kind the train grid was filtered for, so visibility only
## changes when the selection context does.
var _train_producer_kind := "<unset>"

func _ready() -> void:
	var research_progress_icon := _icon_for_kind("status", "ResearchProgress")
	research_icon.texture = research_progress_icon
	research_icon.visible = research_progress_icon != null
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var buildings: Dictionary = catalogue.get("buildings", {})
	var units: Dictionary = catalogue.get("units", {})
	var research_catalogue: Dictionary = catalogue.get("research", {})
	_building_entries = buildings
	_unit_entries = units
	_carry_limit = int(catalogue.get("carry_limit", 0))
	for kind in units:
		var producer := str(units[kind].get("producer", ""))
		if producer != "":
			_producer_kinds[producer] = true
	_build_common_tiles(buildings, units)
	_build_order_tiles()
	_build_queue_sockets()
	wave_clock.texture = _icon_for_kind("status", "WaveCountdown")
	_build_glow_textures()
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

func _build_order_tiles() -> void:
	for button in _order_grid.get_children():
		if not button is Button:
			continue
		var order := str(button.get_meta("order", ""))
		if order == "cancel":
			button.text = "✕"
			button.add_theme_font_size_override("font_size", 22)
			continue
		var icon := _icon_for_kind("actions", order)
		button.icon = icon
		button.add_theme_constant_override("icon_max_width", 26)

## The HUD only mirrors the controller's one armed-order string: pressed tile
## for the modal order, disabled outside Playing. It never stores the state.
func refresh_orders(armed_order: String, playing: bool) -> void:
	for button in _order_grid.get_children():
		if not button is Button:
			continue
		var order := str(button.get_meta("order", ""))
		button.disabled = not playing
		if button.toggle_mode:
			button.set_pressed_no_signal(order == armed_order)

func _build_common_tiles(buildings: Dictionary, units: Dictionary) -> void:
	for button in _build_grid.get_children():
		var kind := str(button.get_meta("kind"))
		var entry: Dictionary = buildings.get(kind, {})
		var building_icon := _icon_for_kind("buildings", kind)
		if building_icon != null:
			button.icon = building_icon
			button.add_theme_constant_override("icon_max_width", 44)
		button.set_meta("cost", entry)
		_set_tile_cost(button, entry)
		button.tooltip_text = _building_tooltip(kind, entry)
		# The catalogue-derived research threshold rides the button meta; the
		# per-frame lock refresh compares it against the live point total.
		var unlock_points := int(entry.get("unlock_points", 0))
		button.set_meta("unlock_points", unlock_points)
		if unlock_points > 0:
			button.tooltip_text += "\nUnlocks at %d research" % unlock_points
		_add_counter_icon(button, entry)
	for button in _train_grid.get_children():
		var kind := str(button.get_meta("kind"))
		var entry: Dictionary = units.get(kind, {})
		var unit_icon := _icon_for_kind("units", kind)
		if unit_icon != null:
			button.icon = unit_icon
			button.add_theme_constant_override("icon_max_width", 44)
		button.set_meta("cost", entry)
		button.set_meta("producer", str(entry.get("producer", "")))
		_set_tile_cost(button, entry)
		button.tooltip_text = _unit_tooltip(entry)
		_add_counter_icon(button, entry)

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
			parts.append("%s%d" % [_inline_icon("resources", resource.capitalize(), 14).strip_edges(), amount])
	label.text = "[center]%s[/center]" % " ".join(parts)

## Full text of the four-currency catalogue price, one spelling for the
## tooltip and the underground rows.
func _cost_text(cost: Dictionary) -> String:
	var parts := PackedStringArray()
	for resource in ["food", "scrap", "coal", "crystals"]:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%d %s" % [amount, resource.capitalize()])
	return ", ".join(parts) if not parts.is_empty() else "free"

## Optional combat/counter row shared by unit and building tooltips.
func _combat_line(entry: Dictionary) -> String:
	if not entry.has("damage"):
		return ""
	var line := "\nDamage %d · Range %s" % [
		int(entry.get("damage", 0)), _number(float(entry.get("range", 0.0)))]
	var target := str(entry.get("counter_target", ""))
	if target != "":
		line += " · Strong vs %s (+%d)" % [target, int(entry.get("counter_bonus", 0))]
	return line

func _number(value: float) -> String:
	return String.num(value, 1).trim_suffix(".0")

func _unit_tooltip(entry: Dictionary) -> String:
	return "Cost: %s\nTrain: %ds\nHP: %d%s" % [
		_cost_text(entry), int(entry.get("seconds", 0)),
		int(entry.get("max_health", 0)), _combat_line(entry)]

func _building_tooltip(kind: String, entry: Dictionary) -> String:
	var text := "Cost: %s\nBuild: %ds\nHP: %d" % [
		_cost_text(entry), int(entry.get("seconds", 0)), int(entry.get("max_health", 0))]
	var supply := int(entry.get("heat_supply", 0))
	var demand := int(entry.get("heat_demand", 0))
	if supply > 0 or demand > 0:
		text += "\nHeat: +%d supply · %d demand" % [supply, demand]
	text += _combat_line(entry)
	return text

## Guard/Rifleman and combat buildings illustrate their catalogue counter
## with the already-shipped alien icon in the tile corner. Display only.
func _add_counter_icon(button: Button, entry: Dictionary) -> void:
	var target := str(entry.get("counter_target", ""))
	if target == "" or not entry.has("damage"):
		return
	var icon := _icon_for_kind("units", target)
	if icon == null:
		return
	var rect := TextureRect.new()
	rect.name = "CounterIcon"
	rect.texture = icon
	rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	rect.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	rect.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	rect.tooltip_text = "Strong vs %s (+%d)" % [target, int(entry.get("counter_bonus", 0))]
	rect.set_anchors_preset(Control.PRESET_TOP_RIGHT)
	rect.offset_left = -26.0
	rect.offset_top = 2.0
	rect.offset_right = -4.0
	rect.offset_bottom = 24.0
	button.add_child(rect)

## Static queue sockets: five FIFO frames and a "+N" overflow chip built
## once, never per frame. The live icons/times/progress update on refresh.
func _build_queue_sockets() -> void:
	var socket_style := StyleBoxFlat.new()
	socket_style.bg_color = Color(0.09, 0.13, 0.17, 0.96)
	socket_style.border_color = Color(0.4, 0.49, 0.56, 0.95)
	socket_style.set_border_width_all(1)
	socket_style.set_corner_radius_all(4)
	for index in QUEUE_SOCKET_COUNT:
		var socket := Panel.new()
		socket.name = "Socket%d" % (index + 1)
		socket.custom_minimum_size = Vector2(62, 44)
		socket.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		socket.mouse_filter = Control.MOUSE_FILTER_IGNORE
		socket.add_theme_stylebox_override("panel", socket_style)
		var icon := TextureRect.new()
		icon.name = "Icon"
		icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
		icon.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		icon.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
		icon.set_anchors_preset(Control.PRESET_FULL_RECT)
		icon.offset_top = 2.0
		icon.offset_bottom = -12.0
		icon.offset_left = 6.0
		icon.offset_right = -6.0
		socket.add_child(icon)
		var time := Label.new()
		time.name = "Time"
		time.mouse_filter = Control.MOUSE_FILTER_IGNORE
		time.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		time.add_theme_font_size_override("font_size", 10)
		time.set_anchors_preset(Control.PRESET_BOTTOM_WIDE)
		time.offset_top = -14.0
		time.offset_bottom = -1.0
		socket.add_child(time)
		queue_sockets.add_child(socket)
		_socket_icons.append(icon)
		_socket_times.append(time)
	_queue_overflow = Label.new()
	_queue_overflow.name = "OverflowChip"
	_queue_overflow.custom_minimum_size = Vector2(36, 44)
	_queue_overflow.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_queue_overflow.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_queue_overflow.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_queue_overflow.add_theme_font_size_override("font_size", 13)
	_queue_overflow.add_theme_color_override("font_color", Color(0.98, 0.78, 0.45))
	var chip := StyleBoxFlat.new()
	chip.bg_color = Color(0.16, 0.12, 0.07, 0.96)
	chip.border_color = Color(0.93, 0.55, 0.22, 0.9)
	chip.set_border_width_all(1)
	chip.set_corner_radius_all(4)
	_queue_overflow.add_theme_stylebox_override("normal", chip)
	queue_sockets.add_child(_queue_overflow)

## One amber gradient per screen edge, faded inward. The four overlays are
## noninteractive and only the zone names in the live wave snapshot show them.
func _build_glow_textures() -> void:
	var directions := {
		"North": [Vector2(0.5, 0.0), Vector2(0.5, 1.0)],
		"South": [Vector2(0.5, 1.0), Vector2(0.5, 0.0)],
		"West": [Vector2(0.0, 0.5), Vector2(1.0, 0.5)],
		"East": [Vector2(1.0, 0.5), Vector2(0.0, 0.5)],
	}
	for direction in directions:
		var gradient := Gradient.new()
		gradient.set_color(0, Color(0.96, 0.58, 0.16, 0.45))
		gradient.set_color(1, Color(0.96, 0.58, 0.16, 0.0))
		var texture := GradientTexture2D.new()
		texture.gradient = gradient
		texture.width = 64
		texture.height = 64
		texture.fill = GradientTexture2D.FILL_LINEAR
		texture.fill_from = directions[direction][0]
		texture.fill_to = directions[direction][1]
		var edge := incoming_glow.get_node(direction) as TextureRect
		edge.texture = texture
		edge.mouse_filter = Control.MOUSE_FILTER_IGNORE

## Resource bar plus the idle-worker jump label, straight off the controller's
## once-per-frame economy snapshot. One horizontal icon-first row: painted
## 28 px resource/population/heat icons with live values, matching the
## approved top bar instead of a compressed text block.
func refresh_economy(economy: Dictionary) -> void:
	var surface := float(economy.get("surface_efficiency", 1.0))
	var surface_percent := roundi(surface * 100.0)
	# A real deficit must never display as a full 100%: clamp the rounded
	# value down to 99 while the sim reports any efficiency shortfall.
	if surface_percent >= 100 and surface < 1.0:
		surface_percent = 99
	economy_label.text = "%s%d   %s%d   %s%d   %s%d   %s%d / %d   %s%d / %d · surface %d%%" % [
		_inline_icon("resources", "Food", 28),
		int(economy.get("food", 0)),
		_inline_icon("resources", "Scrap", 28),
		int(economy.get("scrap", 0)),
		_inline_icon("resources", "Coal", 28),
		int(economy.get("coal", 0)),
		_inline_icon("resources", "Crystals", 28),
		int(economy.get("crystals", 0)),
		_inline_icon("status", "Population", 28),
		int(economy.get("population_used", 0)),
		int(economy.get("population_cap", 0)),
		_inline_icon("resources", "Heat", 28),
		int(economy.get("heat_supply", 0)),
		int(economy.get("heat_demand", 0)),
		surface_percent,
	]
	# The supply-versus-demand bar and cold chip are display-only projections
	# of the same live snapshot; both are hidden at full surface efficiency.
	var supply := int(economy.get("heat_supply", 0))
	var demand := int(economy.get("heat_demand", 0))
	var deficit := surface < 1.0
	heat_bar.visible = deficit
	cold_chip.visible = deficit
	if deficit:
		var denominator := maxi(demand, 1)
		heat_bar.max_value = float(denominator)
		heat_bar.value = float(mini(supply, denominator))
		var detail := "Heat supply %d / demand %d · surface %d%%" % [
			supply, demand, surface_percent]
		heat_bar.tooltip_text = detail
		cold_chip.tooltip_text = detail
		cold_chip.text = "%s %d%%" % [
			_inline_icon("status", "SurfaceCold", 20).strip_edges(), surface_percent]
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
## so HUD text and incoming pings can never disagree. The countdown ring is
## exactly seconds_remaining / timer_total_seconds and its amber state plus
## the directional edge glows are exactly "incoming_zones is nonempty".
func refresh_wave(wave: Dictionary) -> void:
	var remaining := maxi(ceili(float(wave.get("seconds_remaining", 0.0))), 0)
	var remaining_seconds := maxf(float(wave.get("seconds_remaining", 0.0)), 0.0)
	var total_seconds := maxf(float(wave.get("timer_total_seconds", 0.0)), 1.0)
	var incoming_names := PackedStringArray()
	for zone in wave.get("incoming_zones", []):
		incoming_names.append(str(zone.get("name", "")))
	var warning := not incoming_names.is_empty()
	wave_ring.set_state(remaining_seconds / total_seconds, warning)
	for direction in ["North", "East", "South", "West"]:
		(incoming_glow.get_node(direction) as TextureRect).visible = \
			incoming_names.has(direction)
	var wave_number := int(wave.get("wave", 0))
	var current_label := "Prep" if wave_number == 0 else "Wave %d" % wave_number
	wave_label.text = "%s\n%sNext wave %d in %d:%02d\nAliens %d" % [
		current_label,
		_inline_icon("status", "WaveCountdown"),
		int(wave.get("next_wave", 1)),
		int(remaining / 60),
		remaining % 60,
		int(wave.get("live_aliens", 0)),
	]
	if warning:
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
## per-frame view map the controller uses for command context. Health is the
## live view ratio times the catalogue max_health for exactly one unit or
## building — a mixed selection renders no aggregate number. The queue is a
## projection of the real training_queue snapshot, never a second queue.
func refresh_selection(selected_ids: Array[int], selected_building_id: int, building: Dictionary,
		unit_kinds: Dictionary, unit_task: Dictionary, unit_health: float,
		building_health: float) -> void:
	_clear_selection_detail()
	if selected_ids.size() == 1:
		var id := selected_ids[0]
		var kind := str(unit_kinds.get(id, ""))
		var entry: Dictionary = _unit_entries.get(kind, {})
		selection_name.text = "%s #%d" % [kind if kind != "" else "Unit", id]
		portrait_icon.texture = _icon_for_kind("units", kind)
		_render_health(unit_health, int(entry.get("max_health", 0)))
		selection_panel.tooltip_text = "%s\n%s" % [kind, _unit_tooltip(entry)]
		var status := _task_text(unit_task)
		selection_label.text = status if status != "" else "Ready"
		_set_train_producer("")
		if kind == "Worker":
			carry_bar.visible = true
			carry_label.visible = true
			var amount := int(unit_task.get("carry_amount", 0))
			var carried := str(unit_task.get("carry_kind", ""))
			carry_bar.max_value = maxf(float(_carry_limit), 1.0)
			carry_bar.value = float(amount)
			var icon := _inline_icon("resources", carried, 14) if carried != "" else ""
			carry_label.text = "%s%d / %d" % [icon, amount, _carry_limit]
		return
	if selected_ids.size() > 1:
		var common := str(unit_kinds.get(selected_ids[0], ""))
		for id in selected_ids:
			if str(unit_kinds.get(id, "")) != common:
				common = ""
				break
		selection_name.text = "%s ×%d" % [common if common != "" else "Units", selected_ids.size()]
		selection_label.text = "Multiple units selected"
		_set_train_producer("")
		return
	if selected_building_id > 0 and not building.is_empty():
		var kind := str(building.get("kind", "?"))
		var entry: Dictionary = _building_entries.get(kind, {})
		_set_train_producer(kind)
		selection_name.text = "%s #%d" % [kind, selected_building_id]
		portrait_icon.texture = _icon_for_kind("buildings", kind)
		_render_health(building_health, int(entry.get("max_health", 0)))
		selection_panel.tooltip_text = "%s\n%s" % [kind, _building_tooltip(kind, entry)]
		var lines := PackedStringArray()
		if bool(building.get("complete", false)):
			lines.append("Construction complete")
		else:
			lines.append("Construction %d%%" % roundi(float(building.get("construction_progress", 0.0)) * 100.0))
		var blocked := int(building.get("blocked_reason", 0))
		if blocked != 0:
			lines.append("Blocked (code %d)" % blocked)
		if int(building.get("rally_x", -1)) >= 0:
			lines.append("Rally (%d, %d)" % [int(building.get("rally_x", -1)), int(building.get("rally_y", -1))])
		selection_label.text = "\n".join(lines)
		if _producer_kinds.has(kind):
			_render_queue(building)
		return
	selection_name.text = "Nothing selected"
	_set_train_producer("")

## One live unit/building health readout: clampi(round(ratio * max), 0, max),
## hidden whenever there is no single live view or no catalogue maximum.
func _render_health(ratio: float, max_health: int) -> void:
	if ratio < 0.0 or max_health <= 0:
		return
	var current := clampi(roundi(ratio * max_health), 0, max_health)
	health_bar.visible = true
	health_bar.max_value = float(max_health)
	health_bar.value = float(current)
	health_label.visible = true
	health_label.text = "%d / %d" % [current, max_health]

## Worker task line: the sim's task name plus the id it names; empty for a
## non-worker, which then reads Ready.
func _task_text(task: Dictionary) -> String:
	var name := str(task.get("task", ""))
	if name == "":
		return ""
	var text := name
	match name:
		"ToSource":
			text = "Moving to source"
		"Gathering":
			text = "Gathering"
		"ToDropoff":
			text = "Returning to depot"
		"ToConstruction":
			text = "Moving to build site"
		"Constructing":
			text = "Constructing"
		"Idle":
			text = "Idle"
	if int(task.get("source", -1)) > 0:
		text += " · source #%d" % int(task.get("source", -1))
	elif int(task.get("building", -1)) > 0:
		text += " · building #%d" % int(task.get("building", -1))
	return text

## First five FIFO jobs in sockets, head remaining seconds from the
## catalogue train seconds and live head progress, and the +N chip for
## anything beyond the fifth socket.
func _render_queue(building: Dictionary) -> void:
	var queue: Array = building.get("training_queue", [])
	var progress := float(building.get("queue_progress", 0.0))
	for index in QUEUE_SOCKET_COUNT:
		var icon := _socket_icons[index]
		var time := _socket_times[index]
		if index >= queue.size():
			icon.texture = null
			time.text = ""
			continue
		var kind := str(queue[index])
		icon.texture = _icon_for_kind("units", kind)
		if index == 0:
			var seconds := float(_unit_entries.get(kind, {}).get("seconds", 0))
			var remaining := maxi(ceili(seconds * (1.0 - progress)), 0)
			time.text = "%ds" % remaining
			icon.modulate = Color(1, 1, 1, 1)
		else:
			time.text = ""
			icon.modulate = Color(0.75, 0.82, 0.88, 0.95)
	_queue_overflow.visible = queue.size() > QUEUE_SOCKET_COUNT
	_queue_overflow.text = "+%d" % (queue.size() - QUEUE_SOCKET_COUNT)
	queue_sockets.visible = true

func _clear_selection_detail() -> void:
	health_bar.visible = false
	health_label.visible = false
	carry_bar.visible = false
	carry_label.visible = false
	queue_sockets.visible = false
	selection_panel.tooltip_text = ""
	portrait_icon.texture = null

## Contextual train tiles: visible exactly when the selected building's
## catalogue producer matches the unit row. No selection hides the whole row.
func _set_train_producer(producer_kind: String) -> void:
	if producer_kind == _train_producer_kind:
		return
	_train_producer_kind = producer_kind
	var any := false
	for button in _train_grid.get_children():
		var visible := producer_kind != "" and str(button.get_meta("producer", "")) == producer_kind
		button.visible = visible
		any = any or visible
	_train_grid.visible = any

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
