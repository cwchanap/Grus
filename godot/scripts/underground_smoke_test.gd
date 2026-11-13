extends Node

## Real paid underground journey. No injected resources, timer skips or
## direct underground commands: purchases use the same Buttons as players.
## Two paid Barracks make the Lab's cold-independent contribution observable
## before the Tap warms the base; workers fund every purchase by gathering.
const SmokeHelpers := preload("res://scripts/smoke_helpers.gd")
const VIEWPORT_SIZE := Vector2i(1280, 720)
## The fixture's seeded treasury — fixture data, not catalogue tuning.
const SEED_TREASURY := {"food": 200, "scrap": 300, "coal": 150, "crystals": 0}
## The tail checks after the margin assert need a few more simulated
## seconds even on a slow runner: a rejected purchase, the real-button
## restart and their settles.
const MARGIN_FLOOR_S := 10.0

var _main: Node3D
var _panel: Panel
var _status: Label
var _excavate: Button
var _levels: VBoxContainer
var _catalogue: Dictionary
var _started_ms: int
var _spent_scrap := 0
var _spent_coal := 0
var _failed := false
var _track_deposits := false
var _last_stock: Dictionary
var _deposits := {"scrap": 0, "coal": 0}
## The live `next_cost` dict each paid dig charged — recorded so expected
## totals derive from snapshot prices, never a copied price table.
var _dig_costs: Array[Dictionary] = []

## Only count observed positive bank movements, never infer gathering from
## assumed prices. A deposit sharing a purchase frame may be hidden by the
## debit, so these are conservative lower bounds on real worker deposits.
func _process(_delta: float) -> void:
	if not _track_deposits:
		return
	var current := _stock()
	for currency in _deposits:
		_deposits[currency] += maxi(int(current[currency]) - int(_last_stock[currency]), 0)
	_last_stock = current

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	if _failed:
		return
	_failed = true
	push_error("underground smoke: %s; underground=%s economy=%s wave=%s feedback=%s" % [
		message, GrusBridge.underground_snapshot(), GrusBridge.economy_snapshot(),
		GrusBridge.wave_snapshot(), GrusBridge.command_feedback()])
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(1)

func _check(condition: bool, message: String) -> bool:
	if not condition:
		_fail(message)
	return condition

## Wall-clock bound (not scaled SceneTree timers); all prep polling also
## enforces the unchanged 300-s first-wave guard.
func _wait(condition: Callable, timeout_s: float, message: String) -> bool:
	var deadline := Time.get_ticks_msec() + int(timeout_s * 1000.0)
	while Time.get_ticks_msec() < deadline:
		var wave_error := SmokeHelpers.no_launched_wave_error(message)
		if not wave_error.is_empty():
			_fail(wave_error)
			return false
		if condition.call():
			return true
		await get_tree().process_frame
	_fail(message)
	return false

func _settle() -> void:
	await get_tree().process_frame
	await get_tree().process_frame

func _key(keycode: int) -> void:
	for pressed in [true, false]:
		var event := InputEventKey.new()
		event.keycode = keycode
		event.pressed = pressed
		get_tree().root.push_input(event)
	await _settle()

func _click(control: Control) -> void:
	SmokeHelpers.push_mouse_click(get_tree(), control.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await _settle()

func _click_view(view: Node3D, button: int) -> void:
	SmokeHelpers.push_mouse_click(get_tree(), _main.camera.unproject_position(view.global_position), button)
	await _settle()

func _kind(kind: String) -> Button:
	var order: Array = _catalogue.underground.room_kinds
	return _panel.get_node("KindsBox").get_child(order.find(kind)) as Button

func _slot(level: int, slot: int) -> Button:
	return _levels.get_child(level - 1).get_child(slot + 1) as Button

func _slot_state(level: int, slot: int) -> Dictionary:
	return GrusBridge.underground_snapshot().levels[level - 1].slots[slot]

func _icon_path(button: Button) -> String:
	return button.icon.resource_path if button.icon != null else ""

## The plate a Button paints for a state, read from the live control (slot
## plates are explicit overrides) or the active theme (kinds). Textures give
## their resource path; a flat recess gives "". No style table is copied.
func _plate_path(button: Button, state: String) -> String:
	var plate := button.get_theme_stylebox(state)
	return (plate as StyleBoxTexture).texture.resource_path if plate is StyleBoxTexture else ""

func _tile_path(state: String) -> String:
	var plate := _panel.get_theme_stylebox(state, "Tile")
	return (plate as StyleBoxTexture).texture.resource_path if plate is StyleBoxTexture else ""

## Excavation presentation: the themed bar and remaining-time label sit below
## Excavate and outside the clipped 152 px level viewport, and their values
## follow the same read-only snapshot as the header. `tolerance`/`slack`
## absorb the one-refresh lag possible while the 20x sim advances; the paused
## call uses exact values because the snapshot is frozen.
func _dig_view_error(where: String, tolerance := 0.001, slack := 0) -> String:
	var snap: Dictionary = GrusBridge.underground_snapshot()
	var bar := _panel.get_node_or_null("DigProgress") as ProgressBar
	var time_label := _panel.get_node_or_null("DigTime") as Label
	if bar == null or time_label == null:
		return "%s dig progress bar/label are missing" % where
	var excavating := bool(snap.get("excavating", false))
	if bar.visible != excavating or time_label.visible != excavating:
		return "%s dig progress visibility %s/%s does not match excavating=%s" % [
			where, bar.visible, time_label.visible, excavating]
	if not excavating:
		return ""
	if absf(bar.value - float(snap.get("progress", 0.0))) > tolerance:
		return "%s dig bar value %.3f does not match snapshot progress %.3f" % [
			where, bar.value, float(snap.get("progress", 0.0))]
	if not time_label.text.ends_with("s left") or not time_label.text.trim_suffix("s left").is_valid_int():
		return "%s dig label \"%s\" is not a remaining-time readout" % [where, time_label.text]
	var seconds := maxi(ceili(float(snap.get("remaining", 0.0))), 0)
	if absi(int(time_label.text.trim_suffix("s left")) - seconds) > slack:
		return "%s dig label \"%s\" does not match snapshot remaining %ds" % [
			where, time_label.text, seconds]
	if not (_panel.get_node("HeaderLabel") as Label).text.contains(time_label.text):
		return "%s header does not repeat the dig label %s" % [where, time_label.text]
	return ""

## The 152 px level viewport keeps showing three complete rows; the dig bar
## and its readout live below Excavate, never inside the clipped rows.
func _dig_layout_error(scroll: ScrollContainer) -> String:
	var bar := _panel.get_node_or_null("DigProgress") as ProgressBar
	var time_label := _panel.get_node_or_null("DigTime") as Label
	if bar == null or time_label == null:
		return "dig progress bar/label are missing"
	var view := scroll.get_global_rect()
	var bar_rect := bar.get_global_rect()
	var time_rect := time_label.get_global_rect()
	if view.size.y < 152.0:
		return "level viewport shrank to %.1f px; three rows need 152" % view.size.y
	if bar_rect.intersects(view) or time_rect.intersects(view):
		return "dig progress draws inside the level viewport: bar=%s time=%s view=%s" % [
			bar_rect, time_rect, view]
	if bar_rect.position.y < _excavate.get_global_rect().end.y:
		return "dig progress sits above the Excavate control"
	if time_label.get_theme_font_size("font_size") < 12:
		return "dig remaining-time readout is smaller than 12 px"
	if not _panel.get_global_rect().encloses(bar_rect) \
			or not _panel.get_global_rect().encloses(time_rect):
		return "dig progress leaves the drawer: bar=%s time=%s" % [bar_rect, time_rect]
	return ""

## The Button's own icon + text must fit the live content width (stylebox
## content margins included); otherwise Godot ellipsizes it under the painted
## frame. The old "Shelter 99%" single line failed this at 1280x720.
func _button_content_error(button: Button) -> String:
	var plate := button.get_theme_stylebox("disabled" if button.disabled else "normal")
	var font := button.get_theme_font("font")
	var font_size := button.get_theme_font_size("font_size")
	var text_width := font.get_string_size(button.text, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size).x
	var icon_width := 0.0
	if button.icon != null:
		icon_width = button.icon.get_width()
		var icon_max := float(button.get_theme_constant("icon_max_width"))
		if icon_max > 0.0:
			icon_width = minf(icon_width, icon_max)
	var needed := icon_width + (float(button.get_theme_constant("h_separation")) if icon_width > 0.0 else 0.0) + text_width
	var available := button.size.x - float(plate.get_content_margin(SIDE_LEFT)) \
			- float(plate.get_content_margin(SIDE_RIGHT))
	if needed > available + 0.5:
		return "icon+text need %.1f px but only %.1f px fit" % [needed, available]
	return ""

## The Button's own rendered text box: font metrics, the centring the Button
## applies to its icon+text group, and the live style's content margins. The
## whole Button rectangle says nothing about where its text actually lands.
func _button_text_rect(button: Button) -> Rect2:
	var plate := button.get_theme_stylebox("disabled" if button.disabled else "normal")
	var font := button.get_theme_font("font")
	var font_size := button.get_theme_font_size("font_size")
	var text_size := font.get_string_size(button.text, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size)
	var text_height := font.get_height(font_size)
	var content := Rect2(
		button.get_global_rect().position + Vector2(
			plate.get_content_margin(SIDE_LEFT), plate.get_content_margin(SIDE_TOP)),
		button.size - Vector2(
			plate.get_content_margin(SIDE_LEFT) + plate.get_content_margin(SIDE_RIGHT),
			plate.get_content_margin(SIDE_TOP) + plate.get_content_margin(SIDE_BOTTOM)))
	var icon_width := 0.0
	if button.icon != null:
		icon_width = button.icon.get_width()
		var icon_max := float(button.get_theme_constant("icon_max_width"))
		if icon_max > 0.0:
			icon_width = minf(icon_width, icon_max)
	var separation := float(button.get_theme_constant("h_separation")) if icon_width > 0.0 else 0.0
	var group_width := icon_width + separation + text_size.x
	var text_position := Vector2(
		content.position.x + (content.size.x - group_width) * 0.5 + icon_width + separation,
		content.position.y + (content.size.y - text_height) * 0.5)
	return Rect2(text_position, Vector2(text_size.x, text_height))

## A Label's painted text box from its font metrics and alignment.
func _label_text_rect(label: Label) -> Rect2:
	var font := label.get_theme_font("font")
	var font_size := label.get_theme_font_size("font_size")
	var text_size := font.get_string_size(label.text, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size)
	var text_height := font.get_height(font_size)
	var rect := label.get_global_rect()
	var x := rect.position.x
	if label.horizontal_alignment == HORIZONTAL_ALIGNMENT_CENTER:
		x += (rect.size.x - text_size.x) * 0.5
	elif label.horizontal_alignment == HORIZONTAL_ALIGNMENT_RIGHT:
		x += rect.size.x - text_size.x
	var y := rect.position.y
	if label.vertical_alignment == VERTICAL_ALIGNMENT_CENTER:
		y += (rect.size.y - text_height) * 0.5
	elif label.vertical_alignment == VERTICAL_ALIGNMENT_BOTTOM:
		y += rect.size.y - text_height
	return Rect2(Vector2(x, y), Vector2(text_size.x, text_height))

## The occupied-slot presentation contract: the room name alone keeps the
## centre clear, the construction percentage lives in its top-right badge and
## a completed room keeps its snapshot benefit visible at the bottom. This is
## what the reviewer found missing behind the texture-path checks.
func _slot_readout_error(button: Button, slot: Dictionary, where: String) -> String:
	var content_error := _button_content_error(button)
	if not content_error.is_empty():
		return "%s: %s" % [where, content_error]
	if button.get_theme_font_size("font_size") < 12:
		return "%s room name is smaller than 12 px" % where
	if button.text.contains("%"):
		return "%s still puts the percentage in the room label: %s" % [where, button.text]
	var percent := button.get_node_or_null("SlotPercent") as Label
	var benefit := button.get_node_or_null("SlotBenefit") as Label
	var benefit_backing := button.get_node_or_null("SlotBenefitBacking") as ColorRect
	if percent == null or benefit == null or benefit_backing == null:
		return "%s is missing its percent/benefit readouts" % where
	var tile_rect := button.get_global_rect()
	var name_text := _button_text_rect(button)
	if not tile_rect.encloses(name_text):
		return "%s room name text leaves its tile" % where
	if bool(slot.get("complete", false)):
		var effect := str(slot.get("benefit", ""))
		if percent.visible:
			return "%s completed room still shows a construction percentage" % where
		if not benefit.visible or benefit.text != effect:
			return "%s completed room lost its always-visible benefit \"%s\" (visible=%s text=\"%s\")" % [
				where, effect, benefit.visible, benefit.text]
		if not benefit_backing.visible \
				or not benefit_backing.get_global_rect().encloses(benefit.get_global_rect()):
			return "%s benefit readout has no clear band" % where
		if benefit.get_theme_font_size("font_size") < 11:
			return "%s benefit readout is smaller than 11 px" % where
		var benefit_font := benefit.get_theme_font("font")
		var benefit_width := benefit_font.get_string_size(benefit.text,
				HORIZONTAL_ALIGNMENT_LEFT, -1, benefit.get_theme_font_size("font_size")).x
		if benefit_width > benefit.size.x + 0.5:
			return "%s benefit text is wider than its clear band" % where
		if not tile_rect.encloses(benefit.get_global_rect()):
			return "%s benefit readout leaves its tile" % where
		if name_text.intersects(_label_text_rect(benefit)):
			return "%s room name and benefit text overlap" % where
	else:
		if benefit.visible or benefit_backing.visible:
			return "%s constructing room shows a benefit before completion" % where
		if not percent.visible:
			return "%s constructing room has no visible percent badge" % where
		var expected := "%d%%" % roundi(float(slot.get("progress", 0.0)) * 100.0)
		if percent.text != expected:
			return "%s percent badge is %s; expected %s" % [where, percent.text, expected]
		if percent.get_theme_font_size("font_size") < 11:
			return "%s percent badge is smaller than 11 px" % where
		var percent_font := percent.get_theme_font("font")
		var percent_width := percent_font.get_string_size(percent.text,
				HORIZONTAL_ALIGNMENT_LEFT, -1, percent.get_theme_font_size("font_size")).x
		if percent_width > percent.size.x + 0.5:
			return "%s percent text is wider than its badge" % where
		if not tile_rect.encloses(percent.get_global_rect()):
			return "%s percent badge leaves its tile" % where
		if name_text.intersects(_label_text_rect(percent)):
			return "%s room name and percent text overlap" % where
	return ""

## Kind tiles keep a clear name/price strip over the painted frame: the band
## must enclose both labels, the labels must fit it, and the level badge stays
## inside the tile. This catches legibility regressions the texture-path
## assertions cannot see.
func _kind_layout_error(button: Button, room_entry: Dictionary, where: String) -> String:
	var backing := button.get_node_or_null("KindLabelBacking") as ColorRect
	var name_label := button.get_node_or_null("KindLabel") as Label
	var cost_label := button.get_node_or_null("KindCostLabel") as RichTextLabel
	var level_label := button.get_node_or_null("KindLevelLabel") as Label
	if backing == null or name_label == null or cost_label == null or level_label == null:
		return "%s kind tile is missing its label band/name/price/level" % where
	if not backing.visible or backing.color.a < 0.45:
		return "%s kind label band is not visible enough" % where
	var tile_rect := button.get_global_rect()
	if not tile_rect.encloses(backing.get_global_rect()):
		return "%s kind label band leaves the tile" % where
	if not backing.get_global_rect().encloses(name_label.get_global_rect()) \
			or not backing.get_global_rect().encloses(cost_label.get_global_rect()):
		return "%s kind name/price leave their clear band" % where
	if name_label.get_minimum_size().x > backing.size.x - 4.0:
		return "%s kind name is wider than its clear band" % where
	if cost_label.get_content_width() > backing.size.x - 4.0:
		return "%s kind price is wider than its clear band" % where
	if not tile_rect.encloses(level_label.get_global_rect()):
		return "%s kind level badge leaves the tile" % where
	if name_label.get_theme_font_size("font_size") < 12:
		return "%s kind name is smaller than 12 px" % where
	if cost_label.get_theme_font_size("normal_font_size") < 11:
		return "%s kind price is smaller than 11 px" % where
	if name_label.get_global_rect().intersects(cost_label.get_global_rect()):
		return "%s kind name and price rectangles overlap" % where
	if name_label.size.y < name_label.get_theme_font("font").get_height(
			name_label.get_theme_font_size("font_size")):
		return "%s kind name has less height than its font needs" % where
	if cost_label.get_content_height() > cost_label.size.y:
		return "%s kind price content (%s px) is taller than its area" % [
			where, cost_label.get_content_height()]
	if not name_label.has_theme_constant_override("outline_size") \
			or name_label.get_theme_constant("outline_size") < 2:
		return "%s kind name has no readable outline" % where
	return ""

## The exact snapshot the drawer last rendered. Comparing the readouts to this
## avoids a one-tick 20x race between a fresh bridge read and the labels.
func _drawer_slot(level: int, slot: int) -> Dictionary:
	var snap: Dictionary = _panel.get("_snapshot")
	var levels: Array = snap.get("levels", [])
	if level < 1 or level > levels.size():
		return {}
	var slots: Array = levels[level - 1].get("slots", [])
	return slots[slot] if slot >= 0 and slot < slots.size() else {}

func _stock() -> Dictionary:
	var economy: Dictionary = GrusBridge.economy_snapshot()
	return {"food": int(economy.food), "scrap": int(economy.scrap),
		"coal": int(economy.coal), "crystals": int(economy.crystals)}

## The synchronous push must show optimistic queue feedback before Bevy
## processes it; the changed revision then proves authoritative feedback
## reached both bridge and the actual HUD label.
func _purchase(button: Button, queued: String, cost: Dictionary, accepted := true) -> bool:
	var before := _stock()
	var revision: int = GrusBridge.command_feedback_revision()
	SmokeHelpers.push_mouse_click(get_tree(), button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if not _check(_status.text == queued, "missing queued HUD feedback: %s != %s" % [_status.text, queued]):
		return false
	if not await _wait(func(): return GrusBridge.command_feedback_revision() > revision,
			2.0, "purchase never processed"):
		return false
	await _settle()
	var expected := "Command accepted" if accepted else "Command rejected (InsufficientResources)"
	if not _check(str(GrusBridge.command_feedback()) == expected and _status.text == expected,
			"authoritative purchase feedback missing from HUD"):
		return false
	var after := _stock()
	for currency in before:
		var debit := int(cost.get(currency, 0)) if accepted else 0
		# Active gatherers can deposit in the command tick. Without them,
		# require the exact debit (including unchanged unrelated currencies).
		var expected_amount := int(before[currency]) - debit
		var correct := int(after[currency]) >= expected_amount if _track_deposits else int(after[currency]) == expected_amount
		if not _check(correct, "wrong purchase debit: %s" % currency):
			return false
	if accepted:
		_spent_scrap += int(cost.get("scrap", 0))
		_spent_coal += int(cost.get("coal", 0))
	return true

func _dig(level: int) -> bool:
	if not await _wait(func(): return not _excavate.disabled, 2.0, "Excavate never enabled"):
		return false
	var snapshot: Dictionary = GrusBridge.underground_snapshot()
	var cost: Dictionary = snapshot.next_cost
	if not await _wait(func(): return int(_stock().scrap) >= int(cost.scrap) \
			and int(_stock().coal) >= int(cost.coal), 10.0, "workers failed to fund L%d" % level):
		return false
	if not await _purchase(_excavate, "Excavate queued", cost):
		return false
	_dig_costs.append(cost)
	return await _wait(func(): return int(GrusBridge.underground_snapshot().depth) == level,
		8.0, "L%d dig did not complete" % level)

func _fixture_ready() -> bool:
	if get_tree().get_nodes_in_group("unit_views").size() != 4 \
			or get_tree().get_nodes_in_group("resource_views").size() != 18:
		return false
	for unit in get_tree().get_nodes_in_group("unit_views"):
		if str(unit.get_meta("unit_kind", "")) != "Worker":
			return false
	return true

func _run() -> void:
	_started_ms = Time.get_ticks_msec()
	get_window().content_scale_size = VIEWPORT_SIZE
	get_window().size = VIEWPORT_SIZE
	_main = get_node("Main")
	_panel = _main.get_node("HUD/UndergroundPanel")
	_status = _main.get_node("HUD/CommandStatus")
	_excavate = _panel.get_node("ExcavateButton")
	_levels = _panel.get_node("ScrollContainer/LevelsBox")
	_catalogue = GrusBridge.catalogue_snapshot()
	for room_kind in _catalogue.underground.room_kinds:
		var kind_button := _kind(room_kind)
		var expected_icon := "res://assets/ui/icons/underground/%s.png" % str(room_kind).to_snake_case()
		if not _check(_icon_path(kind_button) == expected_icon,
				"%s room-kind Button icon is %s; expected %s" % [room_kind, _icon_path(kind_button), expected_icon]):
			return
		# HPA-698 drawer readability: every kind tile shows its catalogue
		# price and min-level badge next to the painted icon/name.
		var room_entry: Dictionary = _catalogue["underground"]["rooms"][room_kind]
		var kind_cost := kind_button.get_node_or_null("KindCostLabel") as RichTextLabel
		var kind_level := kind_button.get_node_or_null("KindLevelLabel") as Label
		if not _check(kind_cost != null and kind_level != null \
				and kind_cost.get_parsed_text().contains(str(int(room_entry["scrap"]))) \
				and kind_level.text == "L%d" % int(room_entry["min_level"]) \
				and kind_cost.get_theme_font_size("normal_font_size") >= 10,
				"%s room-kind tile is missing its catalogue cost/level badge" % room_kind):
			return
	if not _check(_icon_path(_excavate) == "res://assets/ui/icons/underground/excavate.png",
			"Excavate Button is missing underground/excavate.png"):
		return
	if not _check(GrusBridge.set_sim_speed(20.0), "20x speed rejected"):
		return
	if not await _wait(_fixture_ready, 5.0, "fixture views never settled"):
		return
	if not _check(_stock() == SEED_TREASURY, "wrong seed treasury"):
		return
	var base_rate := float(GrusBridge.research_snapshot().rate_per_second)
	var catalogue_base := float(_catalogue.research.base_per_second)
	if not _check(is_equal_approx(base_rate, catalogue_base),
			"wrong full-heat surface research baseline"):
		return
	await _click(_main.get_node("HUD/SessionPanel/StartButton"))
	if not _check(str(GrusBridge.session_snapshot().phase) == "Playing", "Start UI failed"):
		return

	# First click really pays for L1 and leaves GUI focus on Excavate.
	await _key(KEY_TAB)
	if not _check(_panel.visible, "Tab did not open panel"):
		return
	# HPA-698 drawer reflow: the open drawer must leave the research card's
	# Analyze button and milestone row visible and outside its rect, and a
	# real click must still reach the authoritative zero-Crystal reject.
	var analyze := _main.get_node("HUD/ResearchPanel/AnalyzeButton") as Button
	var markers := _main.get_node("HUD/ResearchPanel/ResearchMarkers") as Control
	var drawer_rect := _panel.get_global_rect()
	var command_card := _main.get_node("HUD/CommandPanel") as Panel
	var command_rect := command_card.get_global_rect()
	if not _check(not drawer_rect.intersects(command_rect),
			"open drawer covers the command card: drawer=%s card=%s" % [
				drawer_rect, command_rect]):
		return
	# The slot rows live inside the ScrollContainer; its own rect must end
	# above the card and its children must be clipped to it, otherwise rows
	# draw over and intercept the round order buttons below.
	var scroll_box := _panel.get_node("ScrollContainer") as ScrollContainer
	var scroll_rect := scroll_box.get_global_rect()
	if not _check(scroll_box.clip_contents and not scroll_rect.intersects(command_rect) \
			and scroll_rect.end.y <= command_rect.position.y,
			"drawer scroll viewport can draw over or intercept the command card: viewport=%s clip=%s card=%s" % [
				scroll_rect, scroll_box.clip_contents, command_rect]):
		return
	if not _check(analyze.visible and analyze.is_visible_in_tree() and not analyze.disabled
			and not drawer_rect.intersects(analyze.get_global_rect()),
			"open drawer covers or disables Analyze: drawer=%s analyze=%s" % [
				drawer_rect, analyze.get_global_rect()]):
		return
	if not _check(markers.visible and markers.is_visible_in_tree()
			and not drawer_rect.intersects(markers.get_global_rect()),
			"open drawer covers the research milestone row: drawer=%s markers=%s" % [
				drawer_rect, markers.get_global_rect()]):
		return
	if not _check(not markers.get_global_rect().intersects(analyze.get_global_rect()),
			"Analyze overlaps the research milestone row: analyze=%s markers=%s" % [
				analyze.get_global_rect(), markers.get_global_rect()]):
		return
	if not _check(markers.get_content_width() <= markers.size.x - 20.0,
			"research milestone row clips while the drawer is open: content=%.1f width=%.1f" % [
				markers.get_content_width(), markers.size.x]):
		return
	# The Analyze probe is presentation/feedback only; run it on the 1× clock
	# so its few frames cannot consume the journey's 20× prep budget.
	if not _check(GrusBridge.set_sim_speed(1.0), "could not slow the Analyze probe"):
		return
	await _settle()
	if not await _purchase(analyze, "Analyze queued", {}, false):
		return
	if not _check(GrusBridge.set_sim_speed(20.0), "could not restore the underground 20x clock"):
		return
	await _settle()
	# A real click on the command card while the drawer is open proves no
	# overflowing slot row intercepts it. Move's top edge is the strip the
	# old overflowing L3 row covered; Cancel then clears the probe.
	var order_worker: Node3D = get_tree().get_nodes_in_group("unit_views")[0]
	await _click_view(order_worker, MOUSE_BUTTON_LEFT)
	if not _check(_main.selected_ids == [int(order_worker.get_meta("unit_id"))],
			"real worker selection behind the open drawer failed"):
		return
	var move_button := _main.get_node("HUD/CommandPanel/OrderGrid/MoveButton") as Button
	var cancel_button := _main.get_node("HUD/CommandPanel/OrderGrid/CancelButton") as Button
	var order_revision: int = GrusBridge.command_feedback_revision()
	SmokeHelpers.push_mouse_click(get_tree(),
		move_button.get_global_rect().position + Vector2(move_button.size.x * 0.5, 6.0),
		MOUSE_BUTTON_LEFT)
	await _settle()
	if not _check(str(_main.get("_armed_order")) == "move" \
			and _status.text == "Move: right-click a ground target",
			"open drawer intercepted the Move order control: armed=%s status=%s" % [
				str(_main.get("_armed_order")), _status.text]):
		return
	await _click(cancel_button)
	if not _check(str(_main.get("_armed_order")) == "" and _status.text == "Order canceled" \
			and int(GrusBridge.command_feedback_revision()) == order_revision,
			"drawer-open order probe did not clear without a command"):
		return
	# Widget-only zero boundary: a real dig is never observed at exactly 0 on a
	# refreshed frame. Feed the drawer a copy of the live idle snapshot with
	# excavating=true and progress=0.0 — presentation only, no command, spend
	# or sim state — then restore from the controller's next refresh before
	# the real paid dig starts.
	var zero_snap: Dictionary = GrusBridge.underground_snapshot().duplicate(true)
	zero_snap["excavating"] = true
	zero_snap["progress"] = 0.0
	zero_snap["remaining"] = float(zero_snap.get("next_seconds", 0.0))
	_panel.call("refresh", zero_snap, "Playing")
	var zero_bar := _panel.get_node("DigProgress") as ProgressBar
	var zero_label := _panel.get_node("DigTime") as Label
	var zero_header := _panel.get_node("HeaderLabel") as Label
	if not _check(zero_bar.visible and is_zero_approx(zero_bar.value) \
			and zero_label.visible and zero_label.text == "%ds left" % ceili(float(zero_snap["remaining"])) \
			and zero_header.text.contains("· 0% ·") and _excavate.text == "Digging L1 · 0%" \
			and _excavate.disabled,
			"zero-progress widget boundary did not render exactly 0%%: bar=%s label=%s header=%s excavate=%s" % [
				zero_bar.value, zero_label.text, zero_header.text, _excavate.text]):
		return
	if not await _wait(func(): return not zero_bar.visible, 2.0,
			"zero-progress widget probe did not restore the live idle drawer"):
		return

	var l1_before := _stock()
	var l1_cost: Dictionary = GrusBridge.underground_snapshot().next_cost
	if not await _purchase(_excavate, "Excavate queued", l1_cost):
		return
	_dig_costs.append(l1_cost)
	if not _check(_stock().scrap == int(l1_before.scrap) - int(l1_cost.scrap) \
			and _stock().coal == int(l1_before.coal) - int(l1_cost.coal),
			"L1 must charge its snapshot price exactly once"):
		return
	# Active dig: the themed bar is visible outside the level viewport and its
	# fill/readout follow the snapshot. At 20x one refresh can lag a tick, so
	# the live calls allow a small tolerance; the paused call below is exact.
	var early_dig: Dictionary = GrusBridge.underground_snapshot()
	if not _check(bool(early_dig.excavating) and float(early_dig.progress) >= 0.0 \
			and float(early_dig.progress) < 1.0,
			"L1 dig is not active with partial progress: %s" % early_dig):
		return
	var dig_error := _dig_view_error("L1 early dig", 0.1, 2)
	if not _check(dig_error.is_empty(), dig_error):
		return
	await _settle()
	var early_bar := (_panel.get_node("DigProgress") as ProgressBar).value
	if not await _wait(func(): return float(GrusBridge.underground_snapshot().progress) \
			> float(early_dig.progress) + 0.05, 3.0, "L1 dig progress did not advance"):
		return
	await _settle()
	var partial_dig: Dictionary = GrusBridge.underground_snapshot()
	if not _check(float(partial_dig.remaining) < float(early_dig.remaining) \
			and (_panel.get_node("DigProgress") as ProgressBar).value > early_bar,
			"L1 dig bar did not advance with its snapshot progress/remaining"):
		return
	dig_error = _dig_view_error("L1 partial dig", 0.1, 2)
	if not _check(dig_error.is_empty(), dig_error):
		return
	# Opt-in real-render evidence of the active dig bar; presentation only.
	var dig_capture := await SmokeHelpers.capture_frame(get_tree(), "underground-dig-progress")
	if not _check(dig_capture.is_empty(), dig_capture):
		return
	if not _check(get_viewport().gui_get_focus_owner() == _excavate, "real Excavate click did not focus button"):
		return
	await _key(KEY_TAB)
	if not _check(not _panel.visible, "Tab after focused click did not close panel"):
		return
	await _key(KEY_TAB)
	await _key(KEY_ESCAPE)
	if not _check(not _panel.visible and str(GrusBridge.session_snapshot().phase) == "Playing",
			"Esc while panel open leaked through to pause"):
		return

	# Closed-panel Esc pauses. Inspection remains available but cannot mutate.
	await _key(KEY_ESCAPE)
	await _key(KEY_TAB)
	var paused: Dictionary = GrusBridge.underground_snapshot()
	if not _check(str(GrusBridge.session_snapshot().phase) == "Paused" \
			and paused.excavating and float(paused.progress) > 0.0 and int(paused.depth) == 0,
			"L1 was not paid and in progress when paused"):
		return
	# Frozen snapshot: the bar must match progress/remaining exactly, not a
	# timer copy of its own.
	var paused_dig_error := _dig_view_error("paused L1 dig")
	if not _check(paused_dig_error.is_empty(), paused_dig_error):
		return
	var paused_stock := _stock()
	var paused_revision: int = GrusBridge.command_feedback_revision()
	for _frame in 12:
		await get_tree().physics_frame
	if not _check(GrusBridge.underground_snapshot() == paused and _stock() == paused_stock \
			and _excavate.disabled and _panel.get_node("HeaderLabel").text.contains("inspection only") \
			and absf((_panel.get_node("DigProgress") as ProgressBar).value - float(paused.progress)) < 0.001,
			"pause failed to freeze dig or inspection controls"):
		return
	await _click(_excavate)
	if not _check(GrusBridge.command_feedback_revision() == paused_revision and _stock() == paused_stock,
			"paused disabled click queued or spent"):
		return
	await _key(KEY_ESCAPE) # closes only
	await _key(KEY_ESCAPE) # resumes
	if not _check(str(GrusBridge.session_snapshot().phase) == "Playing", "Esc did not resume"):
		return

	_track_deposits = true
	_last_stock = _stock()
	# Derive sources from live explored views, never fixture cells or ids.
	var scraps: Array[Node3D] = []
	var coals: Array[Node3D] = []
	for source in get_tree().get_nodes_in_group("resource_views"):
		if not source.is_visible_in_tree():
			continue
		if str(source.get_meta("resource_kind", "")) == "Scrap":
			scraps.append(source)
		elif str(source.get_meta("resource_kind", "")) == "Coal":
			coals.append(source)
	var workers: Array[Node3D] = []
	workers.assign(get_tree().get_nodes_in_group("unit_views"))
	workers.sort_custom(func(a, b): return int(a.get_meta("unit_id")) < int(b.get_meta("unit_id")))
	scraps.sort_custom(func(a, b): return int(a.get_meta("resource_id")) < int(b.get_meta("resource_id")))
	if not _check(scraps.size() == 3 and coals.size() == 1, "expected three explored Scrap and one Coal source"):
		return
	for index in workers.size():
		await _click_view(workers[index], MOUSE_BUTTON_LEFT)
		if not _check(_main.selected_ids == [int(workers[index].get_meta("unit_id"))], "worker UI selection failed"):
			return
		var source: Node3D = coals[0] if index == 0 else scraps[index - 1]
		var gather_revision: int = GrusBridge.command_feedback_revision()
		await _click_view(source, MOUSE_BUTTON_RIGHT)
		if not await _wait(func(): return GrusBridge.command_feedback_revision() > gather_revision,
				2.0, "real Gather never processed"):
			return
		var task: Dictionary = GrusBridge.unit_task_snapshot(int(workers[index].get_meta("unit_id")))
		if not _check(str(GrusBridge.command_feedback()) == "Command accepted for 1 unit(s)" \
				and int(task.get("source", -1)) == int(source.get_meta("resource_id")), "worker did not acquire Gather source: %s" % task):
			return

	await _key(KEY_TAB)
	if not await _wait(func(): return int(GrusBridge.underground_snapshot().depth) == 1, 5.0, "L1 never resumed"):
		return
	await _settle()
	var l1_idle_error := _dig_view_error("completed L1 dig")
	if not _check(l1_idle_error.is_empty() \
			and not (_panel.get_node("DigProgress") as ProgressBar).visible \
			and _excavate.text == "Excavate L2",
			"completed L1 dig did not clear the bar or restore Excavate L2: %s" % l1_idle_error):
		return
	# Four painted Tile appearances (mockup-4 token sheet): an available kind
	# paints the normal steel tile, a not-yet-unlocked kind paints the locked
	# tile. Slots derived from the same read-only theme plates are asserted at
	# L3 below.
	var tile_normal_path := _tile_path("normal")
	var tile_selected_path := _tile_path("pressed")
	var tile_locked_path := _tile_path("disabled")
	if not _check(not tile_normal_path.is_empty() and not tile_selected_path.is_empty() \
			and not tile_locked_path.is_empty() and tile_normal_path != tile_selected_path \
			and tile_normal_path != tile_locked_path,
			"theme Tile normal/selected/locked plates are missing or not distinct: %s %s %s" % [
				tile_normal_path, tile_selected_path, tile_locked_path]):
		return
	if not _check(not _kind("Shelter").disabled \
			and _plate_path(_kind("Shelter"), "normal") == tile_normal_path,
			"available Shelter kind is not the painted normal tile: %s" % _plate_path(_kind("Shelter"), "normal")):
		return
	if not _check(_kind("Lab").disabled \
			and _plate_path(_kind("Lab"), "disabled") == tile_locked_path,
			"locked Lab kind is not the painted locked tile: %s" % _plate_path(_kind("Lab"), "disabled")):
		return
	var dig_layout := _dig_layout_error(scroll_box)
	if not _check(dig_layout.is_empty(), dig_layout):
		return
	# Kind tiles must keep a readable name/price band on top of the painted
	# frame; the texture-path checks above cannot see this.
	for room_kind in _catalogue.underground.room_kinds:
		var kind_layout := _kind_layout_error(_kind(room_kind),
				_catalogue["underground"]["rooms"][room_kind], "L1 %s" % room_kind)
		if not _check(kind_layout.is_empty(), kind_layout):
			return
	if not _check(_slot(1, 0).disabled, "unarmed empty slot must be disabled"):
		return
	if not await _dig(2):
		return
	# Once the final dig's Coal is banked, move that worker to Scrap:
	# surplus Coal cannot fund the two cold-proof Barracks. The deposit
	# bound is the L3 price minus what the seeded treasury still covers
	# after everything spent so far — both read live, never re-typed.
	var l3_cost: Dictionary = GrusBridge.underground_snapshot().next_cost
	var coal_deposits_needed: int = int(l3_cost.coal) - (int(SEED_TREASURY.coal) - _spent_coal)
	if not await _wait(func(): return int(_stock().coal) >= int(l3_cost.coal) \
			and int(_deposits.coal) >= coal_deposits_needed, 5.0,
			"Coal gatherer did not fund L3"):
		return
	await _key(KEY_TAB)
	await _click_view(workers[0], MOUSE_BUTTON_LEFT)
	var retask_revision: int = GrusBridge.command_feedback_revision()
	await _click_view(scraps[0], MOUSE_BUTTON_RIGHT)
	if not await _wait(func(): return GrusBridge.command_feedback_revision() > retask_revision,
			2.0, "Coal worker did not retask to Scrap"):
		return
	await _key(KEY_TAB)
	if not await _dig(3):
		return
	await _settle()
	var snapshot: Dictionary = GrusBridge.underground_snapshot()
	if not _check(snapshot.levels.size() == 3, "wrong snapshot depth rows"):
		return
	for index in 3:
		var row: Dictionary = snapshot.levels[index]
		if not _check(int(row.level) == index + 1 and row.slots.size() == 4, "bad 1-based level/four-slot snapshot"):
			return
		for slot in 4:
			if not _check(str(row.slots[slot].kind) == "" and not bool(row.slots[slot].complete), "bad 0-based empty slot"):
				return
			if not _check(_icon_path(_slot(index + 1, slot)).is_empty(),
					"empty L%d slot %d shows an icon" % [index + 1, slot]):
				return
	if not _check(not bool(snapshot.excavating) \
			and not (_panel.get_node("DigProgress") as ProgressBar).visible \
			and _excavate.text == "Excavate L4",
			"L3 completion did not leave the drawer idle on Excavate L4"):
		return
	# HPA-698 mockup alignment: at 1280 × 720 the drawer must show at least
	# three complete level rows at once. Complete means the row and every
	# slot Button inside it are fully enclosed by the clipped scroll
	# viewport, not merely overlapping it.
	var row_view := scroll_box.get_global_rect()
	var complete_rows := 0
	for row in _levels.get_children():
		var enclosed: bool = row_view.encloses(row.get_global_rect())
		for child in row.get_children():
			enclosed = enclosed and row_view.encloses(child.get_global_rect())
		if enclosed:
			complete_rows += 1
	if not _check(complete_rows >= 3,
			"drawer shows %d complete level rows; expected at least three at 1280x720 (viewport=%s rows=%d)" % [
				complete_rows, row_view, _levels.get_child_count()]):
		return

	# Fund Lab+Tap and two ordinary Barracks. Retire gatherers before exact
	# debit/no-spend checks so no real deposit can disguise a broken charge.
	# The deposit bound is the whole journey's Scrap price minus the seeded
	# treasury — every term live (catalogue prices plus the recorded dig
	# debits), never a re-typed total.
	var room_costs: Dictionary = _catalogue.underground.rooms
	var barracks_cost: Dictionary = _catalogue.buildings.Barracks
	var lab_rate := float(room_costs.Lab.research_per_second)
	var tap_heat := int(room_costs.GeothermalTap.heat_supply_base)
	var remaining_scrap := int(room_costs.Lab.scrap) + int(room_costs.GeothermalTap.scrap) + 2 * int(barracks_cost.scrap)
	var scrap_deposits_needed: int = _spent_scrap + remaining_scrap - int(SEED_TREASURY.scrap)
	if not await _wait(func(): return int(_stock().scrap) >= remaining_scrap \
			and int(_deposits.scrap) >= scrap_deposits_needed, 10.0,
			"Gather did not fund rooms and cold proof"):
		return
	await _key(KEY_TAB)
	for worker in workers:
		await _click_view(worker, MOUSE_BUTTON_LEFT)
		await _key(KEY_S)
	await _settle()
	_track_deposits = false
	var gathered_scrap := int(_deposits.scrap)
	var gathered_coal := int(_deposits.coal)
	if not _check(gathered_scrap >= scrap_deposits_needed and gathered_coal >= coal_deposits_needed,
			"real deposits did not cover total shortfall"):
		return

	# Parallel real surface construction creates a 15/16 cold base. No
	# underground heat demand and no fabricated heat state are involved.
	var rc: Node3D = get_tree().get_nodes_in_group("building_views")[0]
	for index in 2:
		var worker: Node3D = workers[index]
		var id := int(worker.get_meta("unit_id"))
		var anchor := SmokeHelpers.ring_anchor(id, "Barracks", rc)
		if not _check(anchor.x >= 0, "no live placement candidate for cold proof"):
			return
		await _click_view(worker, MOUSE_BUTTON_LEFT)
		var before_barracks := _stock()
		await _click(_main.get_node("HUD/CommandPanel/BuildGrid/BarracksButton"))
		SmokeHelpers.push_mouse_click(get_tree(), _main.camera.unproject_position(
			Vector3(anchor.x + 0.25, 0.0, anchor.y + 0.25)), MOUSE_BUTTON_LEFT)
		if not await _wait(func(): return get_tree().get_nodes_in_group("building_views").size() == index + 2,
				2.0, "real Barracks placement failed"):
			return
		if not _check(int(_stock().scrap) == int(before_barracks.scrap) - int(barracks_cost.scrap),
				"Barracks was not paid exactly once"):
			return
		_spent_scrap += int(barracks_cost.scrap)

	await _key(KEY_TAB)
	await _click(_kind("Lab"))
	if not _check(_kind("Lab").button_pressed, "Lab did not arm"):
		return
	var armed_detail := _panel.get_node_or_null("ArmedDetail") as RichTextLabel
	if not _check(armed_detail != null and armed_detail.visible \
			and armed_detail.get_parsed_text().contains("Lab") \
			and armed_detail.get_parsed_text().contains(str(_catalogue["underground"]["rooms"]["Lab"]["effect"])),
			"armed Lab does not show its catalogue effect line"):
		return
	if not _check(_plate_path(_kind("Lab"), "pressed") == tile_selected_path,
			"armed Lab kind is not the painted selected tile: %s" % _plate_path(_kind("Lab"), "pressed")):
		return
	var disabled_before := _stock()
	var disabled_revision: int = GrusBridge.command_feedback_revision()
	var selected_before: Array = _main.selected_ids.duplicate()
	# All eight disabled clicks dispatch synchronously back to back — a
	# disabled Button ignores them, so no settle is needed between pushes.
	# Fewer render-frame waits keeps the sim-time cost of this sweep flat
	# even on a slow runner, where each awaited frame burns 20x sim time.
	for level in [1, 2]:
		for slot in 4:
			var button := _slot(level, slot)
			if not _check(button.disabled and button.text.contains("L3") \
					and button.tooltip_text.contains("requires L3") \
					and button.accessibility_name.contains("L3") \
					and button.get_theme_font_size("font_size") >= 12 \
					and _icon_path(button) == "res://assets/ui/icons/status/locked.png" \
					and button.get_theme_constant("icon_max_width") >= 22 \
					and _plate_path(button, "disabled") == tile_locked_path,
					"Lab armed shallow target is not the locked tile/lock icon/explanation: icon=%s size=%d font=%d plate=%s name=%s" % [
						_icon_path(button), button.get_theme_constant("icon_max_width"),
						button.get_theme_font_size("font_size"),
						_plate_path(button, "disabled"), button.accessibility_name]):
				return
			SmokeHelpers.push_mouse_click(get_tree(), button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await _settle()
	if not _check(_stock() == disabled_before and GrusBridge.command_feedback_revision() == disabled_revision \
			and _main.selected_ids == selected_before, "disabled slot click queued, spent or leaked to battlefield"):
		return
	for level in [1, 2]:
		for slot in 4:
			if not _check(str(_slot_state(level, slot).kind) == "", "disabled shallow target built a room"):
				return
	# Armed eligible empty at L3: the selected plate while the disabled
	# occupied/locked branches keep their own plate. Then discharge the kind
	# to prove an unarmed empty is the flat recess, never a locked tile.
	var armed_empty := _slot(3, 2)
	if not _check(not armed_empty.disabled and armed_empty.text == "Empty" \
			and armed_empty.icon == null \
			and _plate_path(armed_empty, "normal") == tile_selected_path,
			"armed eligible L3 slot is not the selected tile: %s" % _plate_path(armed_empty, "normal")):
		return
	await _click(_kind("Lab"))
	if not _check(not _kind("Lab").button_pressed, "Lab did not discharge for the unarmed-slot check"):
		return
	var unarmed_empty := _slot(3, 2)
	var unarmed_plate := unarmed_empty.get_theme_stylebox("disabled")
	if not _check(unarmed_empty.disabled and unarmed_empty.text == "Empty" \
			and unarmed_empty.icon == null and unarmed_plate is StyleBoxFlat,
			"unarmed empty L3 slot is not the flat recessed tile"):
		return
	await _click(_kind("Lab"))
	if not _check(_kind("Lab").button_pressed, "Lab did not re-arm after the unarmed-slot check"):
		return
	if not _check(not _slot(3, 0).disabled, "L3 did not enable Lab target"):
		return
	var scroll := scroll_box
	for _attempt in 10:
		if scroll.get_global_rect().encloses(_slot(3, 0).get_global_rect()):
			break
		SmokeHelpers.push_mouse_click(get_tree(), scroll.get_global_rect().get_center(), MOUSE_BUTTON_WHEEL_DOWN)
		await _settle()
	if not _check(scroll.get_global_rect().encloses(_slot(3, 0).get_global_rect()),
			"L3 slot did not scroll fully into the visible underground area: scroll=%s max=%s page=%s viewport=%s target=%s" % [
				scroll.get_v_scroll_bar().value, scroll.get_v_scroll_bar().max_value,
				scroll.get_v_scroll_bar().page, scroll.get_global_rect(),
				_slot(3, 0).get_global_rect()]):
		return
	if not _check(_slot(3, 0).get_global_rect().intersection(scroll.get_global_rect()).end.y \
			<= command_rect.position.y,
			"visible L3 slot row extends into the command card: slot=%s viewport=%s card=%s" % [
				_slot(3, 0).get_global_rect(), scroll.get_global_rect(), command_rect]):
		return
	# Three rows now fit without overflow; the real wheel-input reachability
	# of deeper rows is exercised by the four-row presentation probe below.
	var level_bar := scroll.get_v_scroll_bar()
	if level_bar.max_value > level_bar.page and not _check(level_bar.value > 0.0,
			"level list overflows but real wheel input never scrolled it: value=%s max=%s page=%s" % [
				level_bar.value, level_bar.max_value, level_bar.page]):
		return
	var before_lab := _stock()
	if not await _purchase(_slot(3, 0), "Lab queued (L3 slot 1)", room_costs.Lab):
		return
	if not _check(_stock().scrap == before_lab.scrap - int(room_costs.Lab.scrap) \
			and _stock().coal == before_lab.coal \
			and str(_slot_state(3, 0).kind) == "Lab" and not bool(_slot_state(3, 0).complete),
			"Lab did not charge once at L3/slot0 and start construction"):
		return
	if not _check(_icon_path(_slot(3, 0)) == "res://assets/ui/icons/underground/lab.png" \
			and _plate_path(_slot(3, 0), "disabled") == tile_normal_path,
			"building Lab slot is missing its normal painted tile or room icon"):
		return
	# The construction percentage is a compact top-right badge; the room name
	# keeps the centre and must not ellipsize under the painted frame.
	var building_readout := _slot_readout_error(_slot(3, 0), _drawer_slot(3, 0), "building Lab")
	if not _check(building_readout.is_empty(), building_readout):
		return
	# Real construction evidence. At 20x the 30 s Lab build has ~25 s left
	# when the capture starts; a post-capture check keeps a stalled frame from
	# silently recording a completed room.
	var construction_capture := await SmokeHelpers.capture_frame(get_tree(), "underground-construction")
	if not _check(construction_capture.is_empty(), construction_capture):
		return
	if not _check(not bool(_slot_state(3, 0).complete),
			"Lab completed before its construction frame"):
		return
	await _click(_kind("Shelter"))
	if not _check(not _slot(1, 0).disabled and _slot(3, 0).disabled \
			and _plate_path(_slot(3, 0), "disabled") == tile_normal_path,
			"Shelter failed to re-enable L1 or the disabled occupied Lab lost its normal tile"):
		return
	# Panel blank-space shielding: clicks must neither deselect nor issue
	# contextual orders even with a selected worker behind the panel.
	SmokeHelpers.push_mouse_click(get_tree(), _panel.global_position + Vector2(3, 3), MOUSE_BUTTON_LEFT)
	SmokeHelpers.push_mouse_click(get_tree(), _panel.global_position + Vector2(3, 3), MOUSE_BUTTON_RIGHT)
	await _settle()
	if not _check(_main.selected_ids == selected_before and GrusBridge.command_feedback_revision() == disabled_revision + 1,
			"panel mouse shielding failed"):
		return

	if not await _wait(func(): return bool(_slot_state(3, 0).complete) \
			and int(GrusBridge.economy_snapshot().heat_demand) == 16, 8.0, "Lab/Barracks never completed"):
		return
	await _settle()
	if not _check(_icon_path(_slot(3, 0)) == "res://assets/ui/icons/underground/lab.png" \
			and _plate_path(_slot(3, 0), "disabled") == tile_normal_path,
			"completed Lab slot is not the normal painted tile with its room icon"):
		return
	var lab_benefit := _slot_readout_error(_slot(3, 0), _drawer_slot(3, 0), "completed Lab")
	if not _check(lab_benefit.is_empty(), lab_benefit):
		return
	var cold: Dictionary = GrusBridge.economy_snapshot()
	var cold_rate := float(GrusBridge.research_snapshot().rate_per_second)
	if not _check(int(cold.heat_supply) == 15 and float(cold.surface_efficiency) < 1.0 \
			and absf(cold_rate - (base_rate * float(cold.surface_efficiency) + lab_rate)) < 0.0001,
			"Lab research was cold-scaled instead of additive"):
		return

	await _click(_kind("GeothermalTap"))
	if not _check(str(_status.text) == "GeothermalTap armed — click an eligible slot",
			"real room-kind arm did not surface the controller hint: \"%s\"" % _status.text):
		return
	var before_tap := _stock()
	if not await _purchase(_slot(3, 1), "Tap queued (L3 slot 2)", room_costs.GeothermalTap):
		return
	if not _check(_stock().scrap == before_tap.scrap - int(room_costs.GeothermalTap.scrap) \
			and _stock().coal == before_tap.coal \
			and str(_slot_state(3, 1).kind) == "GeothermalTap" and not bool(_slot_state(3, 1).complete),
			"Tap did not charge exactly once at L3/slot1"):
		return
	if not _check(_icon_path(_slot(3, 1)) == "res://assets/ui/icons/underground/geothermal_tap.png" \
			and _plate_path(_slot(3, 1), "disabled") == tile_normal_path,
			"building Geothermal Tap slot is not the normal painted tile with its room icon"):
		return
	var tap_building := _slot_readout_error(_slot(3, 1), _drawer_slot(3, 1), "building Tap")
	if not _check(tap_building.is_empty(), tap_building):
		return
	if not await _wait(func(): return bool(_slot_state(3, 1).complete), 6.0, "Tap did not complete"):
		return
	await _settle()
	if not _check(_icon_path(_slot(3, 1)) == "res://assets/ui/icons/underground/geothermal_tap.png" \
			and _plate_path(_slot(3, 1), "disabled") == tile_normal_path,
			"completed Geothermal Tap slot is not the normal painted tile with its room icon"):
		return
	var tap_benefit := _slot_readout_error(_slot(3, 1), _drawer_slot(3, 1), "completed Tap")
	if not _check(tap_benefit.is_empty(), tap_benefit):
		return
	var warm: Dictionary = GrusBridge.economy_snapshot()
	if not _check(int(warm.heat_supply) == int(cold.heat_supply) + tap_heat and int(warm.heat_demand) == 16 \
			and is_equal_approx(float(warm.surface_efficiency), 1.0) \
			and is_equal_approx(float(GrusBridge.research_snapshot().rate_per_second), base_rate + lab_rate),
			"L3 Tap did not add its catalogue supply or warm Lab rate lost its additive share"):
		return
	# Opt-in final-head evidence: the paid L3 drawer with Lab/Tap complete and
	# the Tap kind armed — locked, occupied and armed-eligible states together.
	var drawer_capture := await SmokeHelpers.capture_frame(get_tree(), "underground-l3")
	if not _check(drawer_capture.is_empty(), drawer_capture):
		return
	# Presentation-only deeper-row probe. The paid journey stops at L3 — the
	# unchanged first-wave guard leaves no time to fund L4 — so the drawer is
	# handed a temporary four-level read: the real populated L3 row copied to
	# L4. No command, spend, sim tick or gameplay state changes; this only
	# exercises the existing row builder and scroll container. With four rows
	# the fourth row's geometry falls across the command card, the exact
	# overlap the clipped viewport must prevent.
	# Disarm the live Tap kind first so the synthetic row could never
	# dispatch a room command even if a future regression leaves it
	# unclipped; the journey's armed state is restored below.
	await _click(_kind("GeothermalTap"))
	if not _check(not _kind("GeothermalTap").button_pressed,
			"deeper-row probe could not disarm the live Tap kind"):
		return
	# With no kind armed, the completed rooms must keep their always-visible
	# snapshot benefit readout — the reviewer's unarmed-frame regression.
	for slot_index in 2:
		var unarmed_benefit := _slot_readout_error(_slot(3, slot_index),
				_drawer_slot(3, slot_index), "unarmed L3 slot %d" % slot_index)
		if not _check(unarmed_benefit.is_empty(), unarmed_benefit):
			return
	# Opt-in evidence of the fourth appearance: with no kind armed, every
	# empty slot is the flat recess while the completed rooms stay painted.
	var unarmed_capture := await SmokeHelpers.capture_frame(get_tree(), "underground-unarmed")
	if not _check(unarmed_capture.is_empty(), unarmed_capture):
		return
	var phase := str(GrusBridge.session_snapshot().phase)
	var probe: Dictionary = GrusBridge.underground_snapshot().duplicate(true)
	var probe_row: Dictionary = probe.levels[2].duplicate(true)
	probe_row.level = 4
	# First refresh: four completed L4 rooms, one per catalogue kind whose
	# benefit format matches a completed snapshot. Benefits come from the
	# catalogue effect, never copied here. Presentation-only probe; no
	# command, spend or sim state changes and the real snapshot is restored.
	var completed_kinds := ["Shelter", "Greenhouse", "CoalVein", "DeepMine"]
	for slot_index in completed_kinds.size():
		var kind: String = completed_kinds[slot_index]
		probe_row.slots[slot_index] = {
			"kind": kind, "complete": true, "progress": 1.0, "remaining": 0.0,
			"benefit": str(_catalogue["underground"]["rooms"][kind]["effect"]),
		}
	probe.levels.append(probe_row)
	_main.set_process(false)
	_panel.refresh(probe, phase)
	await _settle()
	var deeper_row := _levels.get_child(3)
	if not _check(_levels.get_child_count() == 4,
			"deeper-row probe did not build a fourth row"):
		return
	if not _check(not scroll.get_global_rect().encloses(deeper_row.get_global_rect()) \
			and deeper_row.get_global_rect().intersects(command_rect),
			"fourth row does not sit clipped across the command-card strip: row=%s viewport=%s card=%s" % [
				deeper_row.get_global_rect(), scroll.get_global_rect(), command_rect]):
		return
	# Real command click on the strip the unclipped fourth row would cover:
	# arm Move, then press Cancel's covered top edge. A row Button stealing
	# that press leaves `_armed_order` set.
	var probe_worker: Node3D = get_tree().get_nodes_in_group("unit_views")[0]
	await _click_view(probe_worker, MOUSE_BUTTON_LEFT)
	var probe_move := _main.get_node("HUD/CommandPanel/OrderGrid/MoveButton") as Button
	var probe_cancel := _main.get_node("HUD/CommandPanel/OrderGrid/CancelButton") as Button
	await _click(probe_move)
	if not _check(str(_main.get("_armed_order")) == "move",
			"deeper-row probe could not arm Move: %s" % str(_main.get("_armed_order"))):
		return
	var probe_revision: int = GrusBridge.command_feedback_revision()
	SmokeHelpers.push_mouse_click(get_tree(),
		probe_cancel.get_global_rect().position + Vector2(probe_cancel.size.x * 0.5, 6.0),
		MOUSE_BUTTON_LEFT)
	await _settle()
	if not _check(str(_main.get("_armed_order")) == "" and _status.text == "Order canceled" \
			and int(GrusBridge.command_feedback_revision()) == probe_revision,
			"deeper row intercepted the command-card Cancel click: armed=%s status=%s" % [
				str(_main.get("_armed_order")), _status.text]):
		return
	# Real wheel input must reach the deeper row, not just the visible three.
	for _attempt in 10:
		if scroll.get_global_rect().encloses(deeper_row.get_global_rect()):
			break
		SmokeHelpers.push_mouse_click(get_tree(), scroll.get_global_rect().get_center(), MOUSE_BUTTON_WHEEL_DOWN)
		await _settle()
	var probe_bar := scroll.get_v_scroll_bar()
	if not _check(scroll.get_global_rect().encloses(deeper_row.get_global_rect()) \
			and probe_bar.value > 0.0 and probe_bar.max_value > probe_bar.page,
			"real wheel input did not scroll to the deeper row: row=%s viewport=%s value=%s max=%s page=%s" % [
				deeper_row.get_global_rect(), scroll.get_global_rect(), probe_bar.value,
				probe_bar.max_value, probe_bar.page]):
		return
	# After the scrollbar exists and layout has settled, every completed L4
	# plate must render the same readout contract as a paid room.
	for slot_index in completed_kinds.size():
		var synthetic_slot := _drawer_slot(4, slot_index)
		var synthetic_error := _slot_readout_error(_slot(4, slot_index), synthetic_slot,
				"synthetic L4 %s" % str(synthetic_slot.get("kind", "")))
		if not _check(synthetic_error.is_empty(), synthetic_error):
			return
	# Synthetic presentation-only evidence of the four completed L4 rooms; the
	# real paid journey never funds L4. No command, spend or sim state change.
	var l4_capture := await SmokeHelpers.capture_frame(get_tree(), "underground-synthetic-l4-benefits")
	if not _check(l4_capture.is_empty(), l4_capture):
		return
	# Second refresh: the existing longest-name construction case. The L4 row
	# is already built, so no Button is rebuilt under this refresh.
	probe_row.slots[2] = {
		"kind": "Shelter", "complete": false, "progress": 0.99,
		"remaining": 1.0, "benefit": "",
	}
	_panel.refresh(probe, phase)
	await _settle()
	var long_name_slot := deeper_row.get_child(3) as Button
	if not _check(long_name_slot.text == "Shelter",
			"long-name construction changed the room label: \"%s\"" % long_name_slot.text):
		return
	var long_name_error := _slot_readout_error(long_name_slot, _drawer_slot(4, 2), "synthetic Shelter 99%")
	if not _check(long_name_error.is_empty(), long_name_error):
		return
	# Synthetic presentation-only evidence for the longest name at 99%; the
	# real paid journey never funds L4. No command, spend or sim state change.
	var long_name_capture := await SmokeHelpers.capture_frame(get_tree(), "underground-long-name")
	if not _check(long_name_capture.is_empty(), long_name_capture):
		return
	_main.set_process(true)
	_panel.refresh(GrusBridge.underground_snapshot(), phase)
	await _settle()
	if not _check(_levels.get_child_count() == 3 \
			and str(_slot_state(3, 0).kind) == "Lab" and str(_slot_state(3, 1).kind) == "GeothermalTap",
			"deeper-row probe did not restore the real three-row drawer"):
		return
	await _click(_kind("GeothermalTap"))
	if not _check(_kind("GeothermalTap").button_pressed,
			"deeper-row probe did not restore the armed Tap kind"):
		return
	var final_stock := _stock()
	# Expected totals rebuild from the recorded dig debits plus catalogue
	# room/barracks prices — retuning changes both sides together.
	var expected_scrap := int(room_costs.Lab.scrap) + int(room_costs.GeothermalTap.scrap) \
			+ 2 * int(barracks_cost.scrap)
	var expected_coal := 0
	for dig_cost in _dig_costs:
		expected_scrap += int(dig_cost.scrap)
		expected_coal += int(dig_cost.coal)
	if not _check(_spent_scrap == expected_scrap and _spent_coal == expected_coal \
			and int(final_stock.food) == int(SEED_TREASURY.food) \
			and int(final_stock.crystals) == int(SEED_TREASURY.crystals),
			"wrong purchases or unrelated currency mutation"):
		return
	var margin := float(GrusBridge.wave_snapshot().seconds_remaining)
	if not _check(SmokeHelpers.no_launched_wave_error("underground journey").is_empty() \
			and margin >= MARGIN_FLOOR_S,
			"underground journey left under %ds prep margin for its own tail" % int(MARGIN_FLOOR_S)):
		return

	# An unaffordable deeper dig takes the real rejection/feedback route.
	if not await _purchase(_excavate, "Excavate queued", {}, false):
		return
	if not _check(_stock() == final_stock and not bool(GrusBridge.underground_snapshot().excavating) \
			and not (_panel.get_node("DigProgress") as ProgressBar).visible \
			and int(GrusBridge.economy_snapshot().last_reject_code) == 8,
			"rejected deeper dig spent, started or lost typed InsufficientResources (8)"):
		return
	# Result-only Restart is not exposed while Playing, so emit the real
	# button's `pressed` signal: it still travels the production
	# signal → _on_restart_pressed → _restart_match path (the lifecycle
	# smoke covers its Result-only visibility), not a test-only reset.
	var wave_error := SmokeHelpers.no_launched_wave_error("underground smoke before Restart")
	if not _check(wave_error.is_empty(), wave_error):
		return
	var restart_button: Button = _main.get_node("HUD/SessionPanel/RestartButton")
	restart_button.pressed.emit()
	if not await _wait(func(): return str(GrusBridge.session_snapshot().phase) == "Start" \
			and int(GrusBridge.underground_snapshot().depth) == 0, 3.0, "Restart left stale underground depth"):
		return
	await _settle()
	if not _check(not bool(GrusBridge.underground_snapshot().excavating) \
			and GrusBridge.underground_snapshot().levels.is_empty() and _levels.get_child_count() == 0 \
			and _stock() == SEED_TREASURY, "Restart did not clear jobs/rows/treasury"):
		return
	# Restart must also clear the transient armed room kind, not just the
	# sim state: the GeothermalTap armed earlier in the journey must not
	# stay pressed into the fresh Start-phase match. Kind buttons are
	# toggles, so their public pressed state is the armed surface.
	var armed_cleared := true
	for room_kind in _catalogue.underground.room_kinds:
		armed_cleared = armed_cleared and not _kind(room_kind).button_pressed
	if not _check(armed_cleared, "Restart left a room kind armed or pressed"):
		return
	GrusBridge.set_sim_speed(1.0)
	print("GRUS_UNDERGROUND_SMOKE_OK gathered_scrap=%d gathered_coal=%d paid_scrap=%d paid_coal=%d cold_rate=%.4f heat_delta=%d margin_s=%.2f elapsed_ms=%d" % [
		gathered_scrap, gathered_coal, _spent_scrap, _spent_coal, cold_rate, tap_heat, margin, Time.get_ticks_msec() - _started_ms])
	get_tree().quit(0)
