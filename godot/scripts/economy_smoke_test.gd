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
const BERRY_ID := 1
const TREE_ID := 3
const COAL_ID := 6
## Northeast expansion tree used for the Depot delivery proof.
const FAR_TREE_ID := 14

## Solvency targets before any spending: deposit-driven, so reaching them
## proves real gather cycles (starts are 200 Food / 300 Scrap / 100 Coal).
const TARGET_SCRAP := 400
const TARGET_FOOD := 320
const TARGET_COAL := 160

const BUNKHOUSE_ANCHOR := Vector2i(9, 44)
const DEPOT_ANCHOR := Vector2i(38, 22)
const BARRACKS_ANCHOR := Vector2i(17, 41)

## Open ground west of the base; workers parked here stay well outside the
## 20 px unit-priority click radius of every building origin.
const STAGING_CELL := Vector2i(5, 47)
## Research Center rally cell for the rally-point proof.
const RALLY_CELL := Vector2i(20, 58)
## A cell inside the seeded Research Center footprint (anchor 12,46 + 4x4): the
## read-only placement preview must answer Occupied there.
const OCCUPIED_CELL := Vector2i(13, 47)

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

func _click_view(view: Node3D, button: int, fog_settle := false) -> void:
	_mouse_click(_camera.unproject_position(view.global_position), button)
	await get_tree().process_frame
	# Extra frame only for fog-gated targets whose view was recently
	# revealed; plain clicks keep the original single-frame settle.
	if fog_settle:
		await get_tree().process_frame

func _click_world(point: Vector2, button: int) -> void:
	_mouse_click(_camera.unproject_position(Vector3(point.x + 0.25, 0.0, point.y + 0.25)), button)
	await get_tree().process_frame

func _click_button(button: Button) -> void:
	_mouse_click(button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
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

func _stockpiles_at_least(scrap: int, food: int, coal: int) -> bool:
	var state: Dictionary = GrusBridge.economy_snapshot()
	return int(state.get("scrap", -1)) >= scrap \
		and int(state.get("food", -1)) >= food \
		and int(state.get("coal", -1)) >= coal

func _building_complete_at(id: int) -> bool:
	return bool(GrusBridge.building_snapshot(id).get("complete", false))

## Delivery proof for the Depot: a Scrap stockpile credit that lands while
## the worker stands near the Depot. Its far-tree gather slots are
## >= 4 cells from the Depot origin and the Research Center >= 28, so only a
## real Depot deposit can satisfy this within one physics frame of drift.
func _scrap_deposited_at_depot(worker: Node3D, depot: Vector3, box: Dictionary) -> bool:
	var scrap := int(GrusBridge.economy_snapshot().get("scrap", -1))
	var increased := scrap > int(box.get("scrap", scrap))
	box["scrap"] = scrap
	return increased and worker.global_position.distance_to(depot) < 12.0

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
	if catalogue.has("age_up"):
		_fail("catalogue still advertises the deleted Age-up job")
		return
	if absf(float(boot.get("gather_rate", 0.0)) - 2.0) > 0.0001:
		_fail("starting gather rate is not 2.0: %s" % [boot])
		return
	if int(boot.get("population_used", -1)) != 4 or int(boot.get("population_cap", -1)) != 10:
		_fail("starting population is not 4/10: %s" % [boot])
		return
	if boot.has("age"):
		_fail("economy snapshot still carries the deleted age field: %s" % [boot])
		return
	var research_center := _building_view(1)
	if research_center == null:
		_fail("Research Center view 1 is missing")
		return
	var research_center_id := int(research_center.get_meta("building_id"))
	_building_ids = {1: true, 2: true}
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

	# Step 1: workers onto trees/berries/coal via right-click; stockpiles
	# move only after a deposit, never on the command itself.
	for job in [[1, TREE_ID], [2, BERRY_ID], [3, COAL_ID]]:
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

	# Fog retarget (HPA-473): the Depot anchor (38,22) and the northeast
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

	# Step 5: Depot delivery proof. Retask worker 1 to the northeast
	# expansion tree (~6 cells from the Depot, ~40 from the Research Center):
	# every later Scrap deposit must land at the Depot.
	await _place_building(builder, get_node("Main/HUD/CommandPanel/BuildGrid/DepotButton") as Button, DEPOT_ANCHOR)
	var depot := await _wait_building("Depot", 25.0)
	if depot == null:
		return
	var depot_id := int(depot.get_meta("building_id"))
	if not await _wait_until(_building_complete_at.bind(depot_id), 30.0, "Depot never completed"):
		return
	var scrap_worker := _unit_view(1)
	var far_tree := _resource_view(FAR_TREE_ID)
	if scrap_worker == null or far_tree == null:
		_fail("scrap worker or expansion tree view is missing")
		return
	await _click_view(scrap_worker, MOUSE_BUTTON_LEFT)
	# The far tree's view is fog-revealed: it pays the fog settle.
	await _click_view(far_tree, MOUSE_BUTTON_RIGHT, true)
	var delivery_box := {"scrap": int(GrusBridge.economy_snapshot().get("scrap", -1))}
	if not await _wait_until(_scrap_deposited_at_depot.bind(scrap_worker, depot.global_position, delivery_box), 45.0,
			"no scrap delivery ever reached the Depot"):
		return
	# Retire the scrap gatherer: its northeast corridor stays clear of every
	# later click target, so parking in place is safe.
	await _stop_unit(scrap_worker)

	# Reject-code pin: the read-only placement preview reports the numeric
	# Occupied code for a footprint inside the seeded Research Center — the same
	# RejectReason discriminant the bridge emits on real rejects.
	var occupied_preview: Dictionary = GrusBridge.placement_preview(4, "Bunkhouse", OCCUPIED_CELL.x, OCCUPIED_CELL.y)
	if int(occupied_preview.get("reject_code", -1)) != REJECT_OCCUPIED:
		_fail("placement preview inside the Research Center did not report Occupied (%d): %s" % [REJECT_OCCUPIED, occupied_preview])
		return

	# Retire the food and coal gatherers: the finite sources deplete at 20x
	# virtual time and would otherwise idle a worker inside the Research Center
	# click zone. Park both on the staging ground.
	var food_worker := _unit_view(2)
	var coal_worker := _unit_view(3)
	if food_worker == null or coal_worker == null:
		_fail("workers 2/3 views are missing")
		return
	await _park_at_staging(food_worker)
	await _park_at_staging(coal_worker)

	# Step 6: the Barracks is the single military producer — it accepts both
	# Guard and Rifleman on one FIFO queue, each charged once at acceptance.
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
	# second click lands on the next command tick, so poll briefly).
	if not await _wait_until(
			func():
				var charged: Dictionary = GrusBridge.economy_snapshot()
				return int(charged.get("food", -1)) == food_before - 100 \
					and int(charged.get("scrap", -1)) == scrap_before - 40,
			10.0, "Barracks enqueue did not charge Guard+Rifleman exactly once"):
		return
	var guard_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Guard", 9, guard_box), 30.0,
			"trained Guard view never appeared"):
		return
	if not await _wait_until(func(): return _queue_label(barracks_id) == "Rifleman", 15.0,
			"Rifleman never became the FIFO head after the Guard spawned"):
		return
	var rifleman_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Rifleman", 9, rifleman_box), 35.0,
			"trained Rifleman view never appeared"):
		return
	if not await _wait_until(func(): return _queue_label(barracks_id) == "", 15.0,
			"Rifleman job never left the Barracks queue"):
		return
	# Park both combatants clear of the Research Center click zone below.
	for unit in [guard_box["view"], rifleman_box["view"]]:
		await _move_unit_away(unit as Node3D, research_center.global_position, 4.0)

	# Step 7: a Research Center rally point; the next trained unit walks to it.
	await _select_building(research_center)
	await _click_world(Vector2(RALLY_CELL), MOUSE_BUTTON_RIGHT)
	if not await _wait_until(_rally_stored.bind(research_center_id), 10.0,
			"Research Center rally point was never stored"):
		return
	await _click_button(get_node("Main/HUD/CommandPanel/TrainGrid/WorkerButton") as Button)
	var rally_worker_box := {}
	if not await _wait_until(_unit_of_kind_present.bind("Worker", 9, rally_worker_box), 30.0,
			"rallied Worker view never appeared"):
		return
	var rally_worker: Node3D = rally_worker_box["view"]
	var rally_target := Vector3(RALLY_CELL.x + 0.5, 0.0, RALLY_CELL.y + 0.5)
	if not await _wait_until(
			func(): return rally_worker.global_position.distance_to(rally_target) < 1.5,
			15.0, "rallied worker never moved toward the rally point"):
		return

	# Step 8: idle-worker navigation selects a live idle_worker_ids member.
	if int(GrusBridge.economy_snapshot().get("idle_workers", 0)) < 1:
		_fail("no idle workers before idle navigation")
		return
	await _click_button(get_node("Main/HUD/EconomyPanel/IdleButton") as Button)
	if not await _wait_until(_single_ringed_idle_worker, 10.0,
			"idle navigation did not select an idle_worker_ids member"):
		return

	# Step 9 recap: construction ramp (Bunkhouse), queue ramps (Guard/Rifleman),
	# the Occupied reject code, and the blocked fields were all asserted at
	# their stages above; finish with the final economy state and unblocked
	# producers.
	var final: Dictionary = GrusBridge.economy_snapshot()
	if int(final.get("population_cap", -1)) != 20 or int(final.get("population_used", -1)) != 7:
		_fail("final economy state is not population 7 of 20: %s" % [final])
		return
	for producer_id in [research_center_id, barracks_id]:
		if _blocked_reason(producer_id) != 0:
			_fail("producer %d ended with a blocked queue" % producer_id)
			return

	# Step 10: the same UI train path that accepted solvent enqueues now
	# runs the Food stockpile dry. Every gatherer is retired, so each real
	# GuardButton click charges exactly the 60f catalogue cost until the
	# stockpile cannot pay; an identical click on the drained-empty queue
	# must then reject InsufficientResources and leave stockpiles, the
	# queue and the building census untouched.
	await _select_building(barracks)
	var guard_button := get_node("Main/HUD/CommandPanel/TrainGrid/GuardButton") as Button
	var insufficient := false
	var attempts := 6
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
			if int(after_click.get("food", -1)) != before_click["food"] - 60:
				_fail("accepted Guard enqueue did not charge exactly 60 food: %s -> %s" % [before_click, after_click])
				return
			continue
		if reject_code != REJECT_INSUFFICIENT:
			_fail("Guard enqueue rejected with unexpected code %d (expected %d)" % [reject_code, REJECT_INSUFFICIENT])
			return
		# The rejected click changed nothing: identical stockpiles, queue
		# and building census — the enqueue was refused before any state
		# was touched.
		if int(after_click.get("food", -1)) != before_click["food"] \
				or int(after_click.get("scrap", -1)) != before_click["scrap"] \
				or int(after_click.get("coal", -1)) != before_click["coal"]:
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
	# click re-tests it: an empty head label with zero progress is the
	# authoritative "no jobs" proof — the head label alone cannot see
	# queued successors (building_snapshot exposes only the front job).
	if not await _wait_until(func(): return _queue_label(barracks_id) == "", 60.0,
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
	if int(unaffordable_after.get("food", -1)) != unaffordable_before["food"] \
			or int(unaffordable_after.get("scrap", -1)) != unaffordable_before["scrap"] \
			or int(unaffordable_after.get("coal", -1)) != unaffordable_before["coal"]:
		_fail("rejected Guard enqueue still moved the stockpile")
		return
	if _queue_label(barracks_id) != "" or _queue_progress(barracks_id) != 0.0:
		_fail("rejected Guard enqueue still left a Barracks job: %s" % _queue_label(barracks_id))
		return
	if _blocked_reason(barracks_id) != unaffordable_before["blocked"]:
		_fail("rejected Guard enqueue changed the Barracks blocked state")
		return
	if get_tree().get_nodes_in_group("building_views").size() != unaffordable_before["buildings"]:
		_fail("rejected Guard enqueue still changed the building census")
		return

	if not GrusBridge.set_sim_speed(1.0):
		_fail("failed to restore Bevy virtual time")
		return

	var end: Dictionary = GrusBridge.economy_snapshot()
	print("GRUS_ECONOMY_SMOKE_OK food=%d scrap=%d coal=%d cap=%d units=%d buildings=%d unaffordable_reject=%d elapsed_ms=%d" % [
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
