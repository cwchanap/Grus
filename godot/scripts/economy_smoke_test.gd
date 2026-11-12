extends Node

## HPA-448 end-to-end economy smoke. Drives the real controller/UI paths —
## selection, contextual right-clicks, build buttons with the placement
## preview, production buttons, rally points, and idle-worker navigation —
## through a solvent script that stockpiles, builds, and trains one of each
## combat unit on the single Barracks producer. GrusBridge snapshots are used
## for assertions only; every gameplay write rides the UI.

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## RejectReason discriminant values (declaration order in
## crates/grus-sim/src/commands.rs); the bridge reports "no rejection" as -1.
const REJECT_OCCUPIED := 10
const REJECT_INSUFFICIENT := 8

## Authored fixture source ids (crates/grus-sim/src/fixture.rs).
const RATIONS_ID := 1
const SALVAGE_ID := 3
const COAL_ID := 6
## Northwest expansion salvage used for the Depot delivery proof.
const FAR_SALVAGE_ID := 14

## Solvency targets before any spending: deposit-driven, so reaching them
## proves real gather cycles (starts are 200 Food / 300 Scrap / 150 Coal).
const TARGET_SCRAP := 400
const TARGET_FOOD := 320
const TARGET_COAL := 160

## Post-reset heat leg solvency: the Barracks/Wall/Gate pre-spend (160)
## during the funding leg leaves Bunkhouse 50 + Depot 75 + Gun Turret 100 +
## Coal Furnace 80 + the 40-Scrap Rifleman queue = 345 to cover, so the
## reset's 300 starting Scrap forces at least 205 extra Scrap through real
## gather cycles before the remaining placements (the funding wait runs
## after the defense pre-charges). The Turret's 20 Coal comes out of the
## untouched 150 stockpile, leaving fuel for the Furnace burn proof.
const HEAT_FUNDING_SCRAP := 345

## Free anchors: a Bunkhouse slot beside the central city's south-east
## worker corner and a Depot beside the scouted northwest expansion salvage.
## The Barracks slot and the Occupied probe cell come from the live Research
## Center view at run time (SmokeHelpers) — never a copied fixture cell.
const BUNKHOUSE_ANCHOR := Vector2i(66, 52)
const DEPOT_ANCHOR := Vector2i(40, 20)

## Open ground west of the city; workers parked here stay well outside the
## 20 px unit-priority click radius of every building origin.
const STAGING_CELL := Vector2i(5, 47)
## Research Center rally cell for the rally-point proof.
const RALLY_CELL := Vector2i(20, 58)
## Selection is a real click whose projected point goes stale while the
## buffered input waits at 20x. Each attempt re-projects from the live view;
## the attempt count is a small fixed bound, not an extended timeout.
const STAGING_SELECT_ATTEMPTS := 4
## Far outside the controller's 20 px pick radius: an injected miss cannot
## accidentally select the intended unit.
const STAGING_MISS_OFFSET := Vector2(120.0, 0.0)

const SmokeHelpers := preload("res://scripts/smoke_helpers.gd")

func _visible_text(control: Control) -> String:
	if control is RichTextLabel:
		return (control as RichTextLabel).get_parsed_text()
	return str(control.get("text"))

func _rich_text_has_icon(control: Control, resource_path: String) -> bool:
	return control is RichTextLabel and (control as RichTextLabel).text.contains(resource_path)

var _camera: Camera3D
var _start_ms := 0
var _building_ids := {}
## Test-only failure injection for the staging interaction. Production paths
## never set these; the smoke still sends every click/key through real input.
var _staging_click_miss_remaining := 0
var _staging_click_miss_all := false
var _staging_miss_selection_cleared := false
var _staging_reject_interleave_remaining := 0

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	# Best-effort restore on failure exits too; the process quits either way.
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(1)

func _gather_wait_details(elapsed_seconds: float) -> String:
	var economy: Dictionary = GrusBridge.economy_snapshot()
	var session: Dictionary = GrusBridge.session_snapshot()
	var wave: Dictionary = GrusBridge.wave_snapshot()
	var idle_ids: PackedInt32Array = economy.get("idle_worker_ids", PackedInt32Array())
	var worker_states := PackedStringArray()
	for worker_id in [1, 2, 3, 4]:
		var worker := _unit_view(worker_id)
		if worker == null:
			worker_states.append("%d:missing" % worker_id)
			continue
		var activity := "idle" if idle_ids.has(worker_id) else "ordered"
		worker_states.append("%d:%s@(%.1f,%.1f)" % [
			worker_id, activity, worker.global_position.x, worker.global_position.z])
	var summary := "elapsed=%.1fs stockpile=%d/%d/%d/%d phase=%s wave=%s countdown=%s workers=[%s] command_revision=%d feedback=%s" % [
		elapsed_seconds, int(economy.get("food", -1)), int(economy.get("scrap", -1)),
		int(economy.get("coal", -1)), int(economy.get("crystals", -1)),
		str(session.get("phase", "unknown")), str(wave.get("wave", "unknown")),
		str(wave.get("seconds_remaining", "unknown")), ",".join(worker_states),
		GrusBridge.command_feedback_revision(), GrusBridge.command_feedback()]
	return summary

func _mouse_click(position: Vector2, button: int) -> void:
	var press := InputEventMouseButton.new()
	press.button_index = button
	press.position = position
	press.pressed = true
	Input.parse_input_event(press)

	var release := InputEventMouseButton.new()
	release.button_index = button
	release.position = position
	release.pressed = false
	Input.parse_input_event(release)

func _mouse_motion(position: Vector2) -> void:
	var motion := InputEventMouseMotion.new()
	motion.position = position
	Input.parse_input_event(motion)

func _key_tap(keycode: int) -> void:
	var press := InputEventKey.new()
	press.keycode = keycode
	press.pressed = true
	Input.parse_input_event(press)

	var release := InputEventKey.new()
	release.keycode = keycode
	release.pressed = false
	Input.parse_input_event(release)

func _click_view(view: Node3D, button: int, fog_settle := false) -> void:
	_mouse_click(_camera.unproject_position(view.global_position), button)
	await get_tree().process_frame
	await get_tree().process_frame
	# Extra frame only for fog-gated targets whose view was recently
	# revealed; plain clicks keep the original single-frame settle.
	if fog_settle:
		await get_tree().process_frame

func _click_world(point: Vector2, button: int) -> void:
	_mouse_click(_camera.unproject_position(Vector3(point.x + 0.25, 0.0, point.y + 0.25)), button)
	await get_tree().process_frame
	await get_tree().process_frame

func _click_button(button: Button) -> void:
	_mouse_click(button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame

## Polls a condition every physics frame until it holds or the frame budget
## (timeout_s of physics ticks) is exhausted; fails the smoke on timeout.
## `details` is called only on timeout so the log carries live diagnostics.
func _wait_until(condition: Callable, timeout_s: float, message: String, details := Callable()) -> bool:
	var frame_budget := int(ceilf(timeout_s * Engine.physics_ticks_per_second))
	var gather_wait := message.begins_with("gatherers never stockpiled")
	var gather_started_ms := Time.get_ticks_msec()
	for _frame in frame_budget:
		if condition.call():
			return true
		await get_tree().physics_frame
	var suffix := ""
	if gather_wait:
		var elapsed := float(Time.get_ticks_msec() - gather_started_ms) / 1000.0
		suffix = ": %s" % _gather_wait_details(elapsed)
	elif details.is_valid():
		suffix = ": %s" % details.call()
	_fail("%s (waited %.0fs)%s" % [message, timeout_s, suffix])
	return false

func _find_view(group: String, meta: String, id: int) -> Node3D:
	for node in get_tree().get_nodes_in_group(group):
		if int(node.get_meta(meta, -1)) == id:
			return node as Node3D
	return null

func _unit_view(id: int) -> Node3D:
	return _find_view("unit_views", "unit_id", id)

func _resource_view(id: int) -> Node3D:
	return _find_view("resource_views", "resource_id", id)

func _building_view(id: int) -> Node3D:
	return _find_view("building_views", "building_id", id)

func _construction_progress(id: int) -> float:
	return float(GrusBridge.building_snapshot(id).get("construction_progress", 0.0))

func _queue_label(id: int) -> String:
	return str(GrusBridge.building_snapshot(id).get("queue_label", ""))

func _queue_progress(id: int) -> float:
	return float(GrusBridge.building_snapshot(id).get("queue_progress", 0.0))

func _training_queue(id: int) -> Array:
	return GrusBridge.building_snapshot(id).get("training_queue", [])

func _blocked_reason(id: int) -> int:
	return int(GrusBridge.building_snapshot(id).get("blocked_reason", -1))

## HPA-698 queue presentation floor: every visible socket and its unit icon
## must be visibly larger than the old 44 px / 30 px strip, sit inside the
## selection panel's padded plate, and never overlap the status rows, the
## portrait or each other. Returns "" only when the whole row holds.
func _queue_layout_error(sockets: Container, where: String) -> String:
	var panel := get_node("Main/HUD/SelectionPanel") as Control
	var status := get_node("Main/HUD/SelectionPanel/SelectionLabel") as Control
	var portrait := get_node("Main/HUD/SelectionPanel/PortraitBezel") as Control
	if sockets == null or panel == null or status == null or portrait == null:
		return "%s queue layout controls are missing" % where
	var overflow := sockets.get_node_or_null("OverflowChip") as Control
	if overflow == null:
		return "%s queue overflow chip is missing" % where
	var padded := panel.get_global_rect().grow(-8.0)
	var row := sockets.get_global_rect()
	if not padded.encloses(row):
		return "%s queue row %s leaves the padded selection panel %s" % [where, row, padded]
	if row.intersects(status.get_global_rect()) or row.intersects(portrait.get_global_rect()):
		return "%s queue row %s overlaps the status/portrait content" % [where, row]
	var previous := Rect2()
	var seen := false
	for child in sockets.get_children():
		var control := child as Control
		if control == null or not control.visible:
			continue
		var rect := control.get_global_rect()
		if not padded.encloses(rect):
			return "%s queue control %s leaves the padded selection panel %s" % [
				where, control.name, padded]
		if seen and rect.intersects(previous):
			return "%s queue controls overlap: %s intersects %s" % [where, control.name, previous]
		previous = rect
		seen = true
		if control is Panel:
			if rect.size.x < 64.0 or rect.size.y < 60.0:
				return "%s socket %s is %s, below the 64x60 presentation floor" % [
					where, control.name, rect.size]
			var icon := control.get_node_or_null("Icon") as TextureRect
			if icon == null or icon.size.x < 44.0 or icon.size.y < 44.0:
				return "%s socket %s icon is %s, below the 44 px presentation floor" % [
					where, control.name, icon.size if icon != null else Vector2.ZERO]
			if not rect.encloses(icon.get_global_rect()):
				return "%s socket %s icon escapes its socket" % [where, control.name]
		elif control != overflow:
			return "%s queue row has an unexpected visible child %s" % [where, control.name]
	return ""

## Painted queue treatment: queued sockets reuse the theme's square Tile
## normal plate and the head its pressed plate, read from the active theme so
## the expected texture paths are never copied here. Empty sockets keep the
## recessed flat square (the round socket.png is not a queue bezel). The
## painted plate must not resize the sockets: their 64x64 floor and the
## queue row's bounds stay the ones the test last validated.
func _queue_painted_treatment_error(sockets: Container, filled: int, where: String) -> String:
	if sockets == null:
		return "%s queue sockets are missing" % where
	var hud := get_node("Main/HUD/SelectionPanel") as Control
	var tile_normal := hud.get_theme_stylebox("normal", "Tile")
	var tile_pressed := hud.get_theme_stylebox("pressed", "Tile")
	if not tile_normal is StyleBoxTexture or not tile_pressed is StyleBoxTexture:
		return "%s theme Tile normal/pressed styles are not textures" % where
	var normal_path := (tile_normal as StyleBoxTexture).texture.resource_path
	var pressed_path := (tile_pressed as StyleBoxTexture).texture.resource_path
	if normal_path.is_empty() or pressed_path.is_empty() or normal_path == pressed_path:
		return "%s theme Tile normal/pressed styles have no distinct texture paths: %s %s" % [
			where, normal_path, pressed_path]
	var row := sockets.get_global_rect()
	for index in 5:
		var panel := sockets.get_child(index) as Panel
		if panel == null:
			return "%s socket %d is not a Panel" % [where, index + 1]
		if not row.encloses(panel.get_global_rect()):
			return "%s socket %d bounds %s leave the queue row %s" % [
				where, index + 1, panel.get_global_rect(), row]
		if not panel.get_combined_minimum_size().is_equal_approx(panel.custom_minimum_size):
			return "%s socket %d bounds grew with its plate: min %s, floor %s" % [
				where, index + 1, panel.get_combined_minimum_size(), panel.custom_minimum_size]
		var style := panel.get_theme_stylebox("panel")
		if index < filled:
			var expected := pressed_path if index == 0 else normal_path
			if not style is StyleBoxTexture \
					or (style as StyleBoxTexture).texture.resource_path != expected:
				return "%s socket %d does not carry the theme Tile %s plate %s" % [
					where, index + 1, "pressed" if index == 0 else "normal", expected]
		elif style is StyleBoxTexture:
			return "%s empty socket %d carries a painted plate instead of the recessed square" % [
				where, index + 1]
	return ""

func _fixture_views_settled() -> bool:
	var units := get_tree().get_nodes_in_group("unit_views")
	var buildings := get_tree().get_nodes_in_group("building_views")
	var resources := get_tree().get_nodes_in_group("resource_views")
	if units.size() != 4 or buildings.size() != 1 or resources.size() != 18:
		return false
	for unit in units:
		if int(unit.get_meta("unit_id", -1)) <= 0:
			return false
	for building in buildings:
		if int(building.get_meta("building_id", -1)) <= 0:
			return false
	for resource in resources:
		if int(resource.get_meta("resource_id", -1)) <= 0:
			return false
	return true

func _stockpiles_at_least(scrap: int, food: int, coal: int) -> bool:
	var state: Dictionary = GrusBridge.economy_snapshot()
	return int(state.get("scrap", -1)) >= scrap \
		and int(state.get("food", -1)) >= food \
		and int(state.get("coal", -1)) >= coal

func _building_complete_at(id: int) -> bool:
	return bool(GrusBridge.building_snapshot(id).get("complete", false))

## Heat boundary stage: the Rust snapshot's supply/demand/efficiency and the
## rendered HUD third line must both report the expected stage — the HUD
## text is the acceptance surface, the snapshot pins the exact fraction. The
## top-bar heat bar and cold chip are display-only projections of that same
## snapshot and must appear exactly while the surface is below full efficiency.
func _heat_stage_ok(supply: int, demand: int, efficiency: float, hud_line: String) -> bool:
	var economy: Dictionary = GrusBridge.economy_snapshot()
	if int(economy.get("heat_supply", -1)) != supply \
			or int(economy.get("heat_demand", -1)) != demand \
			or absf(float(economy.get("surface_efficiency", 0.0)) - efficiency) > 0.0001:
		return false
	var label := get_node("Main/HUD/EconomyPanel/EconomyLabel") as Control
	var heat_bar := get_node_or_null("Main/HUD/EconomyPanel/HeatBar") as ProgressBar
	var cold_chip := get_node_or_null("Main/HUD/EconomyPanel/ColdChip") as RichTextLabel
	var frost_overlay := get_node_or_null("Main/HUD/Atmosphere/FrostOverlay") as TextureRect
	if label == null or heat_bar == null or cold_chip == null or frost_overlay == null:
		return false
	var deficit := efficiency < 1.0
	if heat_bar.visible != deficit or cold_chip.visible != deficit \
			or frost_overlay.visible != deficit \
			or frost_overlay.mouse_filter != Control.MOUSE_FILTER_IGNORE \
			or frost_overlay.texture == null:
		return false
	if deficit:
		var percent := roundi(efficiency * 100.0)
		if percent >= 100:
			percent = 99
		if not is_equal_approx(heat_bar.max_value, float(maxi(demand, 1))) \
				or not is_equal_approx(heat_bar.value, float(mini(supply, demand))) \
				or not _visible_text(cold_chip).contains("%d%%" % percent) \
				or not _rich_text_has_icon(cold_chip, "res://assets/ui/icons/status/surface_cold.png"):
			return false
	return _visible_text(label).contains(hud_line)

## Every mesh of a building view's active Model carries the shared
## translucent ice material_overlay exactly while its Rust-pushed cold
## state is set — the material itself is read off the view's script, never
## re-typed, and multiple views must share the one immutable resource.
func _overlay_is(view: Node3D, cold: bool) -> bool:
	var model := view.get_node_or_null("Model") as Node3D
	var building_script: Script = view.get_script()
	if model == null or building_script == null:
		return false
	var ice: Material = building_script.get("ICE_OVERLAY")
	if ice == null:
		return false
	var want: Material = ice if cold else null
	var meshes := model.find_children("*", "MeshInstance3D", true, false)
	if meshes.is_empty():
		return false
	for node in meshes:
		if (node as MeshInstance3D).material_overlay != want:
			return false
	return true

func _all_overlaid(views: Array, cold: bool) -> bool:
	for view in views:
		if not _overlay_is(view, cold):
			return false
	return true

## Delivery proof for the Depot: a Scrap stockpile credit that lands while
## the worker stands near the Depot. Its far-tree gather slots sit a few
## cells from the Depot origin and ~30 from the Research Center, so only a
## real Depot deposit can satisfy this within one physics frame of drift.
## The worker is re-resolved from its stable id every poll: a captured view
## can be freed between frames (the same stale-capture class that already
## aborted this wait), and a freed bound Node raises instead of failing the
## predicate.
func _scrap_deposited_at_depot(worker_id: int, depot: Vector3, box: Dictionary) -> bool:
	var scrap := int(GrusBridge.economy_snapshot().get("scrap", -1))
	var increased := scrap > int(box.get("scrap", scrap))
	box["scrap"] = scrap
	var worker := _unit_view(worker_id)
	return increased and worker != null and worker.global_position.distance_to(depot) < 12.0

func _new_building_present(kind: String, box: Dictionary) -> bool:
	for node in get_tree().get_nodes_in_group("building_views"):
		var id := int(node.get_meta("building_id", -1))
		if id > 0 and not _building_ids.has(id) and str(node.get_meta("building_kind", "")) == kind:
			box["view"] = node
			return true
	return false

func _wait_building(kind: String, timeout_s: float) -> Node3D:
	var box := {"view": null}
	if not await _wait_until(_new_building_present.bind(kind, box), timeout_s, "new %s building view never appeared" % kind):
		return null
	var view: Node3D = box["view"]
	_building_ids[int(view.get_meta("building_id"))] = true
	return view

func _unit_of_kind_present(kind: String, min_id: int, box: Dictionary) -> bool:
	for node in get_tree().get_nodes_in_group("unit_views"):
		var id := int(node.get_meta("unit_id", -1))
		if id >= min_id and str(node.get_meta("unit_kind", "")) == kind:
			box["view"] = node
			box["id"] = id
			return true
	return false

## Waits until the controller's screen-space unit picker cannot intercept a
## click at the building origin, so the next left-click selects the building.
func _wait_units_clear(point: Vector3, timeout_s: float) -> bool:
	var screen_position := _camera.unproject_position(point)
	var main := get_node("Main") as Node3D
	var frame_budget := int(ceilf(timeout_s * Engine.physics_ticks_per_second))
	for _frame in frame_budget:
		if main.call("_nearest_view", "unit_views", screen_position, 1) == null:
			return true
		await get_tree().physics_frame
	_fail("units never cleared the click zone at %s" % point)
	return false

func _select_building(view: Node3D) -> void:
	if not await _wait_units_clear(view.global_position, 15.0):
		return
	await _click_view(view, MOUSE_BUTTON_LEFT)

## The controller's current selection, read from the live test surface.
func _selected_ids() -> Array:
	var main := get_node_or_null("Main")
	var selected: Variant = main.get("selected_ids") if main != null else null
	return selected as Array if selected is Array else []

## Applies a test-only off-target offset when the miss counters are armed.
func _staging_click_point(unit_id: int, attempt: int, screen: Vector2) -> Vector2:
	if not _staging_click_miss_all and _staging_click_miss_remaining <= 0:
		return screen
	if not _staging_click_miss_all:
		_staging_click_miss_remaining -= 1
	print("GRUS_ECONOMY_STAGING_MISS_INJECTED unit=%d attempt=%d at=%s" % [
		unit_id, attempt + 1, screen])
	return screen + STAGING_MISS_OFFSET

## Clears selection through a real left click on the open staging cell before
## a deliberately missed probe click. A pre-existing selection must never make
## an injected miss look like a successful selection.
func _clear_selection_for_miss_probe(unit_id: int) -> bool:
	var screen := _camera.unproject_position(
		Vector3(STAGING_CELL.x + 0.5, 0.0, STAGING_CELL.y + 0.5))
	var main := get_node("Main")
	if (main.call("_nearest_view", "unit_views", screen, 0) as Node3D) != null \
			or (main.call("_building_at", screen, 0) as Node3D) != null:
		_fail("miss probe clear point %s is not open ground for unit %d" % [
			screen, unit_id])
		return false
	for rect in SmokeHelpers.hud_rects(get_node("Main/HUD")):
		if rect.has_point(screen):
			_fail("miss probe clear point %s is under HUD %s" % [screen, rect])
			return false
	_mouse_click(screen, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	var selected := _selected_ids()
	if not selected.is_empty():
		_fail("miss probe could not clear selection before unit %d: %s; %s" % [
			unit_id, selected, _staging_details(unit_id)])
		return false
	print("GRUS_ECONOMY_STAGING_MISS_SELECTION_CLEARED unit=%d at=%s" % [unit_id, screen])
	return true

## Selects exactly one stable id through real left clicks. A buffered click
## projected before a 20x frame can land behind a moving worker, so each
## bounded attempt re-resolves the live view and re-projects immediately
## before sending; no stale coordinate is retried.
func _select_unit(unit_id: int) -> bool:
	var attempts := PackedStringArray()
	var injected_misses := 0
	for attempt in STAGING_SELECT_ATTEMPTS:
		var injected := _staging_click_miss_all or _staging_click_miss_remaining > 0
		if injected and not _staging_miss_selection_cleared:
			if not await _clear_selection_for_miss_probe(unit_id):
				return false
			_staging_miss_selection_cleared = true
		var view := _unit_view(unit_id)
		if view == null:
			_fail("staging target %d vanished during selection: %s" % [
				unit_id, _staging_details(unit_id)])
			return false
		var screen := _staging_click_point(
			unit_id, attempt, _camera.unproject_position(view.global_position))
		_mouse_click(screen, MOUSE_BUTTON_LEFT)
		await get_tree().process_frame
		await get_tree().process_frame
		var selected := _selected_ids()
		attempts.append("%d@%s view=%s%s=>%s" % [
			attempt + 1, screen, view.global_position,
			" (injected miss)" if injected else "", selected])
		if injected:
			injected_misses += 1
			if selected.size() == 1 and int(selected[0]) == unit_id:
				_fail("injected miss falsely reports unit %d selected at %s; probe invalid: %s" % [
					unit_id, screen, _staging_details(unit_id)])
				return false
		elif selected.size() == 1 and int(selected[0]) == unit_id:
			if injected_misses > 0:
				print("GRUS_ECONOMY_STAGING_SELECT_RECOVERED unit=%d attempt=%d attempts=%s" % [
					unit_id, attempt + 1, "; ".join(attempts)])
			return true
	_fail("staging click did not select unit %d after %d fresh-coordinate attempts: %s; %s" % [
		unit_id, STAGING_SELECT_ATTEMPTS, "; ".join(attempts), _staging_details(unit_id)])
	return false

## Retires a gatherer through the UI (select + Stop) for the intended stable
## id. Finite sources deplete at 20x virtual time, and a depleted source
## leaves the worker idled at its drop-off slot — sometimes inside a building
## click zone — so the scenario stops its gatherers once the banked
## thresholds are met. Selection must resolve to exactly this id before S;
## otherwise a missed click would Stop whatever was selected instead.
func _stop_unit(unit_id: int) -> bool:
	if not await _select_unit(unit_id):
		return false
	var selected := _selected_ids()
	if selected.size() != 1 or int(selected[0]) != unit_id:
		_fail("Stop would target %s instead of unit %d: %s" % [
			selected, unit_id, _staging_details(unit_id)])
		return false
	_key_tap(KEY_S)
	await get_tree().process_frame
	# Correlated Stop result: a worker (non-empty task snapshot) must be Idle,
	# proving the accepted Stop cancelled this id's gather/route rather than a
	# different selection. Combatants carry no WorkerTask; their later Move
	# arrival proves the same for the physical order.
	var stop_task := str(GrusBridge.unit_task_snapshot(unit_id).get("task", ""))
	if not stop_task.is_empty() and stop_task != "Idle":
		if not await _wait_until(
				func(): return str(GrusBridge.unit_task_snapshot(unit_id).get("task", "")) == "Idle",
				2.0, "Stop did not cancel unit %d's worker task" % unit_id,
				func(): return _staging_details(unit_id)):
			return false
	return true

## A diagnosis string for one stable unit id; used when staging fails so the
## log records identity, live position, selection, task, session and wave
## phase and the last command verdict instead of a bare timeout.
func _staging_details(unit_id: int) -> String:
	var view := _unit_view(unit_id)
	var economy: Dictionary = GrusBridge.economy_snapshot()
	var session: Dictionary = GrusBridge.session_snapshot()
	var wave: Dictionary = GrusBridge.wave_snapshot()
	var idle: PackedInt32Array = economy.get("idle_worker_ids", PackedInt32Array())
	var main := get_node_or_null("Main")
	var selected: Variant = main.get("selected_ids") if main != null else null
	var position := "missing" if view == null else "(%.2f,%.2f)" % [
		view.global_position.x, view.global_position.z]
	return "unit=%d view=%s position=%s idle=%s selection=%s task=%s phase=%s wave=%s countdown=%s revision=%d feedback=%s last_reject=%d" % [
		unit_id, "live" if view != null else "missing", position, idle.has(unit_id),
		selected if selected is Array else [], GrusBridge.unit_task_snapshot(unit_id),
		str(session.get("phase", "unknown")), str(wave.get("wave", "unknown")),
		str(wave.get("seconds_remaining", "unknown")),
		GrusBridge.command_feedback_revision(), GrusBridge.command_feedback(),
		GrusBridge.last_reject_code()]

## Stable-id preflight for staging: "" only while the id resolves to a live
## unit view. Callers pass ids, never captured views — a view can be freed
## between frames, and a typed freed argument aborts before any check.
func _staging_target_error(unit_id: int) -> String:
	if _unit_view(unit_id) != null:
		return ""
	return "staging target %d is not a live unit view: %s" % [unit_id, _staging_details(unit_id)]

## Stops a unit and parks it on the staging ground, clear of every later
## building-origin click. Takes the stable id and re-resolves the view after
## every await; returns false with the diagnosis already logged so callers
## stop immediately instead of driving a freed or absent worker further.
func _park_at_staging(unit_id: int) -> bool:
	var target_error := _staging_target_error(unit_id)
	if not target_error.is_empty():
		_fail(target_error)
		return false
	var view := _unit_view(unit_id)
	if view == null:
		_fail("staging target %d was freed before Stop: %s" % [unit_id, _staging_details(unit_id)])
		return false
	if not await _stop_unit(unit_id):
		return false
	view = _unit_view(unit_id)
	if view == null:
		_fail("staging target %d was freed after Stop: %s" % [unit_id, _staging_details(unit_id)])
		return false
	if not await _select_unit(unit_id):
		return false
	view = _unit_view(unit_id)
	if view == null:
		_fail("staging target %d was freed before the Move: %s" % [unit_id, _staging_details(unit_id)])
		return false
	var start_position := view.global_position
	var staging := Vector3(STAGING_CELL.x + 0.5, 0.0, STAGING_CELL.y + 0.5)
	await _click_world(Vector2(STAGING_CELL), MOUSE_BUTTON_RIGHT)
	# Test-only interleave: after the staging Move is dispatched and before its
	# result is read, issue a real rejected Move for another stable unit (the
	# builder) into a bridge-reported ruin. Its route rejection overwrites the
	# global feedback/reject and must never decide the staging Move's result;
	# the intended unit's arrival does.
	if _staging_reject_interleave_remaining > 0:
		_staging_reject_interleave_remaining -= 1
		var rects: PackedInt32Array = GrusBridge.obstacle_rects()
		if rects.size() < 20:
			_fail("unrelated rejection injection has no ruin quad 4: %s" % [rects])
			return false
		var ruin_min := Vector2i(rects[16], rects[17])
		var ruin_max := Vector2i(rects[18], rects[19])
		var ruin_cells := ruin_max - ruin_min + Vector2i.ONE
		var blocked := Vector2(
			ruin_min.x + ruin_cells.x * 0.5, ruin_min.y + ruin_cells.y * 0.5)
		if not await _select_unit(4):
			return false
		await _click_world(blocked, MOUSE_BUTTON_RIGHT)
		await get_tree().process_frame
		await get_tree().process_frame
		var injected_reject := int(GrusBridge.last_reject_code())
		print("GRUS_ECONOMY_STAGING_REJECT_INTERLEAVED reject=%d feedback=%s selection=%s" % [
			injected_reject, GrusBridge.command_feedback(), _selected_ids()])
		if injected_reject == -1:
			_fail("unrelated rejection injection did not land: feedback=%s selection=%s" % [
				GrusBridge.command_feedback(), _selected_ids()])
			return false
	# The global feedback/reject is diagnostic only: an unrelated route failure
	# can overwrite it after this Move. Arrival by the intended stable id is the
	# correlated result that proves the Move reached this unit.
	if not await _wait_until(
			func():
				var live := _unit_view(unit_id)
				return live != null and live.global_position.distance_to(staging) < 3.0,
			15.0, "retired gatherer never reached the staging ground",
			func(): return _staging_arrival_details(unit_id, start_position)):
		return false
	return true

## Timeout diagnosis for a staging Move: how far the intended unit travelled
## from where the order was issued, plus the live identity/phase snapshot.
func _staging_arrival_details(unit_id: int, start: Vector3) -> String:
	var live := _unit_view(unit_id)
	var moved := -1.0 if live == null else start.distance_to(live.global_position)
	return "from=%s moved=%.2f; %s" % [start, moved, _staging_details(unit_id)]

## Full UI placement: select builder, press the build button, sweep the
## placement preview over the anchor, assert it rendered, then place.
func _place_building(builder: Node3D, button: Button, anchor: Vector2i) -> void:
	await _click_view(builder, MOUSE_BUTTON_LEFT)
	await _click_button(button)
	var screen_point := _camera.unproject_position(Vector3(anchor.x + 0.25, 0.0, anchor.y + 0.25))
	_mouse_motion(screen_point)
	await get_tree().process_frame
	var preview := get_node("Main/PlacementPreview") as MeshInstance3D
	if preview == null or not preview.visible:
		_fail("placement preview did not render for anchor %s" % anchor)
		return
	await _click_world(Vector2(anchor.x, anchor.y), MOUSE_BUTTON_LEFT)

func _rally_stored(building_id: int) -> bool:
	var snap: Dictionary = GrusBridge.building_snapshot(building_id)
	return int(snap.get("rally_x", -1)) == RALLY_CELL.x and int(snap.get("rally_y", -1)) == RALLY_CELL.y

## Exactly one friendly unit ringed, and its id is a live idle_worker_ids
## member — the idle navigation contract.
func _single_ringed_idle_worker() -> bool:
	var ringed := PackedInt32Array()
	for node in get_tree().get_nodes_in_group("unit_views"):
		var unit := node as Node3D
		var ring := unit.get_node_or_null("SelectionRing") as MeshInstance3D
		if ring != null and ring.visible:
			ringed.append(int(unit.get_meta("unit_id", -1)))
	if ringed.size() != 1:
		return false
	var idle: PackedInt32Array = GrusBridge.economy_snapshot().get("idle_worker_ids", PackedInt32Array())
	return idle.has(ringed[0])

func _run() -> void:
	_start_ms = Time.get_ticks_msec()
	# Test-only injection knobs. They alter only real-input coordinates or add
	# a real rejected UI command; they never write selection or gameplay.
	_staging_click_miss_remaining = OS.get_environment(
		"GRUS_ECONOMY_STAGING_MISS_CLICKS").to_int()
	_staging_click_miss_all = OS.get_environment("GRUS_ECONOMY_STAGING_MISS_ALL") == "1"
	_staging_reject_interleave_remaining = OS.get_environment(
		"GRUS_ECONOMY_STAGING_REJECT_INTERLEAVE").to_int()
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame

	var viewport_size := get_viewport().get_visible_rect().size
	if viewport_size != Vector2(TEST_VIEWPORT_SIZE):
		_fail("economy smoke viewport did not resize to project baseline: %s" % viewport_size)
		return
	_camera = get_node("Main/Camera3D") as Camera3D

	if not GrusBridge.set_sim_speed(20.0):
		_fail("failed to accelerate Bevy virtual time")
		return

	if not await _wait_until(_fixture_views_settled, 20.0,
			"skirmish startup never settled to 4 units / 1 building / 18 resources"):
		return

	# Stable-id regressions for the staging helper. A missing id must be
	# diagnosed, and the caller pattern must stop the journey instead of
	# driving real input at a freed/absent worker. The failure probes exit
	# nonzero on purpose: GRUS_SMOKE_STAGING_FAILURE_PROBE=1 (missing id) or
	# =missed-click (every real selection click deliberately misses).
	var missing_stage := _staging_target_error(999)
	if missing_stage.is_empty() or not missing_stage.contains("999"):
		_fail("staging preflight did not diagnose a missing stable id: %s" % missing_stage)
		return
	var staging_probe := OS.get_environment("GRUS_SMOKE_STAGING_FAILURE_PROBE")
	if staging_probe == "1":
		if not await _park_at_staging(999):
			print("GRUS_ECONOMY_STAGING_FAILURE_PROPAGATED")
		return
	if staging_probe == "missed-click":
		_staging_click_miss_all = true
		if not await _park_at_staging(2):
			print("GRUS_ECONOMY_STAGING_MISSED_CLICK_PROPAGATED")
		return

	var boot: Dictionary = GrusBridge.economy_snapshot()
	if int(boot.get("food", -1)) != 200 or int(boot.get("scrap", -1)) != 300 or int(boot.get("coal", -1)) != 150:
		_fail("starting stockpile is not 200/300/150: %s" % [boot])
		return
	# The catalogue snapshot is the only gameplay-data source for the UI:
	# prove it carries real cost/producer entries from the Rust table — and
	# none of the deleted PvP-roster kinds.
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var catalogue_units: Dictionary = catalogue.get("units", {})
	var worker_entry: Dictionary = catalogue_units.get("Worker", {})
	if int(worker_entry.get("food", -1)) != 50 or str(worker_entry.get("producer", "")) != "ResearchCenter":
		_fail("catalogue snapshot is missing Worker cost/producer: %s" % [catalogue])
		return
	var guard_entry: Dictionary = catalogue_units.get("Guard", {})
	if int(guard_entry.get("food", -1)) != 60 or str(guard_entry.get("producer", "")) != "Barracks":
		_fail("catalogue snapshot does not produce Guards and Riflemen at the Barracks: %s" % [catalogue])
		return
	var rifleman_entry: Dictionary = catalogue_units.get("Rifleman", {})
	if int(rifleman_entry.get("food", -1)) != 40 or int(rifleman_entry.get("scrap", -1)) != 40 \
			or str(rifleman_entry.get("producer", "")) != "Barracks":
		_fail("catalogue snapshot does not produce Riflemen at the Barracks: %s" % [catalogue])
		return
	for removed in ["Cavalry", "Farm", "ArcheryRange", "Stable"]:
		if catalogue_units.has(removed) or catalogue.get("buildings", {}).has(removed):
			_fail("catalogue still advertises the deleted %s" % removed)
			return
	# The training catalogue stays human-only: alien rows would otherwise
	# contribute an empty producer kind and route ordinary ground
	# right-clicks to set_rally(-1, ...).
	if catalogue_units.size() != 3 or catalogue_units.has("Swarmer"):
		_fail("training catalogue is not the human-only UnitKind::HUMAN loop: %s" % [catalogue_units.keys()])
		return
	# HPA-698 combat/health stats ride the same Rust specs: Worker is a
	# noncombatant with no optional combat keys, Guard and Rifleman carry
	# their real damage/range/counter values.
	if int(worker_entry.get("max_health", -1)) != 50 or worker_entry.has("damage"):
		_fail("Worker catalogue stats are wrong: %s" % [worker_entry])
		return
	if int(guard_entry.get("max_health", -1)) != 100 \
			or int(guard_entry.get("damage", -1)) != 10 \
			or not is_equal_approx(float(guard_entry.get("range", -1.0)), 1.5) \
			or str(guard_entry.get("counter_target", "")) != "Swarmer" \
			or int(guard_entry.get("counter_bonus", -1)) != 5:
		_fail("Guard catalogue combat stats are wrong: %s" % [guard_entry])
		return
	if int(rifleman_entry.get("max_health", -1)) != 70 \
			or int(rifleman_entry.get("damage", -1)) != 8 \
			or not is_equal_approx(float(rifleman_entry.get("range", -1.0)), 6.0) \
			or str(rifleman_entry.get("counter_target", "")) != "Spitter" \
			or int(rifleman_entry.get("counter_bonus", -1)) != 4:
		_fail("Rifleman catalogue combat stats are wrong: %s" % [rifleman_entry])
		return
	if int(catalogue.get("carry_limit", -1)) != 10:
		_fail("catalogue carry_limit is not the Rust CARRY_LIMIT: %s" % [catalogue])
		return
	var turret_entry: Dictionary = catalogue.get("buildings", {}).get("GunTurret", {})
	if int(turret_entry.get("max_health", -1)) != 350 \
			or int(turret_entry.get("heat_supply", -1)) != 0 \
			or int(turret_entry.get("heat_demand", -1)) != 2 \
			or int(turret_entry.get("damage", -1)) != 10 \
			or not is_equal_approx(float(turret_entry.get("range", -1.0)), 8.0) \
			or str(turret_entry.get("counter_target", "")) != "Spitter" \
			or int(turret_entry.get("counter_bonus", -1)) != 5:
		_fail("Gun Turret catalogue stat row is wrong: %s" % [turret_entry])
		return
	var research_center_entry: Dictionary = catalogue.get("buildings", {}).get("ResearchCenter", {})
	if int(research_center_entry.get("max_health", -1)) != 800 \
			or int(research_center_entry.get("heat_supply", -1)) != 15 \
			or int(research_center_entry.get("heat_demand", -1)) != 10 \
			or research_center_entry.has("damage"):
		_fail("Research Center catalogue stat row is wrong: %s" % [research_center_entry])
		return
	# HPA-181: the Coal Furnace build button exists and reads its footprint
	# and cost/build time from this same catalogue snapshot — no Godot
	# balance literals.
	var furnace_entry: Dictionary = catalogue.get("buildings", {}).get("CoalFurnace", {})
	if int(furnace_entry.get("scrap", -1)) != 80 or int(furnace_entry.get("seconds", -1)) != 20 \
			or int(furnace_entry.get("width", -1)) != 2 or int(furnace_entry.get("height", -1)) != 2:
		_fail("catalogue snapshot is missing the 2x2 / 80-Scrap / 20 s Coal Furnace: %s" % [furnace_entry])
		return
	var furnace_button := get_node_or_null("Main/HUD/CommandPanel/BuildGrid/CoalFurnaceButton") as Button
	if furnace_button == null \
			or not furnace_button.tooltip_text.begins_with("CoalFurnace\nCost: 80 Scrap\nBuild: 20s") \
			or not furnace_button.tooltip_text.contains("HP: %d" % int(furnace_entry.get("max_health", 0))) \
			or not furnace_button.tooltip_text.contains("Heat: +12 supply"):
		_fail("Coal Furnace build button does not show the catalogue-derived tooltip: \"%s\"" %
				(furnace_button.tooltip_text if furnace_button != null else "<missing>"))
		return
	if catalogue.has("age_up"):
		_fail("catalogue still advertises the deleted Age-up job")
		return
	if absf(float(boot.get("gather_rate", 0.0)) - 2.0) > 0.0001:
		_fail("starting gather rate is not 2.0: %s" % [boot])
		return
	if int(boot.get("population_used", -1)) != 4 or int(boot.get("population_cap", -1)) != 10:
		_fail("starting population is not 4/10: %s" % [boot])
		return
	# Invalid ids stay empty snapshots: the new queue/carry keys sit behind
	# the same missing/fog-hidden guards as the existing payload.
	for bad_id in [-1, 999999]:
		if not GrusBridge.building_snapshot(bad_id).is_empty() \
				or not GrusBridge.unit_task_snapshot(bad_id).is_empty():
			_fail("invalid id %d leaked a snapshot: %s / %s" % [bad_id,
				GrusBridge.building_snapshot(bad_id), GrusBridge.unit_task_snapshot(bad_id)])
			return
	var idle_task: Dictionary = GrusBridge.unit_task_snapshot(1)
	if str(idle_task.get("task", "")) != "Idle" or int(idle_task.get("carry_amount", -1)) != 0 \
			or str(idle_task.get("carry_kind", "?")) != "":
		_fail("idle worker 1 snapshot does not report an empty Carry: %s" % [idle_task])
		return
	if boot.has("age"):
		_fail("economy snapshot still carries the deleted age field: %s" % [boot])
		return
	# The HUD economy label must render all four currencies with the live
	# boot values — Crystals included — straight off economy_snapshot. The
	# session is still at Start, so the label cannot have drifted from the
	# 200/300/150/0 seed; the placeholder's zero stocks differ on Food.
	var economy_label := get_node("Main/HUD/EconomyPanel/EconomyLabel") as RichTextLabel
	if economy_label == null:
		_fail("economy HUD label is missing")
		return
	# Icon-first readability: the row uses 28 px painted icons and an 18 px
	# value face — the approved mockup's prominent horizontal resource bar,
	# not the old compressed three-line block.
	if economy_label.get_theme_font_size("normal_font_size") < 18:
		_fail("economy HUD value font is below the 18 px readability floor: %d" % economy_label.get_theme_font_size("normal_font_size"))
		return
	for icon_name in ["food", "scrap", "coal", "crystals", "heat"]:
		var icon_path := "res://assets/ui/icons/resources/%s.png" % icon_name
		if not _rich_text_has_icon(economy_label, icon_path):
			_fail("economy HUD is missing required icon %s" % icon_path)
			return
	if not _rich_text_has_icon(economy_label, "res://assets/ui/icons/status/population.png"):
		_fail("economy HUD is missing the population icon")
		return
	var heat_bar := get_node_or_null("Main/HUD/EconomyPanel/HeatBar") as ProgressBar
	var cold_chip := get_node_or_null("Main/HUD/EconomyPanel/ColdChip") as RichTextLabel
	if heat_bar == null or cold_chip == null:
		_fail("economy HUD is missing the heat supply/demand bar or cold chip")
		return
	if heat_bar.visible or cold_chip.visible:
		_fail("warm economy HUD shows the cold bar/chip at full efficiency")
		return
	if _rich_text_has_icon(economy_label, "res://assets/ui/icons/status/surface_cold.png"):
		_fail("warm economy HUD shows an incorrect surface-cold warning")
		return
	var economy_text := _visible_text(economy_label)
	if economy_text.contains("\n"):
		_fail("economy HUD is not a single horizontal icon/value row: \"%s\"" % economy_text)
		return
	var economy_markup := economy_label.text
	var previous_pair_end := 0
	for resource in [
		["Food", "food", "food", 0],
		["Scrap", "scrap", "scrap", 0],
		["Coal", "coal", "coal", 0],
		["Crystals", "crystals", "crystals", 0],
	]:
		var currency := str(resource[0])
		var icon_path := "res://assets/ui/icons/resources/%s.png" % str(resource[2])
		var shown := str(int(boot.get(str(resource[1]), -1)))
		var expected_pair := "[img=28x28]%s[/img] %s" % [icon_path, shown]
		var pair_position := economy_markup.find(expected_pair, previous_pair_end)
		if pair_position < 0:
			_fail("economy HUD does not pair the %s icon with its live value %s: \"%s\"" % [
				currency, shown, economy_text])
			return
		previous_pair_end = pair_position + expected_pair.length()
	var population_pair := "[img=28x28]res://assets/ui/icons/status/population.png[/img] %d / %d" % [
		int(boot.get("population_used", -1)), int(boot.get("population_cap", -1))]
	if not economy_markup.contains(population_pair):
		_fail("economy HUD population pair is not the live used/cap value: %s" % economy_markup)
		return
	var heat_pair := "[img=28x28]res://assets/ui/icons/resources/heat.png[/img] %d / %d" % [
		int(boot.get("heat_supply", -1)), int(boot.get("heat_demand", -1))]
	if not economy_markup.contains(heat_pair):
		_fail("economy HUD heat pair is not the live supply/demand value: %s" % economy_markup)
		return
	# Rendered bounds, not just text: after the layout settles, the label's
	# laid-out extent (rect grown to the minimum text size — wrapping is
	# off, so a wide line expands past the authored offsets) must stay
	# inside EconomyPanel and the baseline viewport.
	await get_tree().process_frame
	var rendered := economy_label.get_global_rect()
	var content_size := Vector2(economy_label.get_content_width(), economy_label.get_content_height())
	rendered.size = rendered.size.max(content_size)
	var panel_rect := (get_node("Main/HUD/EconomyPanel") as Control).get_global_rect()
	if not panel_rect.encloses(rendered):
		_fail("economy HUD label rendered bounds %s escape EconomyPanel %s" % [rendered, panel_rect])
		return
	var viewport_rect := Rect2(Vector2.ZERO, get_viewport().get_visible_rect().size)
	if not viewport_rect.encloses(rendered):
		_fail("economy HUD label rendered bounds %s escape the %s viewport" % [rendered, viewport_rect])
		return
	if content_size.y > 40.0 or content_size.x > panel_rect.size.x - 20.0:
		_fail("economy HUD row wraps or overflows instead of one horizontal bar: %s" % content_size)
		return
	# HPA-181: the live heat boundary stays in the same row; the boot is
	# still at Start, so the warm Research Center baseline must be exact.
	if not economy_text.contains("15 / 10 · surface 100%"):
		_fail("economy HUD heat readout is not the warm 15/10 baseline: \"%s\"" % economy_text)
		return
	var research_center := _building_view(1)
	if research_center == null:
		_fail("Research Center view 1 is missing")
		return
	var research_center_id := int(research_center.get_meta("building_id"))
	_building_ids = {1: true}
	var builder := _unit_view(4)
	if builder == null:
		_fail("builder worker 4 is missing")
		return

	# Normal skirmish boots into Start; this scenario starts the match before
	# its first gameplay command.
	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return

	# Step 1: workers onto salvage/rations/coal via right-click; stockpiles
	# move only after a deposit, never on the command itself.
	for job in [[1, SALVAGE_ID], [2, RATIONS_ID], [3, COAL_ID]]:
		var worker := _unit_view(job[0])
		var source := _resource_view(job[1])
		if worker == null or source == null:
			_fail("gather worker/source view missing for %s" % [job])
			return
		await _click_view(worker, MOUSE_BUTTON_LEFT)
		await _click_view(source, MOUSE_BUTTON_RIGHT)

	var before: Dictionary = GrusBridge.economy_snapshot()
	for _frame in 2:
		await get_tree().physics_frame
	var after: Dictionary = GrusBridge.economy_snapshot()
	if int(after.get("scrap", -1)) != int(before.get("scrap", -1)) \
			or int(after.get("food", -1)) != int(before.get("food", -1)) \
			or int(after.get("coal", -1)) != int(before.get("coal", -1)):
		_fail("stockpile moved before any worker could gather and deposit: %s -> %s" % [before, after])
		return

	# The live worker Carry reaches unit_task_snapshot through the same real
	# gather order: worker 1 heads to the authored salvage source, so its
	# carried kind reads Scrap and its amount stays inside the catalogue's
	# Rust CARRY_LIMIT. Poll after the stockpile comparison so deposits the
	# gather orders have already begun can never fail that earlier premise.
	if not await _wait_until(
			func():
				var task: Dictionary = GrusBridge.unit_task_snapshot(1)
				return str(task.get("carry_kind", "")) == "Scrap" \
					and int(task.get("carry_amount", 0)) > 0 \
					and int(task.get("carry_amount", 0)) <= int(catalogue.get("carry_limit", 0)),
			30.0, "worker 1 never reported a live Scrap carry through unit_task_snapshot"):
		return
	# HPA-698 carry detail: the selected worker's HUD bar and label mirror the
	# same live carry amount and the Rust CARRY_LIMIT, with the resource icon
	# only while a kind is held.
	await _click_view(_unit_view(1), MOUSE_BUTTON_LEFT)
	var carry_bar := get_node_or_null("Main/HUD/SelectionPanel/CarryBar") as ProgressBar
	var carry_label := get_node_or_null("Main/HUD/SelectionPanel/CarryLabel") as RichTextLabel
	if carry_bar == null or carry_label == null:
		_fail("selection carry detail nodes are missing under HUD/SelectionPanel")
		return
	if not await _wait_until(func():
			var task: Dictionary = GrusBridge.unit_task_snapshot(1)
			var parsed := carry_label.get_parsed_text()
			var amount_text := parsed.split(" / ")[0].strip_edges()
			var limit := int(catalogue.get("carry_limit", 0))
			# The HUD refresh and this bridge read can straddle one gather tick;
			# require the rendered bar/label to agree with each other exactly and
			# with the live carry within one gather step, rather than forcing the
			# same instant under frame throttling.
			return carry_bar.visible and carry_label.visible \
				and is_equal_approx(carry_bar.max_value, float(limit)) \
				and amount_text.is_valid_int() and int(amount_text) == int(carry_bar.value) \
				and parsed.contains("/ %d" % limit) \
				and absf(carry_bar.value - float(task.get("carry_amount", -1))) <= 3.0 \
				and carry_label.text.contains("resources/%s.png" % str(task.get("carry_kind", "")).to_snake_case()),
			10.0, "selected worker carry detail does not mirror the live Carry snapshot"):
		return

	# Steps 2-3: solvent gather targets (deposit-driven) before any spending.
	if not await _wait_until(_stockpiles_at_least.bind(TARGET_SCRAP, TARGET_FOOD, TARGET_COAL), 150.0,
			"gatherers never stockpiled %d scrap / %d food / %d coal" % [TARGET_SCRAP, TARGET_FOOD, TARGET_COAL]):
		return

	# Step 4: Bunkhouse raises the population cap 10 -> 20; its construction
	# progress ramps through snapshots (Step 10 evidence).
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/BunkhouseButton") as Button, BUNKHOUSE_ANCHOR)
	var bunkhouse := await _wait_building("Bunkhouse", 20.0)
	if bunkhouse == null:
		return
	var bunkhouse_id := int(bunkhouse.get_meta("building_id"))
	# HPA-291: the unfinished Bunkhouse renders the generic construction
	# site until the bridge's pushed state says complete.
	if not await _wait_until(func():
			var model := bunkhouse.get_node_or_null("Model") as Node3D
			return not _building_complete_at(bunkhouse_id) \
				and model != null and model.scene_file_path.ends_with("construction_site.glb"),
			20.0, "incomplete Bunkhouse never showed the construction site"):
		return
	if not await _wait_until(func(): return _construction_progress(bunkhouse_id) > 0.0, 20.0,
			"Bunkhouse construction progress never started"):
		return
	var mid_progress := 0.0
	if not bool(GrusBridge.building_snapshot(bunkhouse_id).get("complete", false)):
		mid_progress = _construction_progress(bunkhouse_id)
	if not await _wait_until(func(): return _construction_progress(bunkhouse_id) > mid_progress, 20.0,
			"Bunkhouse construction progress never advanced"):
		return
	if not await _wait_until(_building_complete_at.bind(bunkhouse_id), 30.0, "Bunkhouse never completed"):
		return
	if _construction_progress(bunkhouse_id) < 1.0:
		_fail("completed Bunkhouse construction progress is not 1.0")
		return
	if int(GrusBridge.economy_snapshot().get("population_cap", -1)) != 20:
		_fail("completed Bunkhouse did not raise the population cap to 20")
		return
	# The completion push swaps the construction site for the kind's final
	# GLB exactly once; the geometry must still sit inside the normalized
	# ±0.5 footprint the 2x2 root scale blows up to world size.
	if not await _wait_until(func():
			var model := bunkhouse.get_node_or_null("Model") as Node3D
			return model != null and model.scene_file_path.ends_with("bunkhouse.glb"),
			10.0, "completed Bunkhouse never swapped to its final model"):
		return
	var art_err := SmokeHelpers.view_art_error(bunkhouse)
	if art_err != "":
		_fail(art_err)
		return

	# Fog retarget (HPA-473): the Depot anchor (40,20) and the northwest
	# expansion salvage boot Unexplored — placement validates explored
	# footprints and gather requires explored sources, so scout the corridor
	# first. The expansion salvage's view becoming visible is the explored
	# proof (views are the presentation projection of the sim fog).
	await _click_view(builder, MOUSE_BUTTON_LEFT)
	await _click_world(Vector2(38, 24), MOUSE_BUTTON_RIGHT)
	if not await _wait_until(
			func(): return _resource_view(FAR_SALVAGE_ID) != null \
					and (_resource_view(FAR_SALVAGE_ID) as Node3D).is_visible_in_tree(),
			60.0, "scout never revealed the far expansion salvage"):
		return

	# Step 5: Depot delivery proof. Retask worker 1 to the northwest
	# expansion salvage (a few cells from the Depot, ~30 from the Research
	# Center): every later Scrap deposit must land at the Depot.
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/DepotButton") as Button, DEPOT_ANCHOR)
	var depot := await _wait_building("Depot", 25.0)
	if depot == null:
		return
	var depot_id := int(depot.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(depot_id), 30.0, "Depot never completed"):
		return
	var scrap_worker := _unit_view(1)
	var far_salvage := _resource_view(FAR_SALVAGE_ID)
	if scrap_worker == null or far_salvage == null:
		_fail("scrap worker or expansion salvage view is missing")
		return
	await _click_view(scrap_worker, MOUSE_BUTTON_LEFT)
	# The far salvage's view is fog-revealed: it pays the fog settle.
	await _click_view(far_salvage, MOUSE_BUTTON_RIGHT, true)
	var delivery_box := {"scrap": int(GrusBridge.economy_snapshot().get("scrap", -1))}
	if not await _wait_until(_scrap_deposited_at_depot.bind(1, depot.global_position, delivery_box), 45.0,
			"no scrap delivery ever reached the Depot"):
		return
	# Retire the scrap gatherer: its northwest corridor stays clear of every
	# later click target, so parking in place is safe. Stable id only.
	if not await _stop_unit(1):
		return

	# Reject-code pin: the read-only placement preview reports the numeric
	# Occupied code for a footprint inside the seeded Research Center — the same
	# RejectReason discriminant the bridge emits on real rejects. The probe
	# cell is the live view's center, inside its footprint.
	var occupied_cell := SmokeHelpers.cell_of(research_center)
	var occupied_preview: Dictionary = GrusBridge.placement_preview(4, "Bunkhouse", occupied_cell.x, occupied_cell.y)
	if int(occupied_preview.get("reject_code", -1)) != REJECT_OCCUPIED:
		_fail("placement preview inside the Research Center did not report Occupied (%d): %s" % [REJECT_OCCUPIED, occupied_preview])
		return

	# Retire the food and coal gatherers: the finite sources deplete at 20x
	# virtual time and would otherwise idle a worker inside the Research Center
	# click zone. Park both on the staging ground. Stable ids only — a view can
	# be freed while another staging wait is in flight.
	if not await _park_at_staging(2) or not await _park_at_staging(3):
		return

	# Step 6: a Research Center rally point; the next trained unit walks to it.
	await _select_building(research_center)
	# HPA-698 contextual train filtering: the Research Center is the Worker
	# producer, so only its catalogue-matching tile is visible.
	var train_grid := get_node("Main/HUD/CommandPanel/TrainGrid") as Container
	var worker_button := get_node("Main/HUD/CommandPanel/TrainGrid/WorkerButton") as Button
	var filter_guard_button := get_node("Main/HUD/CommandPanel/TrainGrid/GuardButton") as Button
	var filter_rifleman_button := get_node("Main/HUD/CommandPanel/TrainGrid/RiflemanButton") as Button
	if not await _wait_until(func():
			return train_grid.visible and worker_button.visible \
				and not filter_guard_button.visible and not filter_rifleman_button.visible,
			5.0, "Research Center selection did not filter train tiles to Worker"):
		return
	await _click_world(Vector2(RALLY_CELL), MOUSE_BUTTON_RIGHT)
	if not await _wait_until(_rally_stored.bind(research_center_id), 10.0,
			"Research Center rally point was never stored"):
		return
	# HPA-698 five-socket FIFO projection: six real Worker clicks fill all
	# five sockets and surface the +1 overflow chip. The queue is asserted
	# before the 20x head job (15 s) can finish.
	for _job in 6:
		await _click_button(worker_button)
	if not await _wait_until(func(): return _training_queue(research_center_id).size() == 6,
			10.0, "six real Worker enqueues never reached the FIFO queue"):
		return
	var sockets := get_node("Main/HUD/SelectionPanel/QueueSockets") as Container
	var overflow_chip := sockets.get_node_or_null("OverflowChip") as Label
	if sockets == null or overflow_chip == null or sockets.get_child_count() != 6 \
			or not overflow_chip.visible or overflow_chip.text != "+1":
		_fail("five-socket queue does not render its +1 overflow chip: sockets=%s chip=%s" % [
			sockets.get_child_count() if sockets != null else -1,
			overflow_chip.text if overflow_chip != null else "missing"])
		return
	var queue_layout_error := _queue_layout_error(sockets, "Research Center overflow")
	if not queue_layout_error.is_empty():
		_fail(queue_layout_error)
		return
	var painted_error := _queue_painted_treatment_error(sockets, 5, "Research Center overflow")
	if not painted_error.is_empty():
		_fail(painted_error)
		return
	var capture_error := await SmokeHelpers.capture_frame(get_tree(), "economy-queue-overflow")
	if not capture_error.is_empty():
		_fail(capture_error)
		return
	var worker_icon := "res://assets/ui/icons/units/worker.png"
	for socket_index in 5:
		var socket := sockets.get_child(socket_index) as Panel
		var icon := socket.get_node("Icon") as TextureRect
		if icon.texture == null or icon.texture.resource_path != worker_icon:
			_fail("queue socket %d does not carry the Worker FIFO icon" % (socket_index + 1))
			return
	var head_time := (sockets.get_child(0).get_node("Time") as Label).text
	if head_time.is_empty() or not head_time.ends_with("s") \
			or not (sockets.get_child(1).get_node("Time") as Label).text.is_empty():
		_fail("queue sockets do not show the head remaining time only: %s" % [head_time])
		return
	if not await _wait_until(func():
			var head_progress := sockets.get_child(0).get_node("Progress") as ProgressBar
			var next_progress := sockets.get_child(1).get_node("Progress") as ProgressBar
			return head_progress.visible and not next_progress.visible \
				and absf(head_progress.value - _queue_progress(research_center_id) * 100.0) <= 12.0,
			5.0, "head socket progress does not track the live queue_progress"):
		return
	var rally_worker_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Worker", 5, rally_worker_box), 30.0,
			"rallied Worker view never appeared"):
		return
	if int(GrusBridge.economy_snapshot().get("population_used", -1)) != 5 \
			or int(GrusBridge.economy_snapshot().get("population_cap", -1)) != 20:
		_fail("first economy leg did not reach population 5 of 20 with the first trained Worker")
		return
	var rally_worker: Node3D = rally_worker_box["view"]
	var rally_target := Vector3(RALLY_CELL.x + 0.5, 0.0, RALLY_CELL.y + 0.5)
	if not await _wait_until(
			func(): return rally_worker.global_position.distance_to(rally_target) < 1.5,
			15.0, "rallied worker never moved toward the rally point"):
		return
	# The gather/build/rally leg is complete. Assert the countdown stayed
	# intact before resetting the fixture, then run military production and
	# insufficient-resource coverage in a fresh Playing leg so neither smoke
	# interaction can be masked by an unintended alien wave.
	var wave_error := SmokeHelpers.no_launched_wave_error("economy smoke before reset")
	if not wave_error.is_empty():
		_fail(wave_error)
		return
	if not GrusBridge.reset_fixture():
		_fail("fixture reset between economy smoke legs was rejected")
		return
	if not await _wait_until(_fixture_views_settled, 20.0,
			"economy fixture did not settle after its guarded reset"):
		return
	_building_ids = {1: true}
	research_center = _building_view(1)
	research_center_id = int(research_center.get_meta("building_id", -1))
	builder = _unit_view(4)
	if research_center == null or builder == null:
		_fail("Research Center or builder worker missing after the guarded reset")
		return
	if not GrusBridge.start_match():
		_fail("start_match rejected the second economy smoke leg")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("second economy smoke leg did not reach Playing")
		return

	# HPA-181 heat leg funding: the reset's 300 starting Scrap cannot cover
	# the eight spenditures — Barracks 120 + Bunkhouse 50 + Depot 75 + Gun
	# Turret 100 + Coal Furnace 80 + Wall 10 + Gate 30 + the 40-Scrap
	# Rifleman queue = 505 — so Barracks, Wall and Gate pre-spend during
	# the gather leg while workers 1-3 bank real gather/deposit cycles off
	# the salvage source; the stockpile only moves on deposits, so reaching
	# the remaining 345 proves the funding (300 start − 160 pre-spend +
	# ≥205 gathered ≥ 345).
	for worker_id in [1, 2, 3]:
		var gatherer := _unit_view(worker_id)
		var salvage := _resource_view(SALVAGE_ID)
		if gatherer == null or salvage == null:
			_fail("heat-leg gatherer or salvage view missing for worker %d" % worker_id)
			return
		await _click_view(gatherer, MOUSE_BUTTON_LEFT)
		await _click_view(salvage, MOUSE_BUTTON_RIGHT)
	var barracks_anchor := SmokeHelpers.barracks_anchor(4, research_center)
	if barracks_anchor.x < 0:
		_fail("no valid Barracks anchor in the Research Center's west ring")
		return
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/BarracksButton") as Button, barracks_anchor)
	var barracks := await _wait_building("Barracks", 25.0)
	if barracks == null:
		return
	var barracks_id := int(barracks.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(barracks_id), 40.0, "Barracks never completed"):
		return

	# Zero-demand defenses never feed the heat ledger: the builder raises a
	# Wall and a Gate while the funding gather still runs, so both stand
	# complete when the deficit below arrives — the stage stays pinned and
	# neither view ever takes the cold overlay. Each site completes before
	# the builder is reassigned, and Wall/Gate tool arming is cleared with
	# Esc so subsequent clicks target views cleanly.
	var wall_anchor := SmokeHelpers.ring_anchor(4, "Wall", research_center)
	if wall_anchor.x < 0:
		_fail("no valid heat-leg Wall anchor near the Research Center")
		return
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/WallButton") as Button, wall_anchor)
	var heat_wall := await _wait_building("Wall", 25.0)
	if heat_wall == null:
		return
	var heat_wall_id := int(heat_wall.get_meta("building_id"))
	_key_tap(KEY_ESCAPE)
	await get_tree().process_frame
	if not await _wait_until(_building_complete_at.bind(heat_wall_id), 15.0, "heat-leg Wall never completed"):
		return

	var gate_anchor := SmokeHelpers.ring_anchor(4, "Gate", research_center)
	if gate_anchor.x < 0:
		_fail("no valid heat-leg Gate anchor near the Research Center")
		return
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/GateButton") as Button, gate_anchor)
	var heat_gate := await _wait_building("Gate", 25.0)
	if heat_gate == null:
		return
	var heat_gate_id := int(heat_gate.get_meta("building_id"))
	_key_tap(KEY_ESCAPE)
	await get_tree().process_frame
	if not await _wait_until(_building_complete_at.bind(heat_gate_id), 15.0, "heat-leg Gate never completed"):
		return

	if not await _wait_until(
			func(): return int(GrusBridge.economy_snapshot().get("scrap", -1)) >= HEAT_FUNDING_SCRAP,
			120.0, "heat leg never banked %d Scrap through real gather cycles" % HEAT_FUNDING_SCRAP):
		return

	# Step 7: the Barracks is the single military producer — it accepts both
	# Guard and Rifleman on one FIFO queue, each charged once at acceptance.
	# The ring slot west of the city is probed off the live RC view.
	if _blocked_reason(barracks_id) != 0 or _queue_label(barracks_id) != "" \
			or not _training_queue(barracks_id).is_empty():
		_fail("fresh Barracks queue is not idle and unblocked: blocked=%d label=%s queue=%s" % [
			_blocked_reason(barracks_id), _queue_label(barracks_id), _training_queue(barracks_id)])
		return

	# HPA-181 heat boundary leg, all through real UI placement paths and
	# funded inside this post-reset leg. Warm baseline first: the Research
	# Center (15 supply / 10 demand) plus the completed Barracks (+3 demand)
	# reads 15/13, still a full 100%.
	if not await _wait_until(
			_heat_stage_ok.bind(15, 13, 1.0, "15 / 13 · surface 100%"),
			10.0, "heat baseline after the Barracks never read 15/13 at 100%"):
		return
	# Funding proven earlier in the leg — retire the gatherers like the
	# first leg: finite
	# sources idle workers at drop-off slots at 20x, and the later
	# stockpile-unchanged assertions (queue charge, enqueue rejects) need a
	# quiet Scrap flow to stay exact.
	for worker_id in [1, 2, 3]:
		if not await _park_at_staging(worker_id):
			return

	# Cold demand: Depot (+1) and Gun Turret (+2) push demand past supply —
	# 15/16 ≈ 93.75%, reported as 94% (a deficit never rounds up to 100%) —
	# and every completed demand-bearing friendly view must carry the icy
	# overlay on its active model while it stands.
	var depot_anchor := SmokeHelpers.ring_anchor(4, "Depot", research_center)
	if depot_anchor.x < 0:
		_fail("no valid heat-leg Depot anchor near the Research Center")
		return
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/DepotButton") as Button, depot_anchor)
	var heat_depot := await _wait_building("Depot", 25.0)
	if heat_depot == null:
		return
	var heat_depot_id := int(heat_depot.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(heat_depot_id), 30.0, "heat-leg Depot never completed"):
		return

	var turret_anchor := SmokeHelpers.ring_anchor(4, "GunTurret", research_center)
	if turret_anchor.x < 0:
		_fail("no valid heat-leg Gun Turret anchor near the Research Center")
		return
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/GunTurretButton") as Button, turret_anchor)
	var heat_turret := await _wait_building("GunTurret", 25.0)
	if heat_turret == null:
		return
	var heat_turret_id := int(heat_turret.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(heat_turret_id), 35.0, "heat-leg Gun Turret never completed"):
		return
	if not await _wait_until(
			_heat_stage_ok.bind(15, 16, 0.9375, "15 / 16 · surface 94%"),
			10.0, "cold stage after Depot+Turret never read 15/16 at 94%"):
		return
	var heat_views: Array[Node3D] = [research_center, barracks, heat_depot, heat_turret]
	if not await _wait_until(_all_overlaid.bind(heat_views, true), 10.0,
			"cold overlay never reached the completed demand-bearing views"):
		return
	var heat_bunkhouse_anchor := SmokeHelpers.ring_anchor(4, "Bunkhouse", research_center)
	if heat_bunkhouse_anchor.x < 0:
		_fail("no valid heat-leg Bunkhouse anchor near the Research Center")
		return
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/BunkhouseButton") as Button, heat_bunkhouse_anchor)
	var heat_bunkhouse := await _wait_building("Bunkhouse", 25.0)
	if heat_bunkhouse == null:
		return
	var heat_bunkhouse_id := int(heat_bunkhouse.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(heat_bunkhouse_id), 30.0, "heat-leg Bunkhouse never completed"):
		return
	if not await _wait_until(
			_heat_stage_ok.bind(15, 17, 15.0 / 17.0, "15 / 17 · surface 88%"),
			10.0, "cold stage after the Bunkhouse completion never read 15/17 at 88%"):
		return
	heat_views.append(heat_bunkhouse)
	if not await _wait_until(_all_overlaid.bind(heat_views, true), 10.0,
			"cold overlay never reached the completed Bunkhouse"):
		return
	# The zero-demand defenses standing through this deficit never take the
	# cold overlay — and the 15/17 stage above already proved their presence
	# never moves the heat ledger.
	if not _overlay_is(heat_wall, false):
		_fail("the zero-demand Wall took the cold overlay")
		return
	if not _overlay_is(heat_gate, false):
		_fail("the zero-demand Gate took the cold overlay")
		return

	# The fueled Coal Furnace (+12 supply) restores 27/17 — clamped back to
	# a full 100% — and every cold view must drop the icy overlay entirely.
	# The Furnace itself demands no heat, so it never overlays. Then the burn: the
	# completed Furnace drains whole Coal from the Team-1 stockpile while
	# Playing, so the stockpile must visibly decrease.
	var furnace_anchor := SmokeHelpers.ring_anchor(4, "CoalFurnace", research_center)
	if furnace_anchor.x < 0:
		_fail("no valid heat-leg Coal Furnace anchor near the Research Center")
		return
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/CoalFurnaceButton") as Button, furnace_anchor)
	var heat_furnace := await _wait_building("CoalFurnace", 25.0)
	if heat_furnace == null:
		return
	var furnace_id := int(heat_furnace.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(furnace_id), 30.0, "heat-leg Coal Furnace never completed"):
		return
	if not await _wait_until(
			_heat_stage_ok.bind(27, 17, 1.0, "27 / 17 · surface 100%"),
			10.0, "warm stage after the Coal Furnace never read 27/17 at 100%"):
		return
	if not await _wait_until(_all_overlaid.bind(heat_views, false), 10.0,
			"cold overlay never cleared after the Coal Furnace completed"):
		return
	if not _overlay_is(heat_furnace, false):
		_fail("the Coal Furnace (zero heat demand) took the cold overlay")
		return
	var coal_before_burn := int(GrusBridge.economy_snapshot().get("coal", -1))
	if not await _wait_until(
			func(): return int(GrusBridge.economy_snapshot().get("coal", -1)) < coal_before_burn,
			15.0, "fueled Coal Furnace never decreased the Coal stockpile"):
		return

	# Every live view placed this leg — RC, Barracks, Depot, Turret,
	# Bunkhouse, Wall, Gate, Furnace — renders its finished kind GLB scaled
	# and grounded by the catalogue footprint, whatever the heat stage did.
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		var kind := str(view.get_meta("building_kind", ""))
		var model := view.get_node_or_null("Model") as Node3D
		if model == null or not model.scene_file_path.ends_with("%s.glb" % kind.to_snake_case()):
			_fail("completed %s does not render its catalogue GLB: %s" % [
				kind, model.scene_file_path if model != null else "missing"])
			return
		var art_error := SmokeHelpers.view_art_error(view)
		if not art_error.is_empty():
			_fail("completed %s fails the catalogue geometry contract: %s" % [kind, art_error])
			return

	if not await _park_at_staging(4):
		return
	await _select_building(barracks)
	if not await _wait_until(func(): return filter_guard_button.visible and filter_rifleman_button.visible,
			5.0, "Barracks selection did not show its Guard/Rifleman production row"):
		return
	# HPA-698 modal Rally: the tile arms rally, the next real right-click
	# dispatches set_rally and consumes the order once.
	var barracks_main := get_node("Main") as Node3D
	var barracks_status := barracks_main.get_node("HUD/CommandStatus") as Label
	var rally_tile := barracks_main.get_node("HUD/CommandPanel/OrderGrid/RallyButton") as Button
	await _click_button(rally_tile)
	if str(barracks_main.get("_armed_order")) != "rally" or not rally_tile.button_pressed:
		_fail("Rally tile did not arm the modal rally on the selected Barracks")
		return
	var rally_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), _camera.unproject_position(
		Vector3(RALLY_CELL.x + 0.25, 0.0, RALLY_CELL.y + 0.25)), MOUSE_BUTTON_RIGHT)
	if str(barracks_status.text) != "Rally point queued" \
			or str(barracks_main.get("_armed_order")) != "":
		_fail("armed Rally did not consume into set_rally: %s" % barracks_status.text)
		return
	if not await _wait_until(_rally_stored.bind(barracks_id), 10.0,
			"armed Rally did not store the Barracks rally point"):
		return
	if not await _wait_until(func(): return int(GrusBridge.command_feedback_revision()) > rally_revision,
			10.0, "armed Rally produced no sim verdict"):
		return
	var food_before := int(GrusBridge.economy_snapshot().get("food", -1))
	var scrap_before := int(GrusBridge.economy_snapshot().get("scrap", -1))
	await _click_button(get_node("Main/HUD/CommandPanel/TrainGrid/GuardButton") as Button)
	await _click_button(get_node("Main/HUD/CommandPanel/TrainGrid/RiflemanButton") as Button)
	if not await _wait_until(func(): return _queue_label(barracks_id) == "Guard", 10.0,
			"Guard never entered the Barracks queue"):
		return
	if not await _wait_until(func(): return _queue_progress(barracks_id) > 0.0, 10.0,
			"Guard queue progress never started"):
		return
	var mid_queue := _queue_progress(barracks_id)
	if mid_queue < 1.0 and not await _wait_until(func(): return _queue_progress(barracks_id) > mid_queue, 10.0,
			"Guard queue progress never advanced"):
		return
	# Both jobs charged once at enqueue: 60f Guard + 40f/40w Rifleman (the
	# second click lands on the next command tick, so poll briefly). The
	# ordered training_queue must carry both labels before the 20x head
	# Guard can finish.
	if not await _wait_until(
			func():
				var charged: Dictionary = GrusBridge.economy_snapshot()
				return _training_queue(barracks_id) == ["Guard", "Rifleman"] \
					and int(charged.get("food", -1)) == food_before - 100 \
					and int(charged.get("scrap", -1)) == scrap_before - 40,
			10.0, "Barracks enqueue did not charge Guard+Rifleman once into a FIFO training_queue"):
		return
	if _training_queue(barracks_id) != ["Guard", "Rifleman"]:
		_fail("Barracks training_queue lost FIFO order while Guard is the head: %s" % [
			_training_queue(barracks_id)])
		return
	# The Barracks is a Guard/Rifleman producer: the Worker tile is hidden and
	# the two FIFO jobs fill the first two of the five fixed sockets.
	if not await _wait_until(func():
			return worker_button.visible == false and filter_guard_button.visible \
				and filter_rifleman_button.visible,
			5.0, "Barracks selection did not filter train tiles to Guard/Rifleman"):
		return
	if not await _wait_until(func():
			return (sockets.get_child(0).get_node("Icon") as TextureRect).texture != null \
				and (sockets.get_child(0).get_node("Icon") as TextureRect).texture.resource_path \
					== "res://assets/ui/icons/units/guard.png" \
				and (sockets.get_child(1).get_node("Icon") as TextureRect).texture != null \
				and (sockets.get_child(1).get_node("Icon") as TextureRect).texture.resource_path \
					== "res://assets/ui/icons/units/rifleman.png" \
				and (sockets.get_child(2).get_node("Icon") as TextureRect).texture == null \
				and not overflow_chip.visible,
			5.0, "Barracks FIFO sockets do not render Guard then Rifleman with empty overflow"):
		return
	queue_layout_error = _queue_layout_error(sockets, "Barracks FIFO")
	if not queue_layout_error.is_empty():
		_fail(queue_layout_error)
		return
	painted_error = _queue_painted_treatment_error(sockets, 2, "Barracks FIFO")
	if not painted_error.is_empty():
		_fail(painted_error)
		return
	capture_error = await SmokeHelpers.capture_frame(get_tree(), "economy-queue-barracks")
	if not capture_error.is_empty():
		_fail(capture_error)
		return
	var guard_seconds := float(catalogue_units.get("Guard", {}).get("seconds", 0))
	var head_time_text := (sockets.get_child(0).get_node("Time") as Label).text
	var head_time_value := head_time_text.trim_suffix("s")
	if not head_time_value.is_valid_int() \
			or int(head_time_value) < 0 or int(head_time_value) > int(ceilf(guard_seconds)):
		_fail("Guard head socket shows %s, outside the catalogue %ds train time" % [
			head_time_text, int(guard_seconds)])
		return
	if not await _wait_until(func():
			var progress_bar := sockets.get_child(0).get_node("Progress") as ProgressBar
			return progress_bar.visible \
				and absf(progress_bar.value - _queue_progress(barracks_id) * 100.0) <= 12.0,
			5.0, "Guard head socket progress does not track the live queue_progress"):
		return
	var guard_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Guard", 5, guard_box), 30.0,
			"trained Guard view never appeared"):
		return
	if not await _wait_until(func(): return _queue_label(barracks_id) == "Rifleman", 15.0,
			"Rifleman never became the FIFO head after the Guard spawned"):
		return
	if _training_queue(barracks_id) != ["Rifleman"]:
		_fail("Barracks training_queue did not transition to the Rifleman head: %s" % [
			_training_queue(barracks_id)])
		return
	if not await _wait_until(func():
			return (sockets.get_child(0).get_node("Icon") as TextureRect).texture != null \
				and (sockets.get_child(0).get_node("Icon") as TextureRect).texture.resource_path \
					== "res://assets/ui/icons/units/rifleman.png" \
				and (sockets.get_child(1).get_node("Icon") as TextureRect).texture == null,
			5.0, "queue sockets did not shift to the Rifleman FIFO head"):
		return
	var rifleman_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Rifleman", 5, rifleman_box), 35.0,
			"trained Rifleman view never appeared"):
		return
	if not await _wait_until(func(): return _queue_label(barracks_id) == "", 15.0,
			"Rifleman job never left the Barracks queue"):
		return
	if not _training_queue(barracks_id).is_empty():
		_fail("Barracks training_queue did not empty after the Rifleman spawned: %s" % [
			_training_queue(barracks_id)])
		return
	# Park both combatants at staging so their live views cannot intercept the
	# Barracks-origin click below. Stable ids, never the captured view.
	for unit_box in [guard_box, rifleman_box]:
		if not await _park_at_staging(int(unit_box.get("id", -1))):
			return

	# Step 8 recap: construction ramp (Bunkhouse), queue ramps (Guard/Rifleman),
	# the Occupied reject code, and the blocked fields were all asserted at
	# their stages above; finish with the final economy state and unblocked
	# producers.
	var final: Dictionary = GrusBridge.economy_snapshot()
	if int(final.get("population_cap", -1)) != 20 or int(final.get("population_used", -1)) != 6:
		_fail("military production leg did not finish at population 6 of 20: %s" % [final])
		return
	for producer_id in [research_center_id, barracks_id]:
		if _blocked_reason(producer_id) != 0:
			_fail("producer %d ended with a blocked queue" % producer_id)
			return

	# Step 9: the same UI train path that accepted solvent enqueues now
	# runs the Food stockpile dry. Every gatherer is retired, so each real
	# GuardButton click charges exactly the catalogue Guard cost until the
	# stockpile cannot pay; an identical click on the drained-empty queue
	# must then reject InsufficientResources and leave stockpiles, the
	# queue and the building census untouched.
	await _select_building(barracks)
	var main := get_node("Main") as Node3D
	var selected_after_pick := int(main.get("selected_building_id"))
	if selected_after_pick != barracks_id:
		var screen := _camera.unproject_position(barracks.global_position)
		var unit_pick = main.call("_nearest_view", "unit_views", screen, 1) as Node3D
		var building_pick = main.call("_building_at", screen, 1) as Node3D
		var selected_unit := _unit_view(6)
		var session_panel := main.get_node("HUD/SessionPanel") as Control
		_fail("Barracks click did not select its producer: wanted=%d selected=%d unit_pick=%s building_pick=%s screen=%s barracks=%s visible=%s panel=%s phase=%s placement=%s left_pressed=%s last_press=%s selected_units=%s selected_unit_screen=%s" % [
			barracks_id, selected_after_pick,
			str(unit_pick.get_meta("unit_id", -1)) if unit_pick != null else "none",
			str(building_pick.get_meta("building_id", -1)) if building_pick != null else "none",
			screen, barracks.global_position, barracks.is_visible_in_tree(), session_panel.visible,
			str(main.get("_session_phase")), str(main.get("_placement_kind")),
			str(main.get("_left_pressed")), str(main.get("_left_press_position")),
			str(main.get("selected_ids")),
			_camera.unproject_position(selected_unit.global_position) if selected_unit != null else "missing"])
		return
	if not await _wait_until(func(): return filter_guard_button.visible,
			5.0, "reselecting the Barracks did not restore its Guard tile"):
		return
	var guard_button := get_node("Main/HUD/CommandPanel/TrainGrid/GuardButton") as Button
	var insufficient := false
	# The bound comes from live Food: each accepted click charges
	# guard_food, so food/guard_food + 1 clicks always reach the reject.
	var guard_food := int(guard_entry.get("food", 60))
	var attempts := int(GrusBridge.economy_snapshot().get("food", 0)) / guard_food + 1
	for _attempt in attempts:
		var revision: int = GrusBridge.command_feedback_revision()
		var before_click := {
			"food": int(GrusBridge.economy_snapshot().get("food", -1)),
			"scrap": int(GrusBridge.economy_snapshot().get("scrap", -1)),
			"coal": int(GrusBridge.economy_snapshot().get("coal", -1)),
			"queue": _queue_label(barracks_id),
			"blocked": _blocked_reason(barracks_id),
			"buildings": get_tree().get_nodes_in_group("building_views").size(),
		}
		await _click_button(guard_button)
		if not await _wait_until(
				func(): return GrusBridge.command_feedback_revision() != revision,
				10.0, "Guard enqueue command was never processed"):
			return
		var after_click: Dictionary = GrusBridge.economy_snapshot()
		var reject_code := int(after_click.get("last_reject_code", -1))
		if reject_code == -1:
			# A solvent click is accepted and charged exactly once.
			if int(after_click.get("food", -1)) != before_click["food"] - guard_food:
				var selected_id := int(get_node("Main").get("selected_building_id"))
				var selected: Dictionary = GrusBridge.building_snapshot(selected_id)
				_fail("accepted Guard enqueue did not charge exactly %d food: %s -> %s; selected=%d kind=%s status=%s feedback=%s phase=%s wave=%s" % [
					guard_food, before_click, after_click, selected_id, str(selected.get("kind", "")),
					str((get_node("Main/HUD/CommandStatus") as Label).text), GrusBridge.command_feedback(),
					str(GrusBridge.session_snapshot()), GrusBridge.wave_snapshot()])
				return
			continue
		if reject_code != REJECT_INSUFFICIENT:
			_fail("Guard enqueue rejected with unexpected code %d (expected %d)" % [reject_code, REJECT_INSUFFICIENT])
			return
		# The rejected click changed nothing: identical stockpiles, queue
		# and building census — the enqueue was refused before any state
		# was touched.
		# A rejected enqueue can never add Coal, so only an increase fails:
		# the Furnace's live burn may freely drop Coal while the
		# click-to-feedback window spans any number of ticks. Food/Scrap
		# must match exactly (every gatherer is retired by now).
		if int(after_click.get("food", -1)) != before_click["food"] \
				or int(after_click.get("scrap", -1)) != before_click["scrap"] \
				or int(after_click.get("coal", -1)) > before_click["coal"]:
			_fail("rejected Guard enqueue still moved the stockpile")
			return
		if _queue_label(barracks_id) != before_click["queue"] \
				or _blocked_reason(barracks_id) != before_click["blocked"]:
			_fail("rejected Guard enqueue still touched the Barracks queue")
			return
		if get_tree().get_nodes_in_group("building_views").size() != before_click["buildings"]:
			_fail("rejected Guard enqueue still changed the building census")
			return
		insufficient = true
		break
	if not insufficient:
		_fail("%d Guard enqueue(s) never hit InsufficientResources" % attempts)
		return

	# The accepted drain jobs must leave the FIFO before the unaffordable
	# click re-tests it: an empty head label, zero progress and an empty
	# training_queue together are the authoritative "no jobs" proof.
	if not await _wait_until(func(): return _queue_label(barracks_id) == "" \
			and _queue_progress(barracks_id) == 0.0 \
			and _training_queue(barracks_id).is_empty(), 60.0,
			"accepted drain jobs never emptied the Barracks queue"):
		return
	var reject_revision: int = GrusBridge.command_feedback_revision()
	var unaffordable_before := {
		"food": int(GrusBridge.economy_snapshot().get("food", -1)),
		"scrap": int(GrusBridge.economy_snapshot().get("scrap", -1)),
		"coal": int(GrusBridge.economy_snapshot().get("coal", -1)),
		"blocked": _blocked_reason(barracks_id),
		"buildings": get_tree().get_nodes_in_group("building_views").size(),
	}
	await _click_button(guard_button)
	if not await _wait_until(
			func(): return GrusBridge.command_feedback_revision() != reject_revision,
			10.0, "unaffordable Guard enqueue was never processed"):
		return
	var unaffordable_after: Dictionary = GrusBridge.economy_snapshot()
	if int(unaffordable_after.get("last_reject_code", -1)) != REJECT_INSUFFICIENT:
		_fail("unaffordable Guard enqueue did not reject InsufficientResources: %s" % unaffordable_after)
		return
	# A rejected enqueue can never add Coal; the Furnace's live burn, never
	# the rejected enqueue, is free to decrement Coal across the window.
	if int(unaffordable_after.get("food", -1)) != unaffordable_before["food"] \
			or int(unaffordable_after.get("scrap", -1)) != unaffordable_before["scrap"] \
			or int(unaffordable_after.get("coal", -1)) > unaffordable_before["coal"]:
		_fail("rejected Guard enqueue still moved the stockpile")
		return
	if _queue_label(barracks_id) != "" or _queue_progress(barracks_id) != 0.0 \
			or not _training_queue(barracks_id).is_empty():
		_fail("rejected Guard enqueue still left a Barracks job: %s / %s" % [
			_queue_label(barracks_id), _training_queue(barracks_id)])
		return
	if _blocked_reason(barracks_id) != unaffordable_before["blocked"]:
		_fail("rejected Guard enqueue changed the Barracks blocked state")
		return
	if get_tree().get_nodes_in_group("building_views").size() != unaffordable_before["buildings"]:
		_fail("rejected Guard enqueue still changed the building census")
		return
	if int(GrusBridge.economy_snapshot().get("idle_workers", 0)) < 1:
		_fail("no idle workers before idle navigation")
		return
	await _click_button(get_node("Main/HUD/EconomyPanel/IdleButton") as Button)
	if not await _wait_until(_single_ringed_idle_worker, 10.0,
			"idle navigation did not select an idle_worker_ids member"):
		return

	wave_error = SmokeHelpers.no_launched_wave_error("economy smoke")
	if not wave_error.is_empty():
		_fail(wave_error)
		return
	if not GrusBridge.set_sim_speed(1.0):
		_fail("failed to restore Bevy virtual time")
		return

	var end: Dictionary = GrusBridge.economy_snapshot()
	print("GRUS_ECONOMY_SMOKE_OK bunkhouse_model=site_to_glb overlays=cold_warm_recovery zero_demand=wall_gate_furnace bounds=catalogue_grounded food=%d scrap=%d coal=%d cap=%d units=%d buildings=%d unaffordable_reject=%d elapsed_ms=%d" % [
		int(end.get("food", 0)),
		int(end.get("scrap", 0)),
		int(end.get("coal", 0)),
		int(end.get("population_cap", 0)),
		get_tree().get_nodes_in_group("unit_views").size(),
		get_tree().get_nodes_in_group("building_views").size(),
		REJECT_INSUFFICIENT,
		Time.get_ticks_msec() - _start_ms,
	])
	get_tree().quit(0)
