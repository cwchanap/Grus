extends Panel

## Underground drawer view attached directly to the existing UndergroundPanel.
## It is the single owner of the panel's visual arming state and of the
## runtime-created room/slot Buttons. The battlefield controller keeps Tab/Esc
## input, the Playing gate, bridge dispatch and command feedback; it also owns
## the static Excavate button's pressed wiring straight to `excavate_requested`.
## This script resolves the armed room kind and exposes only the two action
## intents below.

signal excavate_requested
signal room_requested(level: int, slot: int, kind: String)

## Cosmetic button labels only — the authoritative kind wire names ride the
## catalogue entries and bridge commands.
const DISPLAY_NAMES := {
	"Shelter": "Shelter",
	"Greenhouse": "Green",
	"CoalVein": "Coal",
	"GeothermalTap": "Tap",
	"Lab": "Lab",
	"DeepMine": "Mine",
}

@onready var _header: Label = $HeaderLabel
@onready var _excavate: Button = $ExcavateButton
@onready var _kinds: Container = $KindsBox
@onready var _armed_detail: RichTextLabel = $ArmedDetail
@onready var _levels: Container = $ScrollContainer/LevelsBox
## Static underground catalogue rows (name/min_level/cost/seconds), cached
## once from the catalogue's "underground" block. Every dynamic value (depth,
## dig, slots) arrives through the controller's per-tick refresh.
var _rooms: Array[Dictionary] = []
var _slots_per_level := 4
## Index into `_rooms` of the armed room kind (-1 = none armed); the panel's
## only selection state — slots derive eligibility from it every refresh.
var _armed_index := -1
var _kind_buttons: Array[Button] = []
## The controller-provided read-only snapshot of the last refresh, reused only
## to re-render slot eligibility immediately after an arm click; never a
## second bridge read or a gameplay copy.
var _snapshot: Dictionary = {}
var _phase := ""
var _icon_cache: Dictionary = {}

func _ready() -> void:
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var underground_catalogue: Dictionary = catalogue.get("underground", {})
	var ug_rooms: Dictionary = underground_catalogue.get("rooms", {})
	for kind in underground_catalogue.get("room_kinds", []):
		var ug_entry: Dictionary = ug_rooms.get(str(kind), {})
		if ug_entry.is_empty():
			continue
		ug_entry["name"] = str(kind)
		_rooms.append(ug_entry)
	_slots_per_level = maxi(int(underground_catalogue.get("slots", 4)), 1)
	# One ordinary toggle Button per room kind, built from the catalogue in
	# wire-name order: clicking arms the kind (the only selection state);
	# tooltips carry the full catalogue cost/time/effect.
	for index in _rooms.size():
		var ug_entry: Dictionary = _rooms[index]
		var kind_button := Button.new()
		kind_button.toggle_mode = true
		kind_button.add_theme_font_size_override("font_size", 14)
		kind_button.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
		kind_button.custom_minimum_size = Vector2(52, 78)
		kind_button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		var kind := str(ug_entry.get("name", ""))
		kind_button.set_meta("kind", kind)
		var short_name := display_name(kind)
		kind_button.text = short_name
		var room_icon := _icon_for_kind("underground", kind)
		if room_icon != null:
			kind_button.icon = room_icon
			kind_button.add_theme_constant_override("icon_max_width", 24)
			kind_button.icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
			kind_button.vertical_icon_alignment = VERTICAL_ALIGNMENT_TOP
			kind_button.add_theme_color_override("icon_disabled_color", Color(1, 1, 1, 0.85))
			kind_button.text = ""
			kind_button.accessibility_name = short_name
			var name_label := Label.new()
			name_label.name = "KindLabel"
			name_label.text = short_name
			name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
			name_label.add_theme_font_size_override("font_size", 11)
			name_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
			name_label.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
			name_label.offset_top = -31.0
			name_label.offset_bottom = -17.0
			kind_button.add_child(name_label)
		# Catalogue price and min-level badge: the mockup's readable kind
		# tiles without a second price table.
		var cost_label := RichTextLabel.new()
		cost_label.name = "KindCostLabel"
		cost_label.bbcode_enabled = true
		cost_label.scroll_active = false
		cost_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
		cost_label.add_theme_font_size_override("normal_font_size", 10)
		cost_label.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
		cost_label.offset_top = -16.0
		cost_label.offset_bottom = -1.0
		cost_label.text = "[center]%s[/center]" % _cost_short(ug_entry)
		kind_button.add_child(cost_label)
		var level_label := Label.new()
		level_label.name = "KindLevelLabel"
		level_label.text = "L%d" % int(ug_entry.get("min_level", 1))
		level_label.add_theme_font_size_override("font_size", 10)
		level_label.add_theme_color_override("font_color", Color(0.98, 0.78, 0.45))
		level_label.mouse_filter = Control.MOUSE_FILTER_IGNORE
		level_label.set_anchors_and_offsets_preset(Control.PRESET_TOP_RIGHT)
		level_label.offset_left = -22.0
		level_label.offset_right = -3.0
		level_label.offset_top = 2.0
		level_label.offset_bottom = 16.0
		kind_button.add_child(level_label)
		kind_button.tooltip_text = _kind_tooltip(ug_entry)
		kind_button.toggled.connect(_on_kind_toggled.bind(index))
		_kinds.add_child(kind_button)
		_kind_buttons.append(kind_button)
	var excavate_icon := _icon_for_kind("underground", "Excavate")
	if excavate_icon != null:
		_excavate.icon = excavate_icon
		_excavate.add_theme_constant_override("icon_max_width", 20)

## Dynamic refresh driven by the controller's existing visibility/tick gate.
## Level rows mirror the snapshot's level count — appended when a dig
## completes, rebuilt only on the depth-shrinking reseed — so a live press is
## never freed out from under a click. Live text (dig countdown, build
## progress, benefit lines, eligibility) refreshes per refresh from the
## read-only snapshot plus the static catalogue. No dig-price or
## room-effects math lives here — the snapshot carries the Rust numbers.
func refresh(snap: Dictionary, phase: String) -> void:
	_snapshot = snap
	_phase = phase
	_sync_rows(snap)
	_refresh_header(snap, phase)
	_refresh_kinds(snap)
	_refresh_armed_detail()
	_refresh_slots(snap, phase)

## Restart clears the transient armed room kind; the controller's open-panel
## refresh already clears the rows from the reseeded empty snapshot.
func reset() -> void:
	_armed_index = -1
	for kind_button in _kind_buttons:
		kind_button.set_pressed_no_signal(false)

func display_name(kind: String) -> String:
	return str(DISPLAY_NAMES.get(kind, kind))

## Keep one row per snapshot level. Growth only ever appends (a level row
## appears exactly when a dig completes), so existing slot buttons — and
## any press in flight on them — are never freed mid-click. Depth shrinks
## only through a match reseed, the one case that still rebuilds.
func _sync_rows(snap: Dictionary) -> void:
	var levels: Array = snap.get("levels", [])
	var rows := _levels.get_child_count()
	if levels.size() < rows:
		for child in _levels.get_children():
			_levels.remove_child(child)
			child.queue_free()
		rows = 0
	for index in range(rows, levels.size()):
		_add_row(int(levels[index].get("level", index + 1)))

func _add_row(level_number: int) -> void:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 2)
	var level_label := Label.new()
	level_label.text = "L%d" % level_number
	level_label.custom_minimum_size = Vector2(24, 48)
	level_label.add_theme_font_size_override("font_size", 14)
	row.add_child(level_label)
	for slot_index in _slots_per_level:
		var button := Button.new()
		button.add_theme_font_size_override("font_size", 13)
		button.add_theme_color_override("font_disabled_color", Color(0.8, 0.8, 0.8))
		button.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
		button.custom_minimum_size = Vector2(84, 48)
		button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		button.pressed.connect(_on_slot_pressed.bind(level_number, slot_index))
		row.add_child(button)
	_levels.add_child(row)

func _refresh_header(snap: Dictionary, phase: String) -> void:
	var playing := phase == "Playing"
	var depth := int(snap.get("depth", 0))
	var target := int(snap.get("target", 1))
	var cost_text := _cost_text(snap.get("next_cost", {}))
	var dig_line := ""
	if bool(snap.get("excavating", false)):
		dig_line = "Digging L%d · %d%% · %ds left" % [
			target, roundi(float(snap.get("progress", 0.0)) * 100.0),
			maxi(ceili(float(snap.get("remaining", 0.0))), 0)]
	else:
		dig_line = "Dig L%d · %s · %ds" % [target, cost_text, int(snap.get("next_seconds", 0))]
	var lock_line := ""
	if phase == "Paused":
		lock_line = "\nPaused — inspection only"
	elif not playing:
		lock_line = "\n%s — inspection only" % phase
	_header.text = "Underground — depth %d · Esc closes\n%s%s" % [
		depth, dig_line, lock_line]
	if bool(snap.get("excavating", false)):
		_excavate.disabled = true
		_excavate.text = "Digging L%d · %d%%" % [
			target, roundi(float(snap.get("progress", 0.0)) * 100.0)]
	else:
		_excavate.disabled = not playing
		# The header line always carries the full price/time; the button keeps
		# the terse form so deeper (longer) prices can never clip.
		_excavate.text = "Excavate L%d" % target

## A kind Button becomes selectable once the completed depth reaches its
## catalogue min_level; the pressed look marks the armed kind.
func _refresh_kinds(snap: Dictionary) -> void:
	var depth := int(snap.get("depth", 0))
	for index in _kind_buttons.size():
		var button := _kind_buttons[index]
		var min_level := int(_rooms[index].get("min_level", 1))
		button.disabled = depth < min_level
		var name_label := button.get_node_or_null("KindLabel") as Label
		if name_label != null:
			var color_name := "font_disabled_color" if button.disabled else "font_color"
			name_label.add_theme_color_override("font_color", button.get_theme_color(color_name))

## Per-refresh slot update against the live snapshot and the armed kind:
## empty slots act only with a kind armed, only on rows at or above its
## min_level, and only while Playing; shallower rows stay disabled and say
## why. Occupied/constructing slots are never actionable.
func _refresh_slots(snap: Dictionary, phase: String) -> void:
	var playing := phase == "Playing"
	var levels: Array = snap.get("levels", [])
	var armed := _armed_index >= 0 and _armed_index < _rooms.size()
	var armed_entry: Dictionary = _rooms[_armed_index] if armed else {}
	var armed_min := int(armed_entry.get("min_level", 1)) if armed else 0
	for row_index in mini(_levels.get_child_count(), levels.size()):
		var row: Node = _levels.get_child(row_index)
		var level_number := int(levels[row_index].get("level", 0))
		var slots: Array = levels[row_index].get("slots", [])
		for slot_index in mini(_slots_per_level, slots.size()):
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
					button.tooltip_text = _kind_tooltip(armed_entry)
					button.disabled = not playing
				else:
					button.text = "Empty"
					button.tooltip_text = "Arm a room kind above"
					button.disabled = true
				continue
			var display := display_name(kind)
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
## recomputes from the catalogue + last snapshot immediately.
func _on_kind_toggled(pressed: bool, index: int) -> void:
	if pressed:
		_armed_index = index
		for other in _kind_buttons.size():
			_kind_buttons[other].set_pressed_no_signal(other == index)
	else:
		_armed_index = -1
	refresh(_snapshot, _phase)

## An eligible empty-slot click resolves the armed kind and emits the intent
## with this 1-based level and 0-based slot; disabled Buttons never fire, and
## Rust revalidates everything authoritatively.
func _on_slot_pressed(level: int, slot: int) -> void:
	if _armed_index < 0 or _armed_index >= _rooms.size():
		return
	room_requested.emit(level, slot, str(_rooms[_armed_index].get("name", "")))

## Armed kind effect line: the same Rust-computed effect/tooltip text as the
## tile, rendered in the drawer's one static status row. Restart/reset leaves
## only the "arm a kind" hint.
func _refresh_armed_detail() -> void:
	var armed := _armed_index >= 0 and _armed_index < _rooms.size()
	if not armed:
		_armed_detail.text = "Arm a room kind — hover a tile for cost and effect"
		return
	var entry: Dictionary = _rooms[_armed_index]
	_armed_detail.text = "[color=#8fd18f]%s[/color] · %s · %ds" % [
		display_name(str(entry.get("name", ""))), str(entry.get("effect", "")),
		int(entry.get("seconds", 0))]

## Formats a cost dictionary carried by a snapshot or catalogue entry;
## GDScript renders, Rust computes.
func _cost_text(cost: Dictionary) -> String:
	var parts := PackedStringArray()
	for resource in ["food", "scrap", "coal", "crystals"]:
		var amount := int(cost.get(resource, 0))
		if amount > 0:
			parts.append("%d %s" % [amount, resource.capitalize()])
	return " + ".join(parts)

## Terse painted-resource cost for the kind tile: same catalogue numbers as
## the full tooltip, with 10 px resource icons so the price fits the tile.
func _cost_short(cost: Dictionary) -> String:
	var parts := PackedStringArray()
	for resource in ["food", "scrap", "coal", "crystals"]:
		var amount := int(cost.get(resource, 0))
		if amount <= 0:
			continue
		var icon := _icon_for_kind("resources", resource)
		if icon != null:
			parts.append("[img=10x10]%s[/img]%d" % [icon.resource_path, amount])
		else:
			parts.append("%d" % amount)
	return " ".join(parts)

## The full kind tooltip: catalogue name, cost, build time and the
## Rust-computed effect line — straight off the static catalogue row,
## never retyped here.
func _kind_tooltip(entry: Dictionary) -> String:
	return "%s — Requires L%d\nCost: %s · Build: %ds\n%s" % [
		str(entry.get("name", "")), int(entry.get("min_level", 1)), _cost_text(entry),
		int(entry.get("seconds", 0)), str(entry.get("effect", ""))]

func _icon_for_kind(group: String, kind: String) -> Texture2D:
	var path := "res://assets/ui/icons/%s/%s.png" % [group, kind.to_snake_case()]
	if not _icon_cache.has(path):
		_icon_cache[path] = ResourceLoader.load(path) as Texture2D if ResourceLoader.exists(path) else null
	return _icon_cache[path] as Texture2D
