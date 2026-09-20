extends Node

## HPA-471 end-to-end economy smoke. Drives the real controller/UI paths —
## selection, contextual right-clicks, build buttons with the placement
## preview, production buttons, rally points, and idle-worker navigation —
## through a solvent script that stockpiles, builds, trains one of each
## combat unit, and reaches Age 2. GrusBridge snapshots are used for
## assertions only; every gameplay write rides the UI.

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## RejectReason discriminant values (declaration order in
## crates/grus-sim/src/commands.rs); the bridge reports "no rejection" as -1.
const REJECT_NONE := -1
const REJECT_LOCKED := 7
const REJECT_FARM_OCCUPIED := 12

## Authored fixture source ids (crates/grus-sim/src/fixture.rs).
const BERRY_ID := 1
const TREE_ID := 3
const GOLD_ID := 6
## Northeast expansion tree used for the Storehouse delivery proof.
const FAR_TREE_ID := 14

## Solvency targets before any spending: 615 total Wood spend against 300
## starting Wood, 480 Food for Spearman+Archer+Age2+Cavalry plus 50 for the
## rally-proof Villager, and 260 Gold for Age2+Cavalry.
const TARGET_WOOD := 615
const TARGET_FOOD := 530
const TARGET_GOLD := 260

const HOUSE_ANCHOR := Vector2i(9, 44)
const STOREHOUSE_ANCHOR := Vector2i(38, 22)
const FARM_ANCHOR := Vector2i(18, 52)
const BARRACKS_ANCHOR := Vector2i(17, 41)
const RANGE_ANCHOR := Vector2i(9, 52)
const STABLE_ANCHOR := Vector2i(8, 41)

## Open ground west of the base; villagers parked here stay well outside the
## 20 px unit-priority click radius of every building origin.
const STAGING_CELL := Vector2i(5, 47)
## Town Center rally cell for the rally-point proof.
const RALLY_CELL := Vector2i(20, 58)

var _camera: Camera3D
var _start_ms := 0
var _building_ids := {}

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	# Best-effort restore on failure exits too; the process quits either way.
	GrusBridge.set_sim_speed(1.0)
	get_tree().quit(1)

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

func _click_view(view: Node3D, button: int) -> void:
	_mouse_click(_camera.unproject_position(view.global_position), button)
	await get_tree().process_frame
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
func _wait_until(condition: Callable, timeout_s: float, message: String) -> bool:
	var frame_budget := int(ceilf(timeout_s * Engine.physics_ticks_per_second))
	for _frame in frame_budget:
		if condition.call():
			return true
		await get_tree().physics_frame
	_fail("%s (waited %.0fs)" % [message, timeout_s])
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

func _blocked_reason(id: int) -> int:
	return int(GrusBridge.building_snapshot(id).get("blocked_reason", -1))

func _fixture_views_settled() -> bool:
	var units := get_tree().get_nodes_in_group("unit_views")
	var buildings := get_tree().get_nodes_in_group("building_views")
	var resources := get_tree().get_nodes_in_group("resource_views")
	if units.size() != 8 or buildings.size() != 2 or resources.size() != 18:
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

func _stockpiles_at_least(wood: int, food: int, gold: int) -> bool:
	var state: Dictionary = GrusBridge.economy_snapshot()
	return int(state.get("wood", -1)) >= wood \
		and int(state.get("food", -1)) >= food \
		and int(state.get("gold", -1)) >= gold

func _building_complete_at(id: int) -> bool:
	return bool(GrusBridge.building_snapshot(id).get("complete", false))

## Delivery proof for the Storehouse: a Wood stockpile credit that lands while
## the worker stands near the Storehouse. Its far-tree gather slots are
## >= 4 cells from the Storehouse origin and the Town Center >= 28, so only a
## real Storehouse deposit can satisfy this within one physics frame of drift.
func _wood_deposited_at_storehouse(worker: Node3D, storehouse: Vector3, box: Dictionary) -> bool:
	var wood := int(GrusBridge.economy_snapshot().get("wood", -1))
	var increased := wood > int(box.get("wood", wood))
	box["wood"] = wood
	return increased and worker.global_position.distance_to(storehouse) < 12.0

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
			return true
	return false

func _units_near(point: Vector3, radius: float) -> int:
	var count := 0
	for node in get_tree().get_nodes_in_group("unit_views"):
		if int(node.get_meta("team_id", -1)) != 1:
			continue
		if (node as Node3D).global_position.distance_to(point) <= radius:
			count += 1
	return count

func _area_clear_of_units(point: Vector3, radius: float) -> bool:
	return _units_near(point, radius) == 0

## Waits until no friendly unit sits inside the unit-priority click radius of
## a building origin, so the next left-click selects the building.
func _wait_units_clear(point: Vector3, radius: float, timeout_s: float) -> bool:
	var frame_budget := int(ceilf(timeout_s * Engine.physics_ticks_per_second))
	for _frame in frame_budget:
		if _area_clear_of_units(point, radius):
			return true
		await get_tree().physics_frame
	for node in get_tree().get_nodes_in_group("unit_views"):
		print("DEBUG unit %d kind=%s team=%d pos=%s" % [
			int(node.get_meta("unit_id", -1)), str(node.get_meta("unit_kind", "")),
			int(node.get_meta("team_id", -1)), (node as Node3D).global_position])
	_fail("units never cleared the click zone at %s" % point)
	return false

func _select_building(view: Node3D) -> void:
	if not await _wait_units_clear(view.global_position, 2.2, 15.0):
		return
	await _click_view(view, MOUSE_BUTTON_LEFT)

## Selects a unit and sends it to the staging ground, waiting until it left
## the hazard radius (unit-priority selection would otherwise steal a click
## on the hazard).
func _move_unit_away(view: Node3D, hazard: Vector3, min_dist: float) -> void:
	await _click_view(view, MOUSE_BUTTON_LEFT)
	await _click_world(Vector2(STAGING_CELL), MOUSE_BUTTON_RIGHT)
	if not await _wait_until(
			func(): return view.global_position.distance_to(hazard) >= min_dist,
			15.0, "unit never left the hazard zone"):
		return

## Retires a gatherer through the UI (select + Stop). Finite sources deplete
## at 20x virtual time, and a depleted source leaves the worker idled at its
## drop-off slot — sometimes inside a building click zone — so the scenario
## stops its gatherers once the banked thresholds are met.
func _stop_unit(view: Node3D) -> void:
	await _click_view(view, MOUSE_BUTTON_LEFT)
	_key_tap(KEY_S)
	await get_tree().process_frame

## Stops a unit and parks it on the staging ground, clear of every later
## building-origin click.
func _park_at_staging(view: Node3D) -> void:
	await _stop_unit(view)
	await _click_view(view, MOUSE_BUTTON_LEFT)
	await _click_world(Vector2(STAGING_CELL), MOUSE_BUTTON_RIGHT)
	var staging := Vector3(STAGING_CELL.x + 0.5, 0.0, STAGING_CELL.y + 0.5)
	if not await _wait_until(
			func(): return view.global_position.distance_to(staging) < 3.0,
			15.0, "retired gatherer never reached the staging ground"):
		return

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
			"skirmish startup never settled to 8 units / 2 buildings / 18 resources"):
		return

	var boot: Dictionary = GrusBridge.economy_snapshot()
	if int(boot.get("food", -1)) != 200 or int(boot.get("wood", -1)) != 300 or int(boot.get("gold", -1)) != 100:
		_fail("starting stockpile is not 200/300/100: %s" % [boot])
		return
	# The catalogue snapshot is the only gameplay-data source for the UI:
	# prove it carries real cost/producer/unlock entries from the Rust table.
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var catalogue_units: Dictionary = catalogue.get("units", {})
	var villager_entry: Dictionary = catalogue_units.get("Villager", {})
	if int(villager_entry.get("food", -1)) != 50 or str(villager_entry.get("producer", "")) != "TownCenter":
		_fail("catalogue snapshot is missing Villager cost/producer: %s" % [catalogue])
		return
	var stable_entry: Dictionary = catalogue_units.get("Cavalry", {})
	if int(stable_entry.get("age", -1)) != 2 or str(stable_entry.get("producer", "")) != "Stable":
		_fail("catalogue snapshot is missing the Cavalry age gate/producer: %s" % [catalogue])
		return
	var age_up_entry: Dictionary = catalogue.get("age_up", {})
	if int(age_up_entry.get("food", -1)) != 300 or int(age_up_entry.get("gold", -1)) != 200:
		_fail("catalogue snapshot is missing the Age-2 cost: %s" % [catalogue])
		return
	if absf(float(boot.get("gather_rate", 0.0)) - 2.0) > 0.0001:
		_fail("starting gather rate is not 2.0: %s" % [boot])
		return
	if int(boot.get("population_used", -1)) != 4 or int(boot.get("population_cap", -1)) != 10:
		_fail("starting population is not 4/10: %s" % [boot])
		return
	if int(boot.get("age", -1)) != 1:
		_fail("starting age is not 1: %s" % [boot])
		return
	var town_center := _building_view(1)
	if town_center == null:
		_fail("Town Center view 1 is missing")
		return
	var town_center_id := int(town_center.get_meta("building_id"))
	_building_ids = {1: true, 2: true}
	var builder := _unit_view(4)
	if builder == null:
		_fail("builder villager 4 is missing")
		return

	# Normal skirmish boots into Start; this scenario starts the match before
	# its first gameplay command. Scripted choreography runs AI-free: the
	# Start-only test seam must lift the Team-2 controller first.
	if not GrusBridge.disable_ai_for_test():
		_fail("disable_ai_for_test did not lift the AI from Start")
		return
	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return

	# Step 1: villagers onto trees/berries/gold via right-click; stockpiles
	# move only after a deposit, never on the command itself.
	for job in [[1, TREE_ID], [2, BERRY_ID], [3, GOLD_ID]]:
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
	if int(after.get("wood", -1)) != int(before.get("wood", -1)) \
			or int(after.get("food", -1)) != int(before.get("food", -1)) \
			or int(after.get("gold", -1)) != int(before.get("gold", -1)):
		_fail("stockpile moved before any worker could gather and deposit: %s -> %s" % [before, after])
		return

	# Steps 2-3: solvent gather targets (deposit-driven) before any spending.
	if not await _wait_until(_stockpiles_at_least.bind(TARGET_WOOD, TARGET_FOOD, TARGET_GOLD), 150.0,
			"gatherers never stockpiled %d wood / %d food / %d gold" % [TARGET_WOOD, TARGET_FOOD, TARGET_GOLD]):
		return

	# Step 4: House raises the population cap 10 -> 20; its construction
	# progress ramps through snapshots (Step 13 evidence).
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/HouseButton") as Button, HOUSE_ANCHOR)
	var house := await _wait_building("House", 20.0)
	if house == null:
		return
	var house_id := int(house.get_meta("building_id"))
	if not await _wait_until(func(): return _construction_progress(house_id) > 0.0, 20.0,
			"House construction progress never started"):
		return
	var mid_progress := 0.0
	if not bool(GrusBridge.building_snapshot(house_id).get("complete", false)):
		mid_progress = _construction_progress(house_id)
	if not await _wait_until(func(): return _construction_progress(house_id) > mid_progress, 20.0,
			"House construction progress never advanced"):
		return
	if not await _wait_until(_building_complete_at.bind(house_id), 30.0, "House never completed"):
		return
	if _construction_progress(house_id) < 1.0:
		_fail("completed House construction progress is not 1.0")
		return
	if int(GrusBridge.economy_snapshot().get("population_cap", -1)) != 20:
		_fail("completed House did not raise the population cap to 20")
		return

	# Fog retarget (HPA-473): the Storehouse anchor (38,22) and the northeast
	# expansion tree (43,18) boot Unexplored — placement validates explored
	# footprints and gather requires explored sources, so scout the corridor
	# first. The expansion tree's view becoming visible is the explored
	# proof (views are the presentation projection of the sim fog).
	await _click_view(builder, MOUSE_BUTTON_LEFT)
	await _click_world(Vector2(38, 24), MOUSE_BUTTON_RIGHT)
	if not await _wait_until(
			func(): return _resource_view(FAR_TREE_ID) != null \
					and (_resource_view(FAR_TREE_ID) as Node3D).is_visible_in_tree(),
			60.0, "scout never revealed the far expansion tree"):
		return

	# Step 5: Storehouse delivery proof. Retask villager 1 to the northeast
	# expansion tree (~6 cells from the Storehouse, ~40 from the Town Center):
	# every later Wood deposit must land at the Storehouse.
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/StorehouseButton") as Button, STOREHOUSE_ANCHOR)
	var storehouse := await _wait_building("Storehouse", 25.0)
	if storehouse == null:
		return
	var storehouse_id := int(storehouse.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(storehouse_id), 30.0, "Storehouse never completed"):
		return
	var wood_worker := _unit_view(1)
	var far_tree := _resource_view(FAR_TREE_ID)
	if wood_worker == null or far_tree == null:
		_fail("wood worker or expansion tree view is missing")
		return
	await _click_view(wood_worker, MOUSE_BUTTON_LEFT)
	await _click_view(far_tree, MOUSE_BUTTON_RIGHT)
	var delivery_box := {"wood": int(GrusBridge.economy_snapshot().get("wood", -1))}
	if not await _wait_until(_wood_deposited_at_storehouse.bind(wood_worker, storehouse.global_position, delivery_box), 45.0,
			"no wood delivery ever reached the Storehouse"):
		return
	# Retire the wood gatherer: its northeast corridor stays clear of every
	# later click target, so parking in place is safe.
	await _stop_unit(wood_worker)

	# Step 6: the completed Farm serves exactly one worker; the second
	# assignment rejects with the numeric FarmOccupied code.
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/FarmButton") as Button, FARM_ANCHOR)
	var farm := await _wait_building("Farm", 20.0)
	if farm == null:
		return
	var farm_id := int(farm.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(farm_id), 30.0, "Farm never completed"):
		return
	if not await _wait_until(func(): return int(farm.get_meta("resource_id", -1)) > 0, 10.0,
			"completed Farm never gained its resource metadata"):
		return
	var food_worker := _unit_view(2)
	var gold_worker := _unit_view(3)
	if food_worker == null or gold_worker == null:
		_fail("villagers 2/3 views are missing")
		return
	await _click_view(food_worker, MOUSE_BUTTON_LEFT)
	await _click_view(farm, MOUSE_BUTTON_RIGHT)
	await _click_view(gold_worker, MOUSE_BUTTON_LEFT)
	await _click_view(farm, MOUSE_BUTTON_RIGHT)
	if not await _wait_until(
			func(): return int(GrusBridge.economy_snapshot().get("last_reject_code", -1)) == REJECT_FARM_OCCUPIED,
			10.0, "second farm worker did not reject with FarmOccupied (%d)" % REJECT_FARM_OCCUPIED):
		return
	# Retire the food and gold gatherers: the finite gold patch depletes at
	# 20x virtual time and would otherwise idle a worker inside the Town
	# Center click zone. Park both on the staging ground.
	await _park_at_staging(food_worker)
	await _park_at_staging(gold_worker)

	# Step 7: Barracks trains a Spearman; queue label/progress via snapshots.
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/BarracksButton") as Button, BARRACKS_ANCHOR)
	var barracks := await _wait_building("Barracks", 25.0)
	if barracks == null:
		return
	var barracks_id := int(barracks.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(barracks_id), 40.0, "Barracks never completed"):
		return
	if _blocked_reason(barracks_id) != 0 or _queue_label(barracks_id) != "":
		_fail("fresh Barracks queue is not idle and unblocked: blocked=%d label=%s" % [_blocked_reason(barracks_id), _queue_label(barracks_id)])
		return
	await _move_unit_away(builder, barracks.global_position, 2.5)
	await _select_building(barracks)
	await _click_button(get_node("Main/HUD/CommandPanel/TrainGrid/SpearmanButton") as Button)
	if not await _wait_until(func(): return _queue_label(barracks_id) == "Spearman", 10.0,
			"Spearman never entered the Barracks queue"):
		return
	if not await _wait_until(func(): return _queue_progress(barracks_id) > 0.0, 10.0,
			"Spearman queue progress never started"):
		return
	var mid_queue := _queue_progress(barracks_id)
	if mid_queue < 1.0 and not await _wait_until(func(): return _queue_progress(barracks_id) > mid_queue, 10.0,
			"Spearman queue progress never advanced"):
		return
	var spearman_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Spearman", 9, spearman_box), 30.0,
			"trained Spearman view never appeared"):
		return
	if not await _wait_until(func(): return _queue_label(barracks_id) == "", 15.0,
			"Spearman job never left the Barracks queue"):
		return

	# Step 8: Archery Range trains an Archer.
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/ArcheryRangeButton") as Button, RANGE_ANCHOR)
	var range_building := await _wait_building("ArcheryRange", 25.0)
	if range_building == null:
		return
	var range_id := int(range_building.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(range_id), 40.0, "Archery Range never completed"):
		return
	if _blocked_reason(range_id) != 0:
		_fail("fresh Archery Range reported a blocked queue")
		return
	await _move_unit_away(builder, range_building.global_position, 2.5)
	await _select_building(range_building)
	await _click_button(get_node("Main/HUD/CommandPanel/TrainGrid/ArcherButton") as Button)
	if not await _wait_until(func(): return _queue_label(range_id) == "Archer", 10.0,
			"Archer never entered the Archery Range queue"):
		return
	var archer_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Archer", 9, archer_box), 30.0,
			"trained Archer view never appeared"):
		return

	# Step 9: Age 2. The read-only placement preview reports Locked before the
	# age flip and valid after; the catalogue maps Age 2 to the 2.2/s gather
	# rate, which the unlock flip exercises end to end.
	var locked_preview: Dictionary = GrusBridge.placement_preview(4, "Stable", STABLE_ANCHOR.x, STABLE_ANCHOR.y)
	if int(locked_preview.get("reject_code", -1)) != REJECT_LOCKED:
		_fail("Stable placement before Age 2 did not report Locked (%d): %s" % [REJECT_LOCKED, locked_preview])
		return
	await _select_building(town_center)
	var age_button := get_node("Main/HUD/CommandPanel/TrainGrid/AgeButton") as Button
	if not await _wait_until(func(): return not age_button.disabled, 10.0,
			"Age button never enabled for the selected Town Center"):
		return
	await _click_button(age_button)
	if not await _wait_until(func(): return _queue_label(town_center_id) == "Age2", 10.0,
			"Age 2 research never entered the Town Center queue"):
		return
	if not await _wait_until(func(): return int(GrusBridge.economy_snapshot().get("age", 1)) == 2, 30.0,
			"Age 2 never completed"):
		return
	# The Age-2 2.2/s gather rate, observed directly through the snapshot.
	var age_two_rate := float(GrusBridge.economy_snapshot().get("gather_rate", 0.0))
	if absf(age_two_rate - 2.2) > 0.0001:
		_fail("Age 2 gather rate is not 2.2: %f" % age_two_rate)
		return
	var unlocked_preview: Dictionary = GrusBridge.placement_preview(4, "Stable", STABLE_ANCHOR.x, STABLE_ANCHOR.y)
	if not bool(unlocked_preview.get("valid", false)) or int(unlocked_preview.get("reject_code", 0)) != REJECT_NONE:
		_fail("Stable placement after Age 2 is not unlocked: %s" % [unlocked_preview])
		return

	# Step 10: build the unlocked Stable and train a Cavalry.
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/StableButton") as Button, STABLE_ANCHOR)
	var stable := await _wait_building("Stable", 30.0)
	if stable == null:
		return
	var stable_id := int(stable.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(stable_id), 45.0, "Stable never completed"):
		return
	if _blocked_reason(stable_id) != 0:
		_fail("fresh Stable reported a blocked queue")
		return
	await _move_unit_away(builder, stable.global_position, 2.5)
	await _select_building(stable)
	await _click_button(get_node("Main/HUD/CommandPanel/TrainGrid/CavalryButton") as Button)
	if not await _wait_until(func(): return _queue_label(stable_id) == "Cavalry", 10.0,
			"Cavalry never entered the Stable queue"):
		return
	var cavalry_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Cavalry", 9, cavalry_box), 30.0,
			"trained Cavalry view never appeared"):
		return
	# Clear the Stable perimeter before the Town Center clicks below.
	var cavalry: Node3D = cavalry_box["view"]
	await _move_unit_away(cavalry, town_center.global_position, 4.0)

	# Step 11: a Town Center rally point; the next trained unit walks to it.
	await _select_building(town_center)
	await _click_world(Vector2(RALLY_CELL), MOUSE_BUTTON_RIGHT)
	if not await _wait_until(_rally_stored.bind(town_center_id), 10.0,
			"Town Center rally point was never stored"):
		return
	await _click_button(get_node("Main/HUD/CommandPanel/TrainGrid/VillagerButton") as Button)
	var rally_villager_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Villager", 9, rally_villager_box), 30.0,
			"rallied Villager view never appeared"):
		return
	var rally_villager: Node3D = rally_villager_box["view"]
	var rally_target := Vector3(RALLY_CELL.x + 0.5, 0.0, RALLY_CELL.y + 0.5)
	if not await _wait_until(
			func(): return rally_villager.global_position.distance_to(rally_target) < 1.5,
			15.0, "rallied villager never moved toward the rally point"):
		return

	# Step 12: idle-worker navigation selects a live idle_worker_ids member.
	if int(GrusBridge.economy_snapshot().get("idle_workers", 0)) < 1:
		_fail("no idle workers before idle navigation")
		return
	await _click_button(get_node("Main/HUD/EconomyPanel/IdleButton") as Button)
	if not await _wait_until(_single_ringed_idle_worker, 10.0,
			"idle navigation did not select an idle_worker_ids member"):
		return

	# Step 13 recap: construction ramp (House), queue ramps (Spearman/Archer/
	# Cavalry/Age2), FarmOccupied and Locked reject codes, and the blocked
	# fields were all asserted at their stages above; finish with the final
	# economy state and unblocked producers.
	var final: Dictionary = GrusBridge.economy_snapshot()
	if int(final.get("age", 1)) != 2 or int(final.get("population_cap", -1)) != 20 \
			or int(final.get("population_used", -1)) != 8:
		_fail("final economy state is not age 2 / population 8 of 20: %s" % [final])
		return
	for producer_id in [town_center_id, barracks_id, range_id, stable_id]:
		if _blocked_reason(producer_id) != 0:
			_fail("producer %d ended with a blocked queue" % producer_id)
			return

	if not GrusBridge.set_sim_speed(1.0):
		_fail("failed to restore Bevy virtual time")
		return

	print("GRUS_ECONOMY_SMOKE_OK food=%d wood=%d gold=%d age=2 cap=20 units=8 buildings=8 elapsed_ms=%d" % [
		int(final.get("food", 0)),
		int(final.get("wood", 0)),
		int(final.get("gold", 0)),
		Time.get_ticks_msec() - _start_ms,
	])
	get_tree().quit(0)
