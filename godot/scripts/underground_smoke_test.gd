extends Node

## Real paid underground journey. No injected resources, timer skips or
## direct underground commands: purchases use the same Buttons as players.
## Two paid Barracks make the Lab's cold-independent contribution observable
## before the Tap warms the base; workers fund every purchase by gathering.
const SmokeHelpers := preload("res://scripts/smoke_helpers.gd")
const VIEWPORT_SIZE := Vector2i(1280, 720)

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
	if not _check(GrusBridge.set_sim_speed(20.0), "20x speed rejected"):
		return
	if not await _wait(_fixture_ready, 5.0, "fixture views never settled"):
		return
	if not _check(_stock() == {"food": 200, "scrap": 300, "coal": 150, "crystals": 0}, "wrong seed treasury"):
		return
	var base_rate := float(GrusBridge.research_snapshot().rate_per_second)
	if not _check(is_equal_approx(base_rate, 2.0), "wrong full-heat surface research baseline"):
		return
	await _click(_main.get_node("HUD/SessionPanel/StartButton"))
	if not _check(str(GrusBridge.session_snapshot().phase) == "Playing", "Start UI failed"):
		return

	# First click really pays for L1 and leaves GUI focus on Excavate.
	await _key(KEY_TAB)
	if not _check(_panel.visible, "Tab did not open panel"):
		return
	var l1_before := _stock()
	if not await _purchase(_excavate, "Excavate queued", GrusBridge.underground_snapshot().next_cost):
		return
	if not _check(_stock().scrap == l1_before.scrap - 100 and _stock().coal == l1_before.coal - 50,
			"L1 must charge exactly 100 Scrap/50 Coal once"):
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
	var paused_stock := _stock()
	var paused_revision: int = GrusBridge.command_feedback_revision()
	for _frame in 12:
		await get_tree().physics_frame
	if not _check(GrusBridge.underground_snapshot() == paused and _stock() == paused_stock \
			and _excavate.disabled and _panel.get_node("HeaderLabel").text.contains("inspection only"),
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
	if not _check(_slot(1, 0).disabled, "unarmed empty slot must be disabled"):
		return
	if not await _dig(2):
		return
	# Once the final dig's Coal is banked, move that worker to Scrap:
	# surplus Coal cannot fund the two cold-proof Barracks.
	if not await _wait(func(): return int(_stock().coal) >= 128 and int(_deposits.coal) >= 108, 5.0, "Coal gatherer did not fund L3"):
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
			if not _check(str(row.slots[slot].kind) == "" and not bool(row.slots[slot].building), "bad 0-based empty slot"):
				return

	# Fund Lab+Tap and two ordinary Barracks. Retire gatherers before exact
	# debit/no-spend checks so no real deposit can disguise a broken charge.
	var room_costs: Dictionary = _catalogue.underground.rooms
	var barracks_cost: Dictionary = _catalogue.buildings.Barracks
	var remaining_scrap := int(room_costs.Lab.scrap) + int(room_costs.GeothermalTap.scrap) + 2 * int(barracks_cost.scrap)
	if not await _wait(func(): return int(_stock().scrap) >= remaining_scrap \
			and int(_deposits.scrap) >= 726, 10.0, "Gather did not fund rooms and cold proof"):
		return
	await _key(KEY_TAB)
	for worker in workers:
		await _click_view(worker, MOUSE_BUTTON_LEFT)
		await _key(KEY_S)
	await _settle()
	_track_deposits = false
	var gathered_scrap := int(_deposits.scrap)
	var gathered_coal := int(_deposits.coal)
	if not _check(gathered_scrap >= 726 and gathered_coal >= 108, "real deposits did not cover total shortfall"):
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
	var disabled_before := _stock()
	var disabled_revision: int = GrusBridge.command_feedback_revision()
	var selected_before: Array = _main.selected_ids.duplicate()
	for level in [1, 2]:
		for slot in 4:
			var button := _slot(level, slot)
			if not _check(button.disabled and button.text.contains("L3") and button.tooltip_text.contains("requires L3"),
					"Lab armed did not disable/explain shallow target"):
				return
			await _click(button)
	if not _check(_stock() == disabled_before and GrusBridge.command_feedback_revision() == disabled_revision \
			and _main.selected_ids == selected_before, "disabled slot click queued, spent or leaked to battlefield"):
		return
	for level in [1, 2]:
		for slot in 4:
			if not _check(str(_slot_state(level, slot).kind) == "", "disabled shallow target built a room"):
				return
	if not _check(not _slot(3, 0).disabled, "L3 did not enable Lab target"):
		return
	var before_lab := _stock()
	if not await _purchase(_slot(3, 0), "Lab queued (L3 slot 1)", room_costs.Lab):
		return
	if not _check(_stock().scrap == before_lab.scrap - 150 and _stock().coal == before_lab.coal \
			and str(_slot_state(3, 0).kind) == "Lab" and not bool(_slot_state(3, 0).complete),
			"Lab did not charge once at L3/slot0 and start construction"):
		return
	await _click(_kind("Shelter"))
	if not _check(not _slot(1, 0).disabled and _slot(3, 0).disabled, "Shelter failed to re-enable L1 or occupied slot actionable"):
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
	var cold: Dictionary = GrusBridge.economy_snapshot()
	var cold_rate := float(GrusBridge.research_snapshot().rate_per_second)
	if not _check(int(cold.heat_supply) == 15 and float(cold.surface_efficiency) < 1.0 \
			and absf(cold_rate - (base_rate * float(cold.surface_efficiency) + 1.0)) < 0.0001,
			"Lab +1/s was cold-scaled instead of additive"):
		return

	await _click(_kind("GeothermalTap"))
	var before_tap := _stock()
	if not await _purchase(_slot(3, 1), "Tap queued (L3 slot 2)", room_costs.GeothermalTap):
		return
	if not _check(_stock().scrap == before_tap.scrap - 120 and _stock().coal == before_tap.coal \
			and str(_slot_state(3, 1).kind) == "GeothermalTap" and not bool(_slot_state(3, 1).complete),
			"Tap did not charge exactly once at L3/slot1"):
		return
	if not await _wait(func(): return bool(_slot_state(3, 1).complete), 6.0, "Tap did not complete"):
		return
	var warm: Dictionary = GrusBridge.economy_snapshot()
	if not _check(int(warm.heat_supply) == int(cold.heat_supply) + 15 and int(warm.heat_demand) == 16 \
			and is_equal_approx(float(warm.surface_efficiency), 1.0) \
			and is_equal_approx(float(GrusBridge.research_snapshot().rate_per_second), base_rate + 1.0),
			"L3 Tap did not add 15 supply and warm Lab rate did not retain +1/s"):
		return
	var final_stock := _stock()
	if not _check(_spent_scrap == 1026 and _spent_coal == 258 \
			and int(final_stock.food) == 200 and int(final_stock.crystals) == 0,
			"wrong purchases or unrelated currency mutation"):
		return
	var margin := float(GrusBridge.wave_snapshot().seconds_remaining)
	if not _check(SmokeHelpers.no_launched_wave_error("underground journey").is_empty() and margin > 0.0,
			"underground journey overran preparation"):
		return

	# An unaffordable deeper dig takes the real rejection/feedback route.
	if not await _purchase(_excavate, "Excavate queued", {}, false):
		return
	if not _check(_stock() == final_stock and not bool(GrusBridge.underground_snapshot().excavating) \
			and int(GrusBridge.economy_snapshot().last_reject_code) == 8,
			"rejected deeper dig spent, started or lost typed InsufficientResources (8)"):
		return
	# Result-only Restart is not exposed while Playing. Exercise the same
	# production controller handler (also used by the lifecycle smoke), not
	# a test-only reset or injected Result, while retaining the prep guard.
	_main.call("_restart_match")
	if not await _wait(func(): return str(GrusBridge.session_snapshot().phase) == "Start" \
			and int(GrusBridge.underground_snapshot().depth) == 0, 3.0, "Restart left stale underground depth"):
		return
	await _settle()
	if not _check(not bool(GrusBridge.underground_snapshot().excavating) \
			and GrusBridge.underground_snapshot().levels.is_empty() and _levels.get_child_count() == 0 \
			and _stock() == {"food": 200, "scrap": 300, "coal": 150, "crystals": 0}, "Restart did not clear jobs/rows/treasury"):
		return
	GrusBridge.set_sim_speed(1.0)
	print("GRUS_UNDERGROUND_SMOKE_OK gathered_scrap=%d gathered_coal=%d paid_scrap=%d paid_coal=%d cold_rate=%.4f heat_delta=15 margin_s=%.2f elapsed_ms=%d" % [
		gathered_scrap, gathered_coal, _spent_scrap, _spent_coal, cold_rate, margin, Time.get_ticks_msec() - _started_ms])
	get_tree().quit(0)
