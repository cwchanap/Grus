extends Node

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## Stable id for the fog-scouting leg; positions are read off the live view
## at run time, never copied from the fixture into GDScript.
const EXPANSION_SALVAGE_ID := 14

const SmokeHelpers := preload("res://scripts/smoke_helpers.gd")

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

func _modifier_event(keycode: int, pressed: bool) -> void:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.pressed = pressed
	Input.parse_input_event(event)

func _mouse_click(position: Vector2, button: int, shift_pressed := false) -> void:
	var press := InputEventMouseButton.new()
	press.button_index = button
	press.position = position
	press.pressed = true
	press.shift_pressed = shift_pressed
	Input.parse_input_event(press)

	var release := InputEventMouseButton.new()
	release.button_index = button
	release.position = position
	release.pressed = false
	release.shift_pressed = shift_pressed
	Input.parse_input_event(release)

func _mouse_drag(start: Vector2, finish: Vector2, button: int) -> void:
	var press := InputEventMouseButton.new()
	press.button_index = button
	press.position = start
	press.pressed = true
	Input.parse_input_event(press)

	var motion := InputEventMouseMotion.new()
	motion.position = finish
	motion.relative = finish - start
	if button == MOUSE_BUTTON_LEFT:
		motion.button_mask = MOUSE_BUTTON_MASK_LEFT
	elif button == MOUSE_BUTTON_MIDDLE:
		motion.button_mask = MOUSE_BUTTON_MASK_MIDDLE
	Input.parse_input_event(motion)

	var release := InputEventMouseButton.new()
	release.button_index = button
	release.position = finish
	release.pressed = false
	Input.parse_input_event(release)

func _key_tap(keycode: int, ctrl_pressed := false) -> void:
	var press := InputEventKey.new()
	press.keycode = keycode
	press.pressed = true
	press.ctrl_pressed = ctrl_pressed
	Input.parse_input_event(press)

	var release := InputEventKey.new()
	release.keycode = keycode
	release.pressed = false
	release.ctrl_pressed = ctrl_pressed
	Input.parse_input_event(release)

func _find_unit(units: Array[Node], id: int) -> Node3D:
	for unit in units:
		if int(unit.get_meta("unit_id", -1)) == id:
			return unit as Node3D
	return null

func _ring(unit: Node3D) -> MeshInstance3D:
	return unit.get_node("SelectionRing") as MeshInstance3D

func _art_bounds(root: Node3D) -> AABB:
	var bounds := AABB()
	var first := true
	for node in root.find_children("*", "MeshInstance3D", true, false):
		var mesh := node as MeshInstance3D
		var local_bounds: AABB = root.global_transform.affine_inverse() * mesh.global_transform * mesh.get_aabb()
		bounds = local_bounds if first else bounds.merge(local_bounds)
		first = false
	return bounds

## Static imported art must never smuggle in gameplay nodes: the .import
## files disable name/type suffixes, and this sweep is the regression
## tripwire for physics bodies, colliders, navigation regions and
## occluders arriving via a future glTF.
func _foreign_nodes(root: Node) -> PackedStringArray:
	var found := PackedStringArray()
	for node in root.find_children("*", "", true, false):
		if node is PhysicsBody3D or node is CollisionShape3D or node is CollisionPolygon3D \
				or node is NavigationRegion3D or node is OccluderInstance3D:
			found.append("%s (%s)" % [node.name, node.get_class()])
	return found

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

func _building_view(id: int) -> Node3D:
	return _find_view("building_views", "building_id", id)

func _resource_view(id: int) -> Node3D:
	return _find_view("resource_views", "resource_id", id)

func _shown(view: Node3D) -> bool:
	return view != null and view.is_visible_in_tree()

func _hidden(view: Node3D) -> bool:
	return view != null and not view.is_visible_in_tree()

## A queued move is not acceptance: the sim's own feedback must say so.
func _accepted_since(revision: int) -> bool:
	return int(GrusBridge.command_feedback_revision()) > revision \
		and str(GrusBridge.command_feedback()).begins_with("Command accepted")

func _defense_placement_anchor(
		builder_id: int, kind: String, reserved_footprints: Array[Rect2i] = []) -> Vector2i:
	var worker := _unit_view(builder_id)
	if worker == null:
		return Vector2i(-1, -1)
	var origin := Vector2i(floori(worker.global_position.x), floori(worker.global_position.z))
	for radius in range(1, 17):
		for y in range(-radius, radius + 1):
			for x in range(-radius, radius + 1):
				if maxi(absi(x), absi(y)) != radius:
					continue
				var candidate := origin + Vector2i(x, y)
				var preview: Dictionary = GrusBridge.placement_preview(builder_id, kind, candidate.x, candidate.y)
				if not bool(preview.get("valid", false)):
					continue
				var footprint := Rect2i(
					Vector2i(int(preview.get("anchor_x", candidate.x)), int(preview.get("anchor_y", candidate.y))),
					Vector2i(int(preview.get("width", 1)), int(preview.get("height", 1))))
				var overlaps_reserved := false
				for reserved in reserved_footprints:
					if footprint.intersects(reserved):
						overlaps_reserved = true
						break
				if not overlaps_reserved:
					return candidate
	return Vector2i(-1, -1)

func _arm_defense(main: Node3D, button: Button, kind: String) -> bool:
	var revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	if int(GrusBridge.command_feedback_revision()) != revision \
			or str(main.get("_placement_kind")) != kind:
		_fail("%s build button leaked a command or did not arm placement" % kind)
		return false
	return true

func _place_defense_at(main: Node3D, camera: Camera3D, kind: String, anchor: Vector2i) -> bool:
	if str(main.get("_placement_kind")) != kind:
		_fail("%s button did not arm placement" % kind)
		return false
	var revision := int(GrusBridge.command_feedback_revision())
	var building_count := get_tree().get_nodes_in_group("building_views").size()
	var screen := camera.unproject_position(Vector3(anchor.x + 0.25, 0.0, anchor.y + 0.25))
	SmokeHelpers.push_mouse_click(get_tree(), screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(
			func(): return _accepted_since(revision) \
				and get_tree().get_nodes_in_group("building_views").size() == building_count + 1 \
				and main.get("_pending_placement_requests").is_empty(),
			10.0, "%s placement was not accepted and rendered at %s" % [kind, anchor]):
		return false
	if str(main.get("_placement_kind")) != kind:
		_fail("successful %s placement did not leave placement armed" % kind)
		return false
	return true

func _move_scout(unit_id: int, target: Vector2, what: String) -> bool:
	var revision_before := int(GrusBridge.command_feedback_revision())
	if not GrusBridge.move_units(PackedInt32Array([unit_id]), target):
		_fail("%s: move was not queued" % what)
		return false
	return await _wait_until(_accepted_since.bind(revision_before), 10.0,
			"%s: move was not accepted by the sim" % what)

func _cell_state(states: PackedInt32Array, cell: Vector2i) -> int:
	return states[cell.y * 128 + cell.x]

## Fog overlay buffers are authoritative only once the overlay's _process
## consumed the same visibility revision the bridge reports; a marching
## scout keeps the revision ticking ahead of it. Returns the snapshot the
## overlay actually rendered, or {} after a loud failure.
func _settled_fog_snapshot(where: String) -> Dictionary:
	var fog := get_node("Main/Battlefield/FogOfWar")
	for _attempt in range(8):
		var synced := func() -> bool:
			var revision := int(GrusBridge.visibility_revision())
			return revision >= 0 and int(fog.get("_revision")) == revision
		if not await _wait_until(synced, 30.0,
				"%s: fog overlay never consumed the bridge visibility revision" % where):
			return {}
		var snap: Dictionary = GrusBridge.visibility_snapshot()
		if int(snap.get("revision", -1)) == int(fog.get("_revision")):
			return snap
		# The revision moved during the read; the next attempt catches up.
	_fail("%s: visibility snapshot never matched the consumed fog revision" % where)
	return {}

## Asserts the fog overlay's packed buffers mirror a settled snapshot
## exactly. The buffer-layout contract itself lives in
## SmokeHelpers.fog_overlay_error — shared with reset_test.
func _assert_fog_overlay(where: String, snap: Dictionary) -> bool:
	var error := SmokeHelpers.fog_overlay_error(where, get_node("Main/Battlefield/FogOfWar"), snap)
	if not error.is_empty():
		_fail(error)
		return false
	return true

func _assert_action_icon_paths(main: Node3D) -> bool:
	for spec in [
		["HUD/CommandPanel/BuildGrid", "buildings", 9],
		["HUD/CommandPanel/TrainGrid", "units", 3],
	]:
		var grid := main.get_node_or_null(spec[0]) as Container
		if grid == null:
			_fail("action icon grid is missing: %s" % spec[0])
			return false
		var count := 0
		for node in grid.get_children():
			if not node is Button:
				continue
			var button := node as Button
			count += 1
			var kind := str(button.get_meta("kind", ""))
			var expected := "res://assets/ui/icons/%s/%s.png" % [spec[1], kind.to_snake_case()]
			var actual := button.icon.resource_path if button.icon != null else ""
			if kind.is_empty() or actual != expected:
				_fail("%s icon path is %s; expected %s" % [button.name, actual if not actual.is_empty() else "missing", expected])
				return false
		if count != int(spec[2]):
			_fail("%s has %d action Buttons; expected %d" % [grid.name, count, int(spec[2])])
			return false
	return true

func _has_visible_icon(root: Node, resource_path: String) -> bool:
	var nodes: Array[Node] = [root]
	nodes.append_array(root.find_children("*", "", true, false))
	for node in nodes:
		if node is CanvasItem and not node.is_visible_in_tree():
			continue
		if node is TextureRect:
			var texture := (node as TextureRect).texture
			if texture != null and texture.resource_path == resource_path:
				return true
		elif node is RichTextLabel and (node as RichTextLabel).text.contains(resource_path):
			return true
	return false

## Pilot structure import check (HPA-291): the PackedScene must load and
## instantiate a nonempty mesh, carry no gameplay nodes, sit inside the
## normalized ±0.5 footprint with a grounded base, and resolve its diffuse
## texture from the committed res://assets/structures/ PNG — the same
## recipe Checkpoint 3 extends to the whole building catalogue.
func _assert_structure_import(path: String) -> bool:
	if not ResourceLoader.exists(path):
		_fail("pilot structure import is missing: %s" % path)
		return false
	var packed := load(path) as PackedScene
	if packed == null:
		_fail("pilot structure import did not load as a PackedScene: %s" % path)
		return false
	var model := packed.instantiate() as Node3D
	if model == null:
		_fail("pilot structure import did not instantiate a Node3D: %s" % path)
		return false
	# A detached node has no global transform — park it in the tree at the
	# origin so _art_bounds sees the same space the live view would.
	add_child(model)
	var foreign := _foreign_nodes(model)
	if not foreign.is_empty():
		_fail("%s imported gameplay nodes: %s" % [path, foreign])
		return false
	var meshes := model.find_children("*", "MeshInstance3D", true, false)
	if meshes.is_empty():
		_fail("%s imported no meshes" % path)
		return false
	var bounds := _art_bounds(model)
	if bounds.size.length() < 0.1 or bounds.position.x < -0.51 or bounds.end.x > 0.51 \
			or bounds.position.z < -0.51 or bounds.end.z > 0.51 or bounds.position.y < -0.01:
		_fail("%s art escapes the normalized footprint: %s" % [path, bounds])
		return false
	for node in meshes:
		var material := (node as MeshInstance3D).get_active_material(0) as StandardMaterial3D
		var texture := material.albedo_texture if material != null else null
		if texture == null or not texture.resource_path.begins_with("res://assets/structures/"):
			_fail("%s mesh material does not resolve a committed structures texture: %s" % [
				path, texture.resource_path if texture != null else "missing"])
			return false
	model.free()
	return true

func _run() -> void:
	var camera := get_node("Main/Camera3D") as Camera3D
	var default_camera_transform := camera.global_transform
	var default_camera_size := camera.size
	var window := get_window()
	window.content_scale_size = TEST_VIEWPORT_SIZE
	window.size = TEST_VIEWPORT_SIZE
	await get_tree().process_frame

	var viewport_size := get_viewport().get_visible_rect().size
	if viewport_size != Vector2(TEST_VIEWPORT_SIZE):
		_fail("smoke viewport did not resize to project baseline: %s" % viewport_size)
		return
	if not _assert_action_icon_paths(get_node("Main") as Node3D):
		return

	var units: Array[Node] = []
	for _frame in range(160):
		await get_tree().physics_frame
		units = get_tree().get_nodes_in_group("unit_views")
		if units.size() == 4:
			break

	if units.size() != 4:
		_fail("expected 4 ECS-backed unit views, found %d" % units.size())
		return

	var unit_one: Node3D = null
	var unit_two: Node3D = null
	var unit_three: Node3D = null
	for _frame in range(30):
		await get_tree().process_frame
		units = get_tree().get_nodes_in_group("unit_views")
		unit_one = _find_unit(units, 1)
		unit_two = _find_unit(units, 2)
		unit_three = _find_unit(units, 3)
		if unit_one != null and unit_two != null and unit_three != null:
			break

	if unit_one == null or unit_two == null or unit_three == null or _unit_view(4) == null:
		_fail("stable UnitId metadata did not initialize for skirmish fixture")
		return
	for id in range(1, 5):
		var member := _find_unit(units, id)
		if member == null or int(member.get_meta("team_id", -1)) != 1:
			_fail("Team 1 worker %d is missing or has the wrong team metadata" % id)
			return
	if _building_view(2) != null:
		_fail("one-base fixture unexpectedly contains a second Research Center")
		return

	# Normal skirmish boots into Start; gameplay opens only via start_match.
	if not GrusBridge.has_method("start_match") or not GrusBridge.has_method("session_snapshot"):
		_fail("session bridge methods are missing")
		return
	var boot_session: Dictionary = GrusBridge.session_snapshot()
	if str(boot_session.get("phase", "")) != "Start":
		_fail("skirmish boot session is not Start: %s" % [boot_session])
		return
	# First-run controls aid: the Start overlay must surface the controls label
	# before gameplay opens. Tokens match case-insensitively against the label's
	# own text; A/S must appear as standalone words, not substrings.
	var controls_label := get_node("Main/HUD/SessionPanel/ControlsLabel") as Label
	if controls_label == null or not controls_label.is_visible_in_tree():
		_fail("HUD/SessionPanel/ControlsLabel is missing or not visible before Start")
		return
	var controls_text := controls_label.text.to_lower()
	for token in ["right click", "minimap", "esc"]:
		if not controls_text.contains(token):
			_fail("ControlsLabel is missing '%s': %s" % [token, controls_label.text])
			return
	var control_words := controls_text.replace("\n", " ").split(" ", false)
	for token in ["a", "s"]:
		if not token in control_words:
			_fail("ControlsLabel is missing standalone token '%s': %s" % [token, controls_label.text])
			return
	# The Start overlay keeps the first wave timer frozen until the player
	# starts the one-base survival match.
	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return
	await get_tree().process_frame
	if controls_label.is_visible_in_tree():
		_fail("ControlsLabel stayed visible after the match reached Playing")
		return

	var command_status := get_node("Main/HUD/CommandStatus") as Label
	var clear_screen := Vector2(viewport_size.x * 0.5, viewport_size.y - 40.0)

	# Camera framing contract, before any pan/zoom/minimap interaction: the
	# live Team-1 Research Center view (stable BuildingId 1) must project
	# into the central half of the viewport. The projection is independent —
	# nothing copies the authored anchor into GDScript — so a layout or
	# camera drift fails loudly here.
	var research_center_view: Node3D = null
	for _frame in range(60):
		await get_tree().process_frame
		research_center_view = _building_view(1)
		if research_center_view != null:
			break
	if research_center_view == null:
		_fail("Research Center view 1 never initialized for the camera check")
		return
	var center_screen := camera.unproject_position(research_center_view.global_position)
	if center_screen.x < viewport_size.x * 0.25 or center_screen.x > viewport_size.x * 0.75 \
			or center_screen.y < viewport_size.y * 0.25 or center_screen.y > viewport_size.y * 0.75:
		_fail("central Research Center projects outside the viewport's central half: %s" % center_screen)
		return

	var salvage_view: Node3D = null
	for _frame in range(60):
		salvage_view = _resource_view(EXPANSION_SALVAGE_ID)
		if salvage_view != null:
			break
		await get_tree().process_frame
	if salvage_view == null:
		_fail("scout target view is missing (salvage %s)" % [salvage_view])
		return

	# HPA-188 research HUD: geometry, catalogue binding, live lock state and
	# the Analyze button's real input dispatch — all during real Playing.
	var research_panel := get_node_or_null("Main/HUD/ResearchPanel") as Panel
	var research_bar := get_node_or_null("Main/HUD/ResearchPanel/ResearchBar") as ProgressBar
	var research_label := get_node_or_null("Main/HUD/ResearchPanel/ResearchLabel") as Label
	var research_icon := get_node_or_null("Main/HUD/ResearchPanel/ResearchIcon") as TextureRect
	var analyze_button := get_node_or_null("Main/HUD/ResearchPanel/AnalyzeButton") as Button
	var wave_hud := get_node("Main/HUD/WaveLabel") as Control
	if research_panel == null or research_bar == null or research_label == null or research_icon == null \
			or analyze_button == null or wave_hud == null:
		_fail("research HUD nodes are missing under HUD/ResearchPanel")
		return
	if not _has_visible_icon(get_node("Main/HUD"), "res://assets/ui/icons/status/research_progress.png"):
		_fail("research HUD is missing status/research_progress.png")
		return
	# The compact panel hugs the left edge below the InputShield band,
	# inside the 1280x720 viewport, and fully contains its Analyze button —
	# never over the northwest corridor's or expansion salvage's projected
	# cells, which must stay clickable under the default framing. The salvage
	# point derives from its live view so the probe tracks the authored layout.
	var research_rect := research_panel.get_global_rect()
	if not Rect2(Vector2.ZERO, viewport_size).encloses(research_rect):
		_fail("research panel escapes the viewport: %s" % research_rect)
		return
	var research_icon_rect := research_icon.get_global_rect()
	var research_icon_minimum := research_icon.get_combined_minimum_size()
	if not research_rect.encloses(research_icon_rect) \
			or not research_icon_rect.size.is_equal_approx(Vector2(18.0, 18.0)) \
			or research_icon_minimum.x > 18.0 or research_icon_minimum.y > 18.0 \
			or research_icon_rect.intersects(research_label.get_global_rect()) \
			or research_icon_rect.intersects(analyze_button.get_global_rect()):
		_fail("research icon exceeds its 18x18 slot or overlaps HUD controls: rect=%s minimum=%s" % [
			research_icon_rect, research_icon_minimum])
		return
	if not camera.global_transform.is_equal_approx(default_camera_transform) \
			or not is_equal_approx(camera.size, default_camera_size):
		_fail("research panel overlap probes are not using the default camera framing")
		return
	var salvage_screen := camera.unproject_position(salvage_view.global_position)
	if research_rect.has_point(salvage_screen):
		_fail("research panel covers the expansion salvage at %s: %s" % [
			salvage_screen, research_rect])
		return
	for corridor_cell in [Vector2(38, 24), Vector2(40, 20)]:
		var corridor_screen := camera.unproject_position(
			Vector3(corridor_cell.x + 0.5, 0.0, corridor_cell.y + 0.5))
		if research_rect.has_point(corridor_screen):
			_fail("research panel covers northwest corridor cell %s at %s: %s" % [
				corridor_cell, corridor_screen, research_rect])
			return
	if not research_rect.encloses(analyze_button.get_global_rect()):
		_fail("Analyze button escapes the research panel")
		return

	# Imported art must stay inside the simulation's blocked rectangles.
	var ruin_rects: PackedInt32Array = GrusBridge.obstacle_rects()
	if ruin_rects.is_empty() or ruin_rects.size() % 4 != 0:
		_fail("obstacle_rects payload is malformed: %s" % [ruin_rects])
		return
	var ruins_root := get_node("Main/Battlefield/Ruins") as Node3D
	var ruin_count := ruin_rects.size() / 4
	if ruins_root.get_child_count() != ruin_count:
		_fail("rendered ruin count %d != bridge payload %d" % [ruins_root.get_child_count(), ruin_count])
		return
	for i in ruin_count:
		var min_cell := Vector2i(ruin_rects[i * 4], ruin_rects[i * 4 + 1])
		var max_cell := Vector2i(ruin_rects[i * 4 + 2], ruin_rects[i * 4 + 3])
		var cells := max_cell - min_cell + Vector2i.ONE
		var ruin := ruins_root.get_node("Ruin%d" % i) as Node3D
		if not ruin.scene_file_path.ends_with(".glb"):
			_fail("ruin %d is not imported glTF art" % i)
			return
		var foreign := _foreign_nodes(ruin)
		if not foreign.is_empty():
			_fail("ruin %d art imported gameplay nodes: %s" % [i, foreign])
			return
		var bounds := _art_bounds(ruin)
		var world_bounds: AABB = ruin.global_transform * bounds
		var want_center := Vector3(min_cell.x + cells.x * 0.5, 0.0, min_cell.y + cells.y * 0.5)
		var bounds_center := world_bounds.get_center()
		# Art covers the blocked rect exactly: same footprint size and
		# center, no under-coverage that would render blocked cells open.
		if ruin.global_position != want_center or bounds.size.length() < 0.1 \
				or absf(world_bounds.size.x - cells.x) > 0.01 \
				or absf(world_bounds.size.z - cells.y) > 0.01 \
				or absf(bounds_center.x - want_center.x) > 0.01 \
				or absf(bounds_center.z - want_center.z) > 0.01 \
				or world_bounds.position.y < -0.01 or world_bounds.end.y > 2.01:
			_fail("ruin %d art does not exactly cover its %dx%d bridge footprint: bounds %s" % [
				i, cells.x, cells.y, world_bounds])
			return
	var ground := get_node("Main/Battlefield/Ground") as MeshInstance3D
	if ground.material_override.albedo_texture == null:
		_fail("winter ground has no snow texture")
		return
	var resource_models := {"Food": "ration_cache.glb", "Scrap": "scrap_ruin.glb", "Coal": "coal_seam.glb"}
	for resource in get_tree().get_nodes_in_group("resource_views"):
		var model := resource.get_node_or_null("Model") as Node3D
		var kind := str(resource.get_meta("resource_kind", ""))
		if model == null or not model.scene_file_path.ends_with(resource_models.get(kind, "missing")):
			_fail("resource %s has no matching imported art" % kind)
			return
		var foreign := _foreign_nodes(model)
		if not foreign.is_empty():
			_fail("resource %s art imported gameplay nodes: %s" % [kind, foreign])
			return
		var bounds := _art_bounds(model)
		if bounds.size.length() < 0.1 or bounds.position.x < -0.5 or bounds.end.x > 0.5 \
				or bounds.position.z < -0.5 or bounds.end.z > 0.5 or bounds.position.y < -0.01:
			_fail("resource %s art extends outside its gather cell: %s" % [kind, bounds])
			return

	# HPA-291 pilot structure imports: only the two committed assets are
	# asserted here — asserting the whole catalogue now would mask every
	# later selection/fog/input leg on the first missing GLB (Checkpoint 3
	# adds the catalogue-wide loop once all eleven models exist).
	for structure in ["construction_site", "bunkhouse"]:
		if not _assert_structure_import("res://assets/structures/%s.glb" % structure):
			return

	var size_before_zoom := camera.size
	_mouse_click(Vector2(640.0, 360.0), MOUSE_BUTTON_WHEEL_UP)
	await get_tree().process_frame
	if camera.size >= size_before_zoom:
		_fail("mouse wheel did not zoom the orthographic camera")
		return

	var camera_before_pan := Vector2(camera.position.x, camera.position.z)
	_mouse_drag(Vector2(640.0, 360.0), Vector2(680.0, 385.0), MOUSE_BUTTON_MIDDLE)
	await get_tree().process_frame
	var camera_after_pan := Vector2(camera.position.x, camera.position.z)
	if camera_after_pan.distance_to(camera_before_pan) <= 0.1:
		_fail("middle-drag did not pan the camera")
		return

	_mouse_click(camera.unproject_position(unit_one.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame

	if not _ring(unit_one).visible:
		_fail("left-click input did not select friendly unit 1 after camera pan/zoom")
		return

	# Resource gathering through real input only: at maximum zoom, a
	# right-click on the farthest-projected art corner of every visible
	# resource must queue a gather — the pick target scales with the cell,
	# not with a fixed pixel radius.
	var controller := get_node("Main")
	var gather_zoom_start := camera.size
	for _step in range(16):
		if camera.size <= controller.MIN_CAMERA_SIZE:
			break
		_mouse_click(Vector2(640.0, 360.0), MOUSE_BUTTON_WHEEL_UP)
		await get_tree().process_frame
	if camera.size > controller.MIN_CAMERA_SIZE:
		_fail("wheel zoom never reached the minimum camera size: %s" % camera.size)
		return
	# Every Food/Scrap/Coal model kind needs at least one targeted pick —
	# their imported bounds differ, so a silhouette that picks nothing or
	# the wrong neighbour can no longer pass on a stale status label.
	var gathered := {}
	# Click points must reach _unhandled_input: skip any spot a HUD Control
	# would consume (shield, minimap, panels, buttons).
	var hud_rects: Array[Rect2] = []
	for node in get_node("Main/HUD").find_children("*", "Control", true, false):
		var control := node as Control
		if control != null and control.is_visible_in_tree() \
				and control.mouse_filter != Control.MOUSE_FILTER_IGNORE:
			hud_rects.append(control.get_global_rect())
	for resource in get_tree().get_nodes_in_group("resource_views"):
		if not resource.is_visible_in_tree():
			continue
		var center_px := camera.unproject_position(resource.global_position)
		var art_bounds := _art_bounds(resource)
		var farthest := Vector2.ZERO
		var farthest_distance := -1.0
		for x in [art_bounds.position.x, art_bounds.end.x]:
			for y in [art_bounds.position.y, art_bounds.end.y]:
				for z in [art_bounds.position.z, art_bounds.end.z]:
					var point := camera.unproject_position(
						resource.global_transform * Vector3(x, y, z))
					var on_hud := hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(point))
					var distance := center_px.distance_to(point)
					if distance > farthest_distance and not on_hud \
							and Rect2(Vector2.ZERO, viewport_size).has_point(point):
						farthest = point
						farthest_distance = distance
		if farthest_distance < 0.0:
			continue
		var resource_id := int(resource.get_meta("resource_id", -1))
		var click_at := center_px + (farthest - center_px) * 0.95
		# Sentinel reset: only a fresh _issue_context_command write replaces
		# it, so input swallowed before _unhandled_input fails here instead
		# of passing on the previous click's stale "Gather command queued".
		command_status.text = "<no command issued>"
		var feedback_before := int(GrusBridge.command_feedback_revision())
		# push_input dispatches through the same window->viewport routing a
		# real click takes, synchronously: "Gather command queued" is set by
		# _issue_context_command and read before the bridge feedback text
		# overwrites the label. A pick miss would read "Move command queued"
		# instead, so the assertion discriminates gather from ground move.
		var gather_press := InputEventMouseButton.new()
		gather_press.button_index = MOUSE_BUTTON_RIGHT
		gather_press.position = click_at
		gather_press.pressed = true
		get_tree().root.push_input(gather_press)
		var gather_release := InputEventMouseButton.new()
		gather_release.button_index = MOUSE_BUTTON_RIGHT
		gather_release.position = click_at
		gather_release.pressed = false
		get_tree().root.push_input(gather_release)
		if command_status.text != "Gather command queued":
			_fail("right-click on resource %s art queued no gather: %s" % [
				resource.get_meta("resource_kind", "?"), command_status.text])
			return
		# Queued is not accepted, and a gather accepted for any resource is
		# not this one: the feedback revision must advance past the click's
		# baseline with an accept, then every commanded worker's task must
		# name this resource's own id.
		if not await _wait_until(_accepted_since.bind(feedback_before), 10.0,
				"gather queued on resource %d was not accepted by the sim" % resource_id):
			return
		for worker_id in controller.selected_ids:
			var task: Dictionary = GrusBridge.unit_task_snapshot(worker_id)
			if int(task.get("source", -1)) != resource_id:
				_fail("right-click on resource %d tasked worker %d with %s" % [
					resource_id, worker_id, task])
				return
		gathered[str(resource.get_meta("resource_kind", "?"))] = resource_id
	for kind in resource_models:
		if not gathered.has(kind):
			_fail("no %s resource art accepted a targeted gather pick" % kind)
			return
	# Restore the prior zoom through real input: later legs unproject
	# targets that only land inside the viewport at this camera size.
	for _step in range(16):
		if camera.size >= gather_zoom_start:
			break
		_mouse_click(Vector2(640.0, 360.0), MOUSE_BUTTON_WHEEL_DOWN)
		await get_tree().process_frame
	if camera.size != gather_zoom_start:
		_fail("wheel zoom never restored the pre-gather camera size: %s" % camera.size)
		return

	var start := unit_one.global_position
	var target := Vector3(30.5, 0.0, 48.5)
	_mouse_click(camera.unproject_position(target), MOUSE_BUTTON_RIGHT)

	for _frame in range(80):
		await get_tree().physics_frame

	if unit_one.global_position.distance_to(start) <= 0.5:
		_fail("right-click input did not move unit 1 through ECS")
		return

	_modifier_event(KEY_SHIFT, true)
	await get_tree().process_frame
	_mouse_click(camera.unproject_position(unit_two.global_position), MOUSE_BUTTON_LEFT, true)
	await get_tree().process_frame
	_modifier_event(KEY_SHIFT, false)
	await get_tree().process_frame
	if not _ring(unit_one).visible or not _ring(unit_two).visible:
		_fail("Shift-click did not add a second friendly unit after camera pan/zoom")
		return

	_modifier_event(KEY_CTRL, true)
	await get_tree().process_frame
	_key_tap(KEY_1, true)
	await get_tree().process_frame
	_modifier_event(KEY_CTRL, false)
	await get_tree().process_frame
	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if _ring(unit_one).visible or _ring(unit_two).visible:
		_fail("clicking outside friendly units did not clear selection")
		return
	_key_tap(KEY_1)
	await get_tree().process_frame
	if not _ring(unit_one).visible or not _ring(unit_two).visible:
		_fail("control-group recall did not restore selected stable IDs after camera pan/zoom")
		return

	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	var screen_two := camera.unproject_position(unit_two.global_position)
	var screen_three := camera.unproject_position(unit_three.global_position)
	var box_start := Vector2(minf(screen_two.x, screen_three.x) - 10.0, minf(screen_two.y, screen_three.y) - 10.0)
	var box_end := Vector2(maxf(screen_two.x, screen_three.x) + 10.0, maxf(screen_two.y, screen_three.y) + 10.0)
	_mouse_drag(box_start, box_end, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_two).visible or not _ring(unit_three).visible:
		_fail("box selection did not include projected friendly units after camera pan/zoom")
		return

	_mouse_click(camera.unproject_position(unit_two.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	var stop_target := Vector3(40.5, 0.0, 44.5)
	_mouse_click(camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	for _frame in range(8):
		await get_tree().physics_frame
	_key_tap(KEY_S)
	for _frame in range(2):
		await get_tree().physics_frame
	var stopped_position := unit_two.global_position
	for _frame in range(15):
		await get_tree().physics_frame
	if unit_two.global_position.distance_to(stopped_position) > 0.05:
		_fail("Stop input did not settle the active move order")
		return

	var input_shield := get_node_or_null("Main/HUD/InputShield") as Control
	if input_shield == null:
		_fail("HUD input shield is missing")
		return
	var feedback_before_ui := int(GrusBridge.command_feedback_revision())
	_mouse_click(input_shield.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_two).visible or int(GrusBridge.command_feedback_revision()) != feedback_before_ui:
		_fail("HUD click leaked into world controls")
		return

	# ResearchPanel sits below InputShield's band; its STOP filter must
	# likewise keep panel clicks from clearing selection or queuing orders.
	var research_panel_leak := get_node_or_null("Main/HUD/ResearchPanel") as Panel
	if research_panel_leak == null:
		_fail("research HUD panel is missing")
		return
	_mouse_click(research_panel_leak.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_two).visible or int(GrusBridge.command_feedback_revision()) != feedback_before_ui:
		_fail("research panel click leaked into world controls")
		return

	# A genuinely blocked destination: the center of a bridge-reported ruin
	# rectangle (quad 4, west of the city) — real impassable terrain, never
	# the friendly Research Center, and its projection lands on open HUD.
	if ruin_count < 5:
		_fail("obstacle_rects returned %d ruin rect(s); the blocked-destination probe indexes quad 4" % ruin_count)
		return
	var ruin_min := Vector2i(ruin_rects[16], ruin_rects[17])
	var ruin_max := Vector2i(ruin_rects[18], ruin_rects[19])
	var ruin_cells := ruin_max - ruin_min + Vector2i.ONE
	var blocked_target := Vector3(
		ruin_min.x + ruin_cells.x * 0.5, 0.0, ruin_min.y + ruin_cells.y * 0.5)
	_mouse_click(camera.unproject_position(blocked_target), MOUSE_BUTTON_RIGHT)
	for _frame in range(3):
		await get_tree().physics_frame
	await get_tree().process_frame
	if not command_status.text.to_lower().contains("unreachable"):
		_fail("blocked destination did not surface unreachable feedback: %s" % command_status.text)
		return

	var cadence_start := unit_three.global_position
	if not GrusBridge.move_units(PackedInt32Array([3]), Vector2(36.5, 44.5)):
		_fail("20 Hz cadence probe move was rejected")
		return
	var cadence_started_ms := Time.get_ticks_msec()
	var previous_visual := unit_three.global_position
	var max_visual_step := 0.0
	var visual_changes := 0
	while Time.get_ticks_msec() - cadence_started_ms < 1000:
		await get_tree().process_frame
		var visual_position := unit_three.global_position
		var visual_step := visual_position.distance_to(previous_visual)
		if visual_step > 0.0001:
			visual_changes += 1
			max_visual_step = maxf(max_visual_step, visual_step)
		previous_visual = visual_position

	var cadence_distance := unit_three.global_position.distance_to(cadence_start)
	if cadence_distance < 4.0 or cadence_distance > 8.0:
		_fail("authoritative simulation is not running near 20 Hz: speed-6 unit moved %.2f units in one second" % cadence_distance)
		return
	# 0.25 is about 42 ms of a speed-6 unit's travel; healthy runs peak at
	# 0.1-0.2. Frame hitches (and the catch-up that follows them) push it over
	# without any interpolation fault: with the host CPU saturated, 12 of 13
	# runs that reached this check failed it. Check host load before
	# debugging interpolation.
	if max_visual_step >= 0.25:
		_fail("20 Hz presentation is not interpolated: max visual step %.3f across %d visual changes" % [max_visual_step, visual_changes])
		return

	# A human scout — worker 4, untouched by the choreography above — reveals
	# an expansion resource, then leaves its vision. Fog tiles and explored
	# resources follow the scout, while no aliens or enemy markers exist
	# before wave 1. Alien visibility is covered by the dedicated wave smoke.
	if not GrusBridge.set_sim_speed(20.0):
		_fail("failed to accelerate the sim for the scouting leg")
		return
	var main := get_node("Main") as Node3D
	var minimap := get_node("Main/HUD/Minimap") as Control

	# The expansion target derives from its live view so the choreography
	# tracks the authored layout instead of a copied coordinate list.
	var salvage_cell := Vector2i(int(salvage_view.global_position.x), int(salvage_view.global_position.z))
	# March beside the expansion ruin (its own cell is blocked), then back
	# toward the live home base so its explored fog remains out of vision.
	var salvage_vantage := Vector2(salvage_view.global_position.x + 1.5, salvage_view.global_position.z + 1.5)
	var home := _building_view(1)
	if home == null:
		_fail("home Research Center view is missing before the scout leg")
		return
	var home_cell := Vector2(home.global_position.x - 18.5, home.global_position.z - 3.5)

	# Pre-scout baseline: the expansion salvage remains Unexplored and hidden;
	# no wave has launched and the fog-gated enemy marker set stays empty.
	var pre_snap: Dictionary = GrusBridge.visibility_snapshot()
	var pre_states: PackedInt32Array = pre_snap.get("states", PackedInt32Array())
	if pre_states.size() != 128 * 96:
		_fail("visibility snapshot payload is not 128x96: %d" % pre_states.size())
		return
	if _cell_state(pre_states, salvage_cell) != 0:
		_fail("expansion salvage did not stay Unexplored before the leg")
		return
	if not _hidden(_resource_view(EXPANSION_SALVAGE_ID)):
		_fail("expansion salvage view was not hidden before the scout leg")
		return
	var wave_error := SmokeHelpers.no_launched_wave_error("general smoke before scout")
	if not wave_error.is_empty():
		_fail(wave_error)
		return
	if int(minimap.enemy_marker_count) != 0:
		_fail("minimap drew alien markers before wave 1")
		return
	if int(minimap.friendly_marker_count) < 4:
		_fail("minimap lost friendly markers")
		return

	# The discrete tile overlay itself: buffers must mirror the fog states
	# exactly and stay explicitly non-interpolated under the project's
	# global physics interpolation — see _assert_fog_overlay.
	var fog_snap_pre := await _settled_fog_snapshot("pre-scout")
	if fog_snap_pre.is_empty() or not _assert_fog_overlay("pre-scout", fog_snap_pre):
		return

	# Scout leg 1: reveal the expansion salvage.
	if not await _move_scout(4, salvage_vantage, "salvage scout"):
		return
	if not await _wait_until(func(): return _shown(_resource_view(EXPANSION_SALVAGE_ID)), 60.0,
			"scout never revealed the expansion salvage"):
		return

	# Reveal transition: the compacted buffers must track the live states
	# with no lerped tiles; the check samples the overlay's settled revision.
	var fog_snap_revealed := await _settled_fog_snapshot("revealed")
	if fog_snap_revealed.is_empty() or not _assert_fog_overlay("revealed", fog_snap_revealed):
		return

	# March home — vision shrinks and exploration persists.
	if not await _move_scout(4, home_cell, "return march"):
		return
	if not await _wait_until(func():
			var snap: Dictionary = GrusBridge.visibility_snapshot()
			var states: PackedInt32Array = snap.get("states", PackedInt32Array())
			return states.size() == 128 * 96 and _cell_state(states, salvage_cell) == 1,
			60.0, "the scouted expansion salvage never returned to Explored"):
		return
	if not _shown(_resource_view(EXPANSION_SALVAGE_ID)):
		_fail("explored expansion salvage lost its persistent view after the scout left")
		return
	for _frame in 2:
		await get_tree().process_frame
	if int(minimap.enemy_marker_count) != 0:
		_fail("minimap drew alien markers before wave 1")
		return
	# The fog data agrees: scouted cells keep Explored instead of snapping
	# back to Unexplored.
	var fog_snap: Dictionary = GrusBridge.visibility_snapshot()
	var fog_states: PackedInt32Array = fog_snap.get("states", PackedInt32Array())
	if fog_states.size() != 128 * 96:
		_fail("visibility snapshot lost its payload")
		return
	if _cell_state(fog_states, salvage_cell) != 1:
		_fail("explored resource fog did not persist after the scout left")
		return

	# Hide transition: exploration persists as state 1 tiles — same exact
	# buffer contract on the other side of the reveal.
	var fog_snap_hidden := await _settled_fog_snapshot("hidden")
	if fog_snap_hidden.is_empty() or not _assert_fog_overlay("hidden", fog_snap_hidden):
		return

	# Minimap click recenters the camera and never queues a command.
	var feedback_revision := int(GrusBridge.command_feedback_revision())
	var click_at := minimap.get_global_rect().position + Vector2(20, 80)
	var press := InputEventMouseButton.new()
	press.button_index = MOUSE_BUTTON_LEFT
	press.position = click_at
	press.pressed = true
	# push_input drives the same Window->Viewport GUI routing an OS click
	# takes; parse_input_event's buffered path is unreliable at this depth in
	# headless runs.
	get_tree().root.push_input(press)
	var release := InputEventMouseButton.new()
	release.button_index = MOUSE_BUTTON_LEFT
	release.position = click_at
	release.pressed = false
	get_tree().root.push_input(release)
	await get_tree().process_frame
	await get_tree().process_frame
	# The camera is pitched down: the click must center the *viewed* ground
	# cell — where the view axis hits y=0 — computed from the logical
	# transform because `project_ray_*` read the physics-interpolated one.
	var forward := -camera.global_transform.basis.z.normalized()
	var ground_hit := camera.global_position + forward * (-camera.global_position.y / forward.y)
	if not Vector2(ground_hit.x, ground_hit.z).is_equal_approx(Vector2(20.5, 80.5)):
		_fail("minimap click did not center the clicked ground cell: %s" % [ground_hit])
		return
	if int(GrusBridge.command_feedback_revision()) != feedback_revision:
		_fail("minimap click leaked a gameplay command")
		return

	if not GrusBridge.set_sim_speed(1.0):
		_fail("failed to restore sim speed")
		return

	# Defense placement runs through the visible build buttons and real world
	# input. Each successful Wall/Gate click stays armed for the next site.
	_mouse_click(camera.unproject_position(unit_one.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_one).visible:
		_fail("defense placement smoke could not select Worker 1")
		return
	var building_catalogue: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {})
	for listed_kind in ["Wall", "Gate", "GunTurret"]:
		var entry: Dictionary = building_catalogue.get(listed_kind, {})
		var catalogue_button := get_node("Main/HUD/CommandPanel/BuildGrid/%sButton" % listed_kind) as Button
		if entry.is_empty() or catalogue_button == null:
			_fail("%s build button/catalogue entry is missing" % listed_kind)
			return
		if not catalogue_button.tooltip_text.contains("Build:") \
				or not catalogue_button.tooltip_text.contains(str(int(entry.get("scrap", 0)))):
			_fail("%s build button does not display its catalogue cost/time: %s" % [listed_kind, catalogue_button.tooltip_text])
			return

	# Queue an ordinary building, then choose another build mode before the
	# authoritative result arrives. The older acceptance must not replace the
	# player's newer mode or its status text.
	var bunkhouse_button := get_node("Main/HUD/CommandPanel/BuildGrid/BunkhouseButton") as Button
	if not await _arm_defense(main, bunkhouse_button, "Bunkhouse"):
		return
	var stale_anchor := _defense_placement_anchor(1, "Bunkhouse")
	if stale_anchor.x < 0:
		_fail("could not find a valid Bunkhouse anchor for the stale-response probe")
		return
	var buildings_before_stale := get_tree().get_nodes_in_group("building_views").size()
	SmokeHelpers.push_mouse_click(get_tree(),
		camera.unproject_position(Vector3(stale_anchor.x + 0.25, 0.0, stale_anchor.y + 0.25)),
		MOUSE_BUTTON_LEFT)
	if str(main.get("_placement_kind")) != "Bunkhouse":
		_fail("ordinary placement cancelled before its authoritative response")
		return
	var wall_button := get_node("Main/HUD/CommandPanel/BuildGrid/WallButton") as Button
	SmokeHelpers.push_mouse_click(get_tree(), wall_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	var wall_selection_status := command_status.text
	if str(main.get("_placement_kind")) != "Wall" or not wall_selection_status.begins_with("Placing Wall"):
		_fail("the newer Wall selection did not replace the pending Bunkhouse placement")
		return
	if not await _wait_until(
			func(): return get_tree().get_nodes_in_group("building_views").size() == buildings_before_stale + 1 \
				and main.get("_pending_placement_requests").is_empty(),
			10.0, "queued Bunkhouse was not authoritatively accepted"):
		return
	if str(main.get("_placement_kind")) != "Wall" or command_status.text != wall_selection_status:
		_fail("the old Bunkhouse acceptance overrode the newer Wall selection")
		return

	# With no later selection, an ordinary placement remains armed while
	# queued, then cancels only after its authoritative acceptance.
	if not await _arm_defense(main, bunkhouse_button, "Bunkhouse"):
		return
	var one_shot_anchor := _defense_placement_anchor(1, "Bunkhouse")
	if one_shot_anchor.x < 0:
		_fail("could not find a second valid Bunkhouse anchor for the one-shot probe")
		return
	var buildings_before_one_shot := get_tree().get_nodes_in_group("building_views").size()
	SmokeHelpers.push_mouse_click(get_tree(),
		camera.unproject_position(Vector3(one_shot_anchor.x + 0.25, 0.0, one_shot_anchor.y + 0.25)),
		MOUSE_BUTTON_LEFT)
	if str(main.get("_placement_kind")) != "Bunkhouse":
		_fail("ordinary Bunkhouse placement cancelled before authoritative acceptance")
		return
	if not await _wait_until(
			func(): return get_tree().get_nodes_in_group("building_views").size() == buildings_before_one_shot + 1 \
				and main.get("_pending_placement_requests").is_empty() \
				and str(main.get("_placement_kind")) == "",
			10.0, "ordinary Bunkhouse placement did not cancel after authoritative acceptance"):
		return

	for defense_kind in ["Wall", "Gate"]:
		var build_button := get_node("Main/HUD/CommandPanel/BuildGrid/%sButton" % defense_kind) as Button
		if not await _arm_defense(main, build_button, defense_kind):
			return
		for _placement in range(2):
			var anchor := _defense_placement_anchor(1, defense_kind)
			if anchor.x < 0:
				_fail("could not find a valid %s anchor near the live Worker view" % defense_kind)
				return
			if not await _place_defense_at(main, camera, defense_kind, anchor):
				return
		if defense_kind == "Wall":
			var cancel_anchor := _defense_placement_anchor(1, defense_kind)
			if cancel_anchor.x < 0:
				_fail("could not find open ground for the right-click cancellation probe")
				return
			var status_before_right_click := command_status.text
			var revision_before_right_click := int(GrusBridge.command_feedback_revision())
			SmokeHelpers.push_mouse_click(get_tree(),
				camera.unproject_position(Vector3(cancel_anchor.x + 0.25, 0.0, cancel_anchor.y + 0.25)),
				MOUSE_BUTTON_RIGHT)
			for _frame in range(5):
				await get_tree().physics_frame
			if str(main.get("_placement_kind")) != "" \
					or int(GrusBridge.command_feedback_revision()) != revision_before_right_click \
					or command_status.text != status_before_right_click:
				_fail("right-click while placing did not cancel without a contextual command")
				return

	# HPA-291 pilot: an unfinished building renders the generic construction
	# site until Rust pushes complete — the freshest Gate (8s build) is
	# still mid-construction here, so its model must be the site GLB.
	var freshest_gate: Node3D = null
	for node in get_tree().get_nodes_in_group("building_views"):
		if str(node.get_meta("building_kind", "")) == "Gate":
			freshest_gate = node as Node3D
	if freshest_gate == null:
		_fail("placed Gate view is missing for the construction-site check")
		return
	if not await _wait_until(func():
			return freshest_gate.get_node_or_null("Model") != null, 5.0,
			"placed Gate view never attached its construction model"):
		return
	var freshest_gate_id := int(freshest_gate.get_meta("building_id"))
	var gate_model := freshest_gate.get_node("Model") as Node3D
	if bool(GrusBridge.building_snapshot(freshest_gate_id).get("complete", false)) \
			or not gate_model.scene_file_path.ends_with("construction_site.glb"):
		_fail("unfinished Gate does not show the generic construction site: %s" % [
			gate_model.scene_file_path])
		return

	# Each preview is affordable against the current stockpile. Queue one
	# turret for another worker, then submit the visible-button placement before
	# the next fixed tick. The first spend makes the second command fail with
	# InsufficientResources, and the rejected ordinary mode must stay armed.
	var gun_turret_button := get_node("Main/HUD/CommandPanel/BuildGrid/GunTurretButton") as Button
	if not await _arm_defense(main, gun_turret_button, "GunTurret"):
		return
	var turret_cost := int(building_catalogue.get("GunTurret", {}).get("scrap", 0))
	var current_scrap := int(GrusBridge.economy_snapshot().get("scrap", 0))
	if turret_cost <= 0 or current_scrap < turret_cost or current_scrap >= turret_cost * 2:
		_fail("delayed rejection probe expected stock for one turret only: scrap=%d cost=%d" % [current_scrap, turret_cost])
		return
	var spending_anchor := _defense_placement_anchor(2, "GunTurret")
	if spending_anchor.x < 0:
		_fail("could not find a valid pre-spend GunTurret anchor for Worker 2")
		return
	var spending_preview: Dictionary = GrusBridge.placement_preview(2, "GunTurret", spending_anchor.x, spending_anchor.y)
	var reserved_footprints: Array[Rect2i] = [Rect2i(
		Vector2i(int(spending_preview.get("anchor_x", spending_anchor.x)), int(spending_preview.get("anchor_y", spending_anchor.y))),
		Vector2i(int(spending_preview.get("width", 1)), int(spending_preview.get("height", 1))))]
	var rejected_anchor := _defense_placement_anchor(1, "GunTurret", reserved_footprints)
	if rejected_anchor.x < 0:
		_fail("could not find a non-overlapping valid GunTurret anchor for the delayed rejection probe")
		return
	var rejected_preview: Dictionary = GrusBridge.placement_preview(1, "GunTurret", rejected_anchor.x, rejected_anchor.y)
	if not bool(spending_preview.get("valid", false)) or not bool(rejected_preview.get("valid", false)):
		_fail("both turret previews must be valid before the queued spend executes")
		return
	var buildings_before_delayed_reject := get_tree().get_nodes_in_group("building_views").size()
	if not GrusBridge.place_building(2, "GunTurret", spending_anchor.x, spending_anchor.y):
		_fail("pre-spend GunTurret command was not queued")
		return
	var feedback_before_delayed_reject := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(),
		camera.unproject_position(Vector3(rejected_anchor.x + 0.25, 0.0, rejected_anchor.y + 0.25)),
		MOUSE_BUTTON_LEFT)
	if str(main.get("_placement_kind")) != "GunTurret" \
			or int(GrusBridge.command_feedback_revision()) != feedback_before_delayed_reject:
		_fail("ordinary placement did not remain armed while awaiting delayed rejection")
		return
	if not await _wait_until(
			func(): return get_tree().get_nodes_in_group("building_views").size() == buildings_before_delayed_reject + 1 \
				and main.get("_pending_placement_requests").is_empty() \
				and command_status.text == "Command rejected (InsufficientResources)",
			10.0, "preview-valid GunTurret placement was not retained after authoritative resource rejection"):
		return
	if str(main.get("_placement_kind")) != "GunTurret":
		_fail("authoritative placement rejection cancelled the ordinary GunTurret mode")
		return

	var phase_before_escape := str(GrusBridge.session_snapshot().get("phase", ""))
	var status_before_escape := command_status.text
	var revision_before_escape := int(GrusBridge.command_feedback_revision())
	_key_tap(KEY_ESCAPE)
	for _frame in range(3):
		await get_tree().physics_frame
	if str(main.get("_placement_kind")) != "" \
			or phase_before_escape != "Playing" \
			or str(GrusBridge.session_snapshot().get("phase", "")) != "Playing" \
			or not (get_node("Main/HUD/PauseButton") as Button).visible \
			or (get_node("Main/HUD/SessionPanel") as Control).visible \
			or int(GrusBridge.command_feedback_revision()) != revision_before_escape \
			or command_status.text != status_before_escape:
		_fail("Esc did not cancel Gate placement before pausing or dispatching a command")
		return

	# Static research catalogue: the panel's own target, the serialized
	# four-currency Analyze price and the per-conversion point reward.
	var research_catalogue: Dictionary = GrusBridge.catalogue_snapshot().get("research", {})
	var target_points := int(research_catalogue.get("target_points", 0))
	var analyze_cost: Dictionary = research_catalogue.get("analyze_cost", {})
	var analyze_gain := int(research_catalogue.get("analyze_gain_points", 0))
	if target_points <= 0 or int(analyze_cost.get("crystals", 0)) <= 0 or analyze_gain <= 0:
		_fail("catalogue research block is incomplete: %s" % [research_catalogue])
		return
	if not analyze_button.text.contains(str(int(analyze_cost.get("crystals", -1)))) \
			or not analyze_button.text.contains(str(analyze_gain)):
		_fail("Analyze button does not render the catalogue cost/gain: %s" % [analyze_button.text])
		return

	# The two advanced towers fill the last cells of the 3x3 BuildGrid and
	# bind their catalogue unlock thresholds; every other kind binds zero.
	var build_grid := get_node("Main/HUD/CommandPanel/BuildGrid") as GridContainer
	if build_grid.columns != 3 or build_grid.get_child_count() != 9:
		_fail("BuildGrid is not the 3x3 layout with the two advanced towers: cols=%d children=%d" % [build_grid.columns, build_grid.get_child_count()])
		return
	for tower_kind in ["FlameTower", "CrystalLance"]:
		var tower_button := get_node("Main/HUD/CommandPanel/BuildGrid/%sButton" % tower_kind) as Button
		var tower_entry: Dictionary = building_catalogue.get(tower_kind, {})
		var unlock := int(tower_entry.get("unlock_points", 0))
		if tower_button == null or tower_entry.is_empty() \
				or unlock <= 0 or unlock >= target_points:
			_fail("%s button/catalogue unlock metadata is missing or malformed: %s" % [tower_kind, tower_entry])
			return
		if int(tower_button.get_meta("unlock_points", -1)) != unlock \
				or not tower_button.tooltip_text.contains(str(unlock)):
			_fail("%s button does not present its catalogue unlock threshold: meta=%s tooltip=%s" % [tower_kind, tower_button.get_meta("unlock_points", "?"), tower_button.tooltip_text])
			return
	if int(building_catalogue.get("Wall", {}).get("unlock_points", -1)) != 0 \
			or int(building_catalogue.get("CrystalLance", {}).get("unlock_points", 0)) \
				<= int(building_catalogue.get("FlameTower", {}).get("unlock_points", 0)):
		_fail("catalogue unlock thresholds are not the always-zero / 25-before-50 contract: %s" % [
			{"Wall": building_catalogue.get("Wall", {}), "Flame": building_catalogue.get("FlameTower", {}), "Lance": building_catalogue.get("CrystalLance", {})}])
		return

	# The dynamic surface is exactly the three live fields — no milestone
	# flags, costs or independent unlock state may leak beside them.
	var research: Dictionary = GrusBridge.research_snapshot()
	if research.size() != 3 or not research.has("points") \
			or not research.has("rate_per_second") or not research.has("eta_seconds"):
		_fail("research_snapshot surface is not exactly points/rate_per_second/eta_seconds: %s" % [research])
		return
	# Raw Variant types first: a string payload would silently coerce in the
	# float() reads below, so every live field must arrive numeric and finite
	# before any conversion touches it.
	for field in ["points", "rate_per_second", "eta_seconds"]:
		var raw: Variant = research[field]
		if typeof(raw) != TYPE_FLOAT and typeof(raw) != TYPE_INT:
			_fail("research_snapshot %s is not a numeric Variant: %s" % [field, research])
			return
		if is_nan(raw) or is_inf(raw):
			_fail("research_snapshot %s is not finite: %s" % [field, research])
			return
	var live_points := float(research.get("points", -1.0))
	var live_rate := float(research.get("rate_per_second", -1.0))
	var live_eta := float(research.get("eta_seconds", -2.0))
	if live_points < 0.0 or live_rate <= 0.0 or live_eta < 0.0:
		_fail("Playing research snapshot is not numeric/positive: %s" % [research])
		return
	# Lock presentation is a per-frame rule: every build button is disabled
	# exactly while the live points fall short of its catalogue threshold.
	for grid_button in build_grid.get_children():
		var required := float(grid_button.get_meta("unlock_points", 0))
		var want_disabled := required > 0.0 and live_points < required
		if grid_button.disabled != want_disabled:
			_fail("build button %s lock %s mismatches live points %f vs threshold %f" % [grid_button.get_meta("kind"), grid_button.disabled, live_points, required])
			return
	# The bar and label track the live snapshot — points, floor-truncated
	# percent (never an early 100%) and a rendered ETA.
	if absf(research_bar.value - live_points) > 5.0 \
			or not is_equal_approx(research_bar.max_value, float(target_points)):
		_fail("research bar does not track the live snapshot: value=%f points=%f max=%f target=%d" % [research_bar.value, live_points, research_bar.max_value, target_points])
		return
	var pct := floori(live_points / float(target_points) * 100.0)
	if pct >= 100 or not research_label.text.contains("%d%%" % pct) \
			or research_label.text.contains("100%") \
			or not research_label.text.contains("ETA") or research_label.text.ends_with("—"):
		_fail("research label mis-renders live progress: %s (points=%f)" % [research_label.text, live_points])
		return
	# The ETA is a live estimate: the bridge value must shrink as sim time
	# passes, and the rendered countdown must agree with the snapshot it was
	# drawn from — a frozen or wrong label cannot hide behind a healthy
	# bridge field. At 1x ten physics frames move only ~0.17 sim-seconds,
	# below the label's whole-second ceil() granularity, so the displayed
	# value may legitimately hold — it must never tick up or drift beyond
	# one rendered frame of skew plus the label's rounding (±5 s).
	var label_eta_before := SmokeHelpers.research_label_eta_seconds(research_label.text)
	if label_eta_before < 0 or absi(label_eta_before - int(ceilf(live_eta))) > 5:
		_fail("research label ETA does not render the live snapshot: %s (eta=%f)" % [research_label.text, live_eta])
		return
	for _frame in 10:
		await get_tree().physics_frame
	var eta_after := float(GrusBridge.research_snapshot().get("eta_seconds", -1.0))
	var label_eta_after := SmokeHelpers.research_label_eta_seconds(research_label.text)
	if eta_after < 0.0 or eta_after >= live_eta:
		_fail("research ETA did not decrease over ten sim ticks: %f -> %f" % [live_eta, eta_after])
		return
	if label_eta_after < 0 or label_eta_after > label_eta_before \
			or absi(label_eta_after - int(ceilf(eta_after))) > 5:
		_fail("research label ETA drifted from the live snapshot: %f -> %f (label %d -> %d)" % [
			live_eta, eta_after, label_eta_before, label_eta_after])
		return

	# Real input dispatch: recenter the camera on the live Research Center
	# through the minimap's own click path — the earlier minimap leg left
	# the view on the southwest corner, where the core projects under the
	# HUD panels and swallows any world click. The click lands on the live
	# view's world X/Z (the minimap maps 1 px per cell), so a fixture
	# relocation cannot strand it on a stale coordinate.
	var rc_view := _building_view(1)
	if rc_view == null:
		_fail("live Research Center view is missing before the Analyze leg")
		return
	var feedback_before_minimap := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(),
		minimap.get_global_rect().position + Vector2(rc_view.global_position.x, rc_view.global_position.z),
		MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	if int(GrusBridge.command_feedback_revision()) != feedback_before_minimap:
		_fail("minimap recenter for the Analyze leg leaked a command")
		return
	# Then march every surviving Team-1 unit onto a proven-walkable spot far
	# from the Research Center — wave casualties have no view to clear —
	# and wait for sim acceptance plus actual arrival. A settled unit
	# cannot drift back into the building's 20px pick radius between the
	# wait and the click.
	var clear_ids := PackedInt32Array()
	for unit_node in get_tree().get_nodes_in_group("unit_views"):
		var unit_view := unit_node as Node3D
		if unit_view != null and int(unit_view.get_meta("team_id", -1)) == 1:
			clear_ids.append(int(unit_view.get_meta("unit_id")))
	if clear_ids.is_empty():
		_fail("no live Team-1 units remain to clear the Research Center pick radius")
		return
	var clear_target := Vector2(36.5, 44.5)
	if not GrusBridge.set_sim_speed(4.0):
		_fail("could not accelerate the worker clear-out for the Analyze leg")
		return
	var clear_revision := int(GrusBridge.command_feedback_revision())
	if not GrusBridge.move_units(clear_ids, clear_target):
		_fail("worker clear-out move was not queued")
		return
	if not await _wait_until(_accepted_since.bind(clear_revision), 10.0,
			"worker clear-out move was not accepted by the sim"):
		return
	var rc_screen := camera.unproject_position(rc_view.global_position)
	if not Rect2(Vector2.ZERO, viewport_size).has_point(rc_screen):
		_fail("Research Center does not project inside the viewport after the minimap recenter: %s" % rc_screen)
		return
	if not await _wait_until(func():
			for moved_node in get_tree().get_nodes_in_group("unit_views"):
				var moved_unit := moved_node as Node3D
				if moved_unit == null or int(moved_unit.get_meta("team_id", -1)) != 1:
					continue
				if moved_unit.global_position.distance_to(
						Vector3(clear_target.x, 0.0, clear_target.y)) > 5.0:
					return false
				if camera.unproject_position(moved_unit.global_position).distance_to(rc_screen) <= 40.0:
					return false
			return true,
			20.0, "live workers never arrived clear of the Research Center"):
		return
	if not GrusBridge.set_sim_speed(1.0):
		_fail("could not restore sim speed after the clear-out")
		return
	# parse_input_event clicks are buffered across iterations — poll the
	# selection outcome rather than guessing a fixed frame count.
	_mouse_click(rc_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == 1 \
				and main.selected_ids.is_empty(),
			5.0, "left-click on the Research Center view did not select building 1"):
		return
	# HPA-291 silhouette picking: a click inside the model's projected art
	# rect — beyond the old 20 px center-radius — must select the same
	# building. The point derives from the live view's active Model bounds
	# (the HealthBar is scoped out of the pick), so it tracks whatever art
	# is attached.
	var rc_model := rc_view.get_node_or_null("Model") as Node3D
	if rc_model == null:
		_fail("Research Center view has no Model for the silhouette pick")
		return
	var rc_art_rect: Rect2 = main.call("_projected_art_rect", rc_view, rc_model)
	var silhouette_farthest := Vector2.ZERO
	var silhouette_distance := -1.0
	for corner in [rc_art_rect.position,
			Vector2(rc_art_rect.end.x, rc_art_rect.position.y), rc_art_rect.end,
			Vector2(rc_art_rect.position.x, rc_art_rect.end.y)]:
		var distance := rc_screen.distance_to(corner)
		if distance > silhouette_distance and Rect2(Vector2.ZERO, viewport_size).has_point(corner):
			silhouette_farthest = corner
			silhouette_distance = distance
	if silhouette_distance <= main.CLICK_RADIUS + 5.0:
		_fail("Research Center art rect %s cannot probe a pick past the old center radius" % rc_art_rect)
		return
	var silhouette_point := rc_screen + (silhouette_farthest - rc_screen) * 0.9
	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func(): return int(main.selected_building_id) == -1,
			5.0, "Research Center did not deselect before the silhouette pick"):
		return
	_mouse_click(silhouette_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == 1 \
				and main.selected_ids.is_empty(),
			5.0, "left-click on the Research Center's projected art did not select building 1"):
		return
	# The enabled state follows the session+selection contract: Playing with
	# the owned, completed Research Center selected — never Crystal cost.
	if not await _wait_until(func(): return not analyze_button.disabled,
			5.0, "Analyze stayed disabled with the owned Research Center selected while Playing"):
		return
	var points_before_click := float(GrusBridge.research_snapshot().get("points", -1.0))
	var crystals_before := int(GrusBridge.economy_snapshot().get("crystals", -1))
	var analyze_price := int(analyze_cost.get("crystals", 0))
	var analyze_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), analyze_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	# Dispatch evidence is the sim's own verdict: the controller's transient
	# "Analyze queued" text is overwritten by command_feedback() on the
	# revision bump. Affordability decides which verdict — either way the
	# press reached Rust through the controller and the economy/research
	# ledgers moved exactly as the catalogue price and gain dictate.
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > analyze_revision,
			10.0, "Analyze button press produced no sim verdict"):
		return
	var crystals_after := int(GrusBridge.economy_snapshot().get("crystals", -1))
	var points_after := float(GrusBridge.research_snapshot().get("points", -1.0))
	if crystals_before < analyze_price:
		# Unaffordable: the existing typed reject, code 8, spends nothing
		# and banks no point jump.
		if int(GrusBridge.last_reject_code()) != 8 \
				or str(GrusBridge.command_feedback()) != "Command rejected (InsufficientResources)":
			_fail("unaffordable Analyze did not surface the typed InsufficientResources reject: code=%d feedback=%s" % [int(GrusBridge.last_reject_code()), GrusBridge.command_feedback()])
			return
		if crystals_after != crystals_before:
			_fail("rejected Analyze changed the Crystal ledger: %d -> %d" % [crystals_before, crystals_after])
			return
		if points_after - points_before_click >= float(analyze_gain):
			_fail("rejected Analyze banked the point reward: before=%f now=%f" % [points_before_click, points_after])
			return
	else:
		# Affordable: the conversion lands — exactly the catalogue price
		# leaves the ledger (the no-launched-wave sentinel below already
		# pins this leg free of kill rewards) and the banked points jump by
		# the catalogue gain (passive accrual only ever adds more on top).
		if not str(GrusBridge.command_feedback()).begins_with("Command accepted"):
			_fail("affordable Analyze was not accepted: %s" % GrusBridge.command_feedback())
			return
		if crystals_after != crystals_before - analyze_price:
			_fail("accepted Analyze did not spend exactly the catalogue price: %d -> %d (price %d)" % [crystals_before, crystals_after, analyze_price])
			return
		if points_after - points_before_click < float(analyze_gain):
			_fail("accepted Analyze did not bank the +%d reward: before=%f now=%f" % [analyze_gain, points_before_click, points_after])
			return
	# Deselect on open ground: Analyze disables again and a click on the
	# disabled button dispatches nothing to the bridge.
	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == -1 \
				and analyze_button.disabled,
			5.0, "Analyze did not disable after the Research Center was deselected"):
		return
	var revision_before_disabled := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), analyze_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	for _frame in 6:
		await get_tree().process_frame
	if int(GrusBridge.command_feedback_revision()) != revision_before_disabled:
		_fail("a click on the disabled Analyze button reached the bridge")
		return

	var pilot_bunkhouse: Node3D = null
	var pilot_bunkhouse_id := -1
	var resume_bunkhouse: Node3D = null
	var resume_bunkhouse_id := -1
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		if str(view.get_meta("building_kind", "")) != "Bunkhouse":
			continue
		var id := int(view.get_meta("building_id", -1))
		if bool(GrusBridge.building_snapshot(id).get("complete", false)):
			pilot_bunkhouse = view
			pilot_bunkhouse_id = id
			break
		resume_bunkhouse = view
		resume_bunkhouse_id = id
	if pilot_bunkhouse == null and resume_bunkhouse != null:
		if not GrusBridge.resume_construction(1, resume_bunkhouse_id):
			_fail("resume_construction for the roof-pick Bunkhouse was not queued")
			return
		if not GrusBridge.set_sim_speed(20.0):
			_fail("could not accelerate the Bunkhouse completion for the roof pick")
			return
		if not await _wait_until(func():
				return bool(GrusBridge.building_snapshot(resume_bunkhouse_id).get("complete", false)),
				20.0, "roof-pick Bunkhouse never completed after resume_construction"):
			return
		if not GrusBridge.set_sim_speed(1.0):
			_fail("could not restore sim speed after the Bunkhouse completion")
			return
		pilot_bunkhouse = resume_bunkhouse
		pilot_bunkhouse_id = resume_bunkhouse_id
	if pilot_bunkhouse == null:
		_fail("no placed Bunkhouse exists for the roof silhouette pick")
		return
	var bunkhouse_model := pilot_bunkhouse.get_node_or_null("Model") as Node3D
	if bunkhouse_model == null or not bunkhouse_model.scene_file_path.ends_with("bunkhouse.glb"):
		_fail("completed Bunkhouse does not carry the pilot GLB for the roof silhouette pick")
		return
	# The 2x2 cabin is too small at the default framing to probe a point
	# past the old 20 px center radius, so frame it like the resource-pick
	# leg: minimap-recenter the view onto it and wheel-zoom to the minimum
	# size. Both are the real input paths the game itself handles.
	var bunkhouse_zoom_start := camera.size
	var bunkhouse_world := pilot_bunkhouse.global_position
	var viewport_center := viewport_size * 0.5
	var minimap_point := minimap.get_global_rect().position + Vector2(bunkhouse_world.x, bunkhouse_world.z)
	var recenter_press := InputEventMouseButton.new()
	recenter_press.button_index = MOUSE_BUTTON_LEFT
	recenter_press.position = minimap_point
	recenter_press.pressed = true
	get_tree().root.push_input(recenter_press)
	var recenter_release := InputEventMouseButton.new()
	recenter_release.button_index = MOUSE_BUTTON_LEFT
	recenter_release.position = minimap_point
	recenter_release.pressed = false
	get_tree().root.push_input(recenter_release)
	await get_tree().process_frame
	await get_tree().process_frame
	var bunkhouse_screen := camera.unproject_position(pilot_bunkhouse.global_position)
	if bunkhouse_screen.distance_to(viewport_center) > 60.0:
		_fail("minimap recenter never centered the roof-pick Bunkhouse: %s" % bunkhouse_screen)
		return
	for _step in range(16):
		if camera.size <= controller.MIN_CAMERA_SIZE:
			break
		_mouse_click(viewport_center, MOUSE_BUTTON_WHEEL_UP)
		await get_tree().process_frame
	bunkhouse_screen = camera.unproject_position(pilot_bunkhouse.global_position)
	var roof_faces: Array[Array] = []
	for node in bunkhouse_model.find_children("*", "MeshInstance3D", true, false):
		var mesh_instance := node as MeshInstance3D
		if mesh_instance.mesh == null:
			continue
		for surface_index in mesh_instance.mesh.get_surface_count():
			var arrays := mesh_instance.mesh.surface_get_arrays(surface_index)
			var vertices := arrays[Mesh.ARRAY_VERTEX] as PackedVector3Array
			var indices = arrays[Mesh.ARRAY_INDEX]
			var triangle_count := int(indices.size() / 3) if indices != null and not indices.is_empty() else int(vertices.size() / 3)
			for face_index in triangle_count:
				var va := vertices[indices[face_index * 3]] if indices != null and not indices.is_empty() else vertices[face_index * 3]
				var vb := vertices[indices[face_index * 3 + 1]] if indices != null and not indices.is_empty() else vertices[face_index * 3 + 1]
				var vc := vertices[indices[face_index * 3 + 2]] if indices != null and not indices.is_empty() else vertices[face_index * 3 + 2]
				var a: Vector3 = mesh_instance.global_transform * va
				var b: Vector3 = mesh_instance.global_transform * vb
				var c: Vector3 = mesh_instance.global_transform * vc
				if (b - a).cross(c - a).normalized().y <= 0.5:
					continue
				roof_faces.append([a, b, c, (a + b + c) / 3.0])
	if roof_faces.is_empty():
		_fail("Bunkhouse model exposes no upward roof triangle for the silhouette pick")
		return
	roof_faces.sort_custom(func(a, b): return a[3].y > b[3].y)
	var roof_point := Vector2.INF
	for face in roof_faces:
		var farthest_vertex: Vector3 = face[0]
		var farthest_distance := -1.0
		for vertex_index in 3:
			var vertex := face[vertex_index] as Vector3
			var distance := bunkhouse_screen.distance_to(camera.unproject_position(vertex))
			if distance > farthest_distance:
				farthest_distance = distance
				farthest_vertex = vertex
		var interior := (face[3] as Vector3) + (farthest_vertex - (face[3] as Vector3)) * 0.75
		var candidate := camera.unproject_position(interior)
		if not Rect2(Vector2.ZERO, viewport_size).has_point(candidate):
			continue
		if bunkhouse_screen.distance_to(candidate) <= main.CLICK_RADIUS + 5.0:
			continue
		var projected_triangle := PackedVector2Array([
				camera.unproject_position(face[0] as Vector3),
				camera.unproject_position(face[1] as Vector3),
				camera.unproject_position(face[2] as Vector3)])
		if not Geometry2D.is_point_in_polygon(candidate, projected_triangle):
			continue
		if (main.call("_nearest_view", "unit_views", candidate, 1) as Node3D) != null:
			continue
		if main.call("_building_at", candidate, 1) == pilot_bunkhouse:
			roof_point = candidate
			break
	if roof_point == Vector2.INF:
		_fail("Bunkhouse roof mesh exposes %d upward faces but none projects inside the viewport beyond the old center radius" % roof_faces.size())
		return
	_mouse_click(roof_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == pilot_bunkhouse_id \
				and main.selected_ids.is_empty(),
			5.0, "left-click on the Bunkhouse's projected roof did not select its building"):
		return
	# Restore the prior zoom through real input like the gather leg — the
	# reset and any later camera-dependent probes keep their framing.
	for _step in range(16):
		if camera.size >= bunkhouse_zoom_start:
			break
		_mouse_click(viewport_center, MOUSE_BUTTON_WHEEL_DOWN)
		await get_tree().process_frame
	if camera.size != bunkhouse_zoom_start:
		_fail("wheel zoom never restored the pre-pick camera size: %s" % camera.size)
		return

	# HPA-291 pilot state leg: every building view carries a Model — the
	# generic construction site exactly while the snapshot reports
	# incomplete, the finished model once complete (bunkhouse.glb for the
	# pilot kind, the placeholder for unlanded ones). The seeded-complete
	# Research Center guarantees a finished-model proof regardless of how
	# far the construction queue drained before the clear-out move leg;
	# economy_smoke owns the full site→GLB completion-swap journey.
	var model_offender := ["none"]
	if not await _wait_until(func():
			model_offender[0] = "a building view has no Model"
			var finished := 0
			for node in get_tree().get_nodes_in_group("building_views"):
				var view := node as Node3D
				var id := int(view.get_meta("building_id", -1))
				var kind := str(view.get_meta("building_kind", ""))
				var model := view.get_node_or_null("Model") as Node3D
				if model == null:
					model_offender[0] = "%s %d has no Model" % [kind, id]
					return false
				var complete := bool(GrusBridge.building_snapshot(id).get("complete", false))
				var is_site := model.scene_file_path.ends_with("construction_site.glb")
				if complete == is_site:
					model_offender[0] = "%s %d complete=%s but model is %s" % [
						kind, id, complete, model.scene_file_path]
					return false
				if complete and kind == "Bunkhouse" \
						and not model.scene_file_path.ends_with("bunkhouse.glb"):
					model_offender[0] = "completed Bunkhouse %d model is %s" % [
						id, model.scene_file_path]
					return false
				if complete:
					finished += 1
			model_offender[0] = "no completed building views yet"
			return finished >= 1,
			30.0, "building views never satisfied the site/finished model contract"):
		_fail("building-model offender: %s" % model_offender[0])
		return

	if not GrusBridge.reset_fixture():
		_fail("restart after defense placement was rejected")
		return
	if not await _wait_until(
			func(): return get_tree().get_nodes_in_group("unit_views").size() == 4 \
				and get_tree().get_nodes_in_group("building_views").size() == 1 \
				and get_tree().get_nodes_in_group("resource_views").size() == 18 \
				and str(GrusBridge.session_snapshot().get("phase", "")) == "Start",
			10.0, "restart did not restore the one-building fixture after defense placement"):
		return
	# Restart wipes banked research with the match: the dynamic snapshot and
	# the HUD both read zero, and Analyze disables outside Playing.
	if float(GrusBridge.research_snapshot().get("points", -1.0)) != 0.0:
		_fail("restart did not reset research points: %s" % [GrusBridge.research_snapshot()])
		return
	if not await _wait_until(func():
			return research_bar.value == 0.0 and analyze_button.disabled,
			5.0, "restart left stale research HUD progress or an enabled Analyze"):
		return

	wave_error = SmokeHelpers.no_launched_wave_error("general smoke")
	if not wave_error.is_empty():
		_fail(wave_error)
		return
	print("GRUS_GODOT_SMOKE_OK units=4 controls=input-derived camera=pan-zoomed unreachable=feedback cadence=20hz interpolation=max_step_%.3f fog=scout_reveal_hide explored_persist=true markers=fog_gated recenter=command_free pre_wave=true winter_art=gltf_exact_cover_input_gather pilot_structures=site_to_glb silhouette_pick=roof_triangle" % max_visual_step)
	get_tree().quit(0)
