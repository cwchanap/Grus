extends Node

const TEST_VIEWPORT_SIZE := Vector2i(1280, 720)

## Stable id for the fog-scouting leg; positions are read off the live view
## at run time, never copied from the fixture into GDScript.
const EXPANSION_SALVAGE_ID := 14

## HPA-698 framing: the opening orthographic size is closer than the former
## 72.0 default so the real starting base reads larger, and one player
## wheel-zoom must still reach that former coverage. Both are contracts on
## the authored scene, not on any copied fixture coordinates.
const INITIAL_CAMERA_SIZE := 54.0
const PRIOR_CAMERA_SIZE := 72.0

const HudHelpers := preload("res://scripts/hud_helpers.gd")
const SmokeHelpers := preload("res://scripts/smoke_helpers.gd")
## RejectReason discriminant values (declaration order in
## crates/grus-sim/src/commands.rs); the bridge reports "no rejection" as -1.
const REJECT_NOT_COMBATANT := 14

class RetryBuildingView extends "res://scripts/building_view.gd":
	var failed_once := false

	func _attach_model() -> bool:
		if not failed_once:
			failed_once = true
			return false
		return super._attach_model()

func _ready() -> void:
	call_deferred("_run")

func _fail(message: String) -> void:
	push_error(message)
	get_tree().quit(1)

## The pinned RichTextLabel parser turns each cue's `alt="..."` into the
## ROLE_IMAGE accessibility name; the shared checker reads the exact BBCode
## the engine parses. Covers every image tag, not only the four expected cues.
func _controls_cue_alt_error(controls: RichTextLabel, expected: Dictionary) -> String:
	return SmokeHelpers.inline_image_alt_error(controls, expected, "ControlsLabel")

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

## The minimap draws the 128x96 world bitmap across its scaled control rect;
## tests convert a world cell into the real click point the same way a player
## clicking the visible map would.
func _minimap_click_point(minimap: Control, cell: Vector2) -> Vector2:
	return minimap.get_global_rect().position + cell * (minimap.size / Vector2(128.0, 96.0))

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

func _wait_until(condition: Callable, timeout_s: float, message: String, details := Callable()) -> bool:
	var frame_budget := int(ceilf(timeout_s * Engine.physics_ticks_per_second))
	for _frame in frame_budget:
		if condition.call():
			return true
		await get_tree().physics_frame
	var suffix := "" if not details.is_valid() else ": %s" % details.call()
	_fail("%s (waited %.0fs)%s" % [message, timeout_s, suffix])
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

## The view transform can lag a marching unit by several ticks under slow
## windowed rendering, so click-picking a mover is frame-rate sensitive;
## wait for the sim task to settle instead. A mover keeps WorkerTask::Idle,
## so the stationary truth is economy_snapshot's idle set (Worker + Idle +
## no MoveOrder) — the task check alone would clear mid-march.
func _unit_idle(unit: Node3D) -> bool:
	var unit_id := int(unit.get_meta("unit_id", -1))
	if str(GrusBridge.unit_task_snapshot(unit_id).get("task", "")) != "Idle":
		return false
	var idle_workers: PackedInt32Array = GrusBridge.economy_snapshot() \
			.get("idle_worker_ids", PackedInt32Array())
	return idle_workers.has(unit_id)

## A clickable world probe site: the candidate must be a valid placement
## preview, clear of reserved footprints, AND its real click point must not
## land under a HUD STOP surface — the full-width top bar owns the viewport
## top, so a world point projected there can never receive the click.
func _defense_placement_anchor(
		builder_id: int, kind: String, reserved_footprints: Array[Rect2i] = []) -> Vector2i:
	var worker := _unit_view(builder_id)
	if worker == null:
		return Vector2i(-1, -1)
	var camera := get_node("Main/Camera3D") as Camera3D
	var hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
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
				if overlaps_reserved:
					continue
				var screen := camera.unproject_position(Vector3(candidate.x + 0.25, 0.0, candidate.y + 0.25))
				if hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(screen)):
					continue
				return candidate
	return Vector2i(-1, -1)

func _arm_defense(main: Node3D, button: Button, kind: String) -> bool:
	var revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	if int(GrusBridge.command_feedback_revision()) != revision \
			or str(main.get("_placement_kind")) != kind \
			or not button.button_pressed:
		_fail("%s build button leaked a command, did not arm placement, or lost its lit tile: placement=%s pressed=%s revision=%d->%d feedback=%s" % [
			kind, main.get("_placement_kind"), button.button_pressed,
			revision, int(GrusBridge.command_feedback_revision()), GrusBridge.command_feedback()])
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
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	for spec in [
		["HUD/CommandPanel/BuildGrid", "buildings", 9, "Build"],
		["HUD/CommandPanel/TrainGrid", "units", 3, "Train"],
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
			# Icon-only tiles still name their catalogue action: the tooltip
			# leads with the real building/unit and the accessible name repeats
			# the Build/Train verb, so hovering and assistive tech identify it.
			if not button.tooltip_text.begins_with("%s\n" % kind) \
					or button.accessibility_name != "%s %s" % [spec[3], kind]:
				_fail("%s does not name %s %s: tooltip=%s accessibility=%s" % [
					button.name, spec[3], kind, button.tooltip_text, button.accessibility_name])
				return false
			# HPA-698 readability: the painted icon and its cost must fill the
			# tile, not sit tiny inside an oversized rim.
			var cost_label := button.get_node_or_null("Cost") as RichTextLabel
			if button.size.y < 70.0 or button.get_theme_constant("icon_max_width") < 40 \
					or cost_label == null \
					or cost_label.get_theme_font_size("normal_font_size") < 13:
				_fail("%s action tile does not fill its icon/cost slot: size=%s icon_width=%d cost_font=%d" % [
					button.name, button.size, button.get_theme_constant("icon_max_width"),
					cost_label.get_theme_font_size("normal_font_size") if cost_label != null else -1])
				return false
			# The tooltip is the catalogue stat surface: price, time, max HP
			# and any optional combat/counter fields — never a copied table.
			var entry: Dictionary = catalogue.get(spec[1], {}).get(kind, {})
			var counter: TextureRect = button.get_node_or_null("CounterIcon") as TextureRect
			if entry.is_empty() or not button.tooltip_text.contains("Cost:") \
					or not button.tooltip_text.contains("%s: %ds" % [spec[3], int(entry.get("seconds", 0))]) \
					or not button.tooltip_text.contains("HP: %d" % int(entry.get("max_health", 0))):
				_fail("%s tooltip is missing its catalogue cost/time/HP row: %s" % [button.name, button.tooltip_text])
				return false
			if entry.has("damage"):
				if not button.tooltip_text.contains("Damage %d" % int(entry.get("damage", 0))) \
						or not button.tooltip_text.contains("Range") \
						or not button.tooltip_text.contains(str(entry.get("counter_target", ""))) \
						or counter == null or not counter.visible \
						or counter.texture == null \
						or not counter.texture.resource_path.ends_with(
							"units/%s.png" % str(entry.get("counter_target", "")).to_snake_case()):
					_fail("%s tooltip/counter art does not project its catalogue combat row: %s" % [button.name, button.tooltip_text])
					return false
			elif counter != null and counter.visible:
				_fail("%s shows a counter icon without catalogue combat stats" % button.name)
				return false
			if spec[1] == "buildings":
				var heat := int(entry.get("heat_supply", 0)) + int(entry.get("heat_demand", 0))
				if heat > 0 and not button.tooltip_text.contains("Heat:"):
					_fail("%s tooltip omits its catalogue heat contribution" % button.name)
					return false
		if count != int(spec[2]):
			_fail("%s has %d action Buttons; expected %d" % [grid.name, count, int(spec[2])])
			return false
	return true

## The painted texture a StyleBox draws, or "" for a flat plate.
func _plate_texture_path(plate: StyleBox) -> String:
	if plate is StyleBoxTexture and (plate as StyleBoxTexture).texture != null:
		return (plate as StyleBoxTexture).texture.resource_path
	return ""

## Mean sRGB colour of a texture's opaque pixels, sampled every fourth pixel.
## A deliberately conservative icon-presence readout: it averages a glyph's
## dark outlines with its bright body instead of sampling its brightest core.
func _opaque_mean_color(texture: Texture2D) -> Color:
	if texture == null:
		return Color.BLACK
	var image := texture.get_image()
	var sample := Color(0, 0, 0, 0)
	var count := 0
	for y in range(0, image.get_height(), 4):
		for x in range(0, image.get_width(), 4):
			var pixel := image.get_pixel(x, y)
			if pixel.a > 0.5:
				sample += pixel
				count += 1
	if count == 0:
		return Color.BLACK
	return Color(sample.r / count, sample.g / count, sample.b / count, 1.0)

## HPA-698 command-tile clarity: the nine build and three train plates paint
## one quiet flat frame, while each exceptional state keeps the shipped painted
## art — armed the cyan selected frame, true locks the locked frame and
## unaffordable actions the rust frame (still enabled). Focus is a ring drawn
## outside the plate, so it stays visible over every state and is not a
## recolouring of the armed edge. The shared Tile variation must stay painted:
## the underground drawer still reads it. Prices and icons must clear contrast
## on the quieter field, with the same content box in every state.
func _assert_command_tile_treatment() -> bool:
	await get_tree().process_frame
	var theme := load(str(ProjectSettings.get_setting("gui/theme/custom", ""))) as Theme
	if theme == null:
		_fail("command-tile check has no project theme")
		return false
	var normal := theme.get_stylebox("normal", "CommandTile")
	var hover := theme.get_stylebox("hover", "CommandTile")
	var pressed := theme.get_stylebox("pressed", "CommandTile")
	var disabled := theme.get_stylebox("disabled", "CommandTile")
	var focus := theme.get_stylebox("focus", "CommandTile")
	# Theme.get_stylebox falls back to the engine default theme for an unknown
	# type, so only has_stylebox can prove the project defines these plates.
	for state in ["normal", "hover", "pressed", "disabled", "focus"]:
		if not theme.has_stylebox(state, "CommandTile"):
			_fail("command tiles have no quiet CommandTile %s variation" % state)
			return false
	# The repeated field: one dark, low-contrast flat plate per tile instead of
	# the painted ice frame that used to repeat twelve times.
	if not normal is StyleBoxFlat:
		_fail("command normal plate still paints %s art" % normal.get_class())
		return false
	var normal_fill := (normal as StyleBoxFlat).bg_color
	var normal_border := (normal as StyleBoxFlat).border_color
	if SmokeHelpers.linear_luminance(normal_fill) > 0.02 \
			or SmokeHelpers.linear_luminance(normal_border) > 0.2 \
			or (normal as StyleBoxFlat).border_width_left != 1:
		_fail("command normal plate is not quiet: fill=%s border=%s width=%d" % [
			normal_fill, normal_border, (normal as StyleBoxFlat).border_width_left])
		return false
	# Hover must answer the pointer without becoming a second lit frame, and it
	# must not be the armed plate under another name.
	if not hover is StyleBoxFlat \
			or SmokeHelpers.linear_luminance((hover as StyleBoxFlat).bg_color) \
				<= SmokeHelpers.linear_luminance(normal_fill) \
			or SmokeHelpers.linear_luminance((hover as StyleBoxFlat).bg_color) > 0.05 \
			or SmokeHelpers.linear_luminance((hover as StyleBoxFlat).border_color) \
				<= SmokeHelpers.linear_luminance(normal_border):
		_fail("command hover plate is not a quiet lift over normal: %s" % hover)
		return false
	# Exceptional states keep their shipped art, each a different picture, and
	# none of them is the ornate frame the quiet plate replaced.
	var painted := {
		"pressed": _plate_texture_path(pressed),
		"disabled": _plate_texture_path(disabled),
		"unaffordable": _plate_texture_path(theme.get_stylebox("normal", "UnaffordableTile")),
	}
	var expected := {
		"pressed": "res://assets/ui/frostline/ui/tile_selected.png",
		"disabled": "res://assets/ui/frostline/ui/tile_locked.png",
		"unaffordable": "res://assets/ui/frostline/ui/tile_unaffordable.png",
	}
	for state in expected:
		if painted[state] != expected[state]:
			_fail("command %s state paints %s; expected %s" % [
				state, painted[state] if painted[state] != "" else "a flat plate", expected[state]])
			return false
	if painted.values().has("res://assets/ui/frostline/ui/tile_normal.png"):
		_fail("command tiles still paint the repeated ornate tile frame")
		return false
	# An unaffordable tile stays clickable, so it needs its own hover lift and
	# the same armed plate: an armed unaffordable tile must never look idle.
	var unaffordable_hover := theme.get_stylebox("hover", "UnaffordableTile") as StyleBoxTexture
	if unaffordable_hover == null or unaffordable_hover.texture == null \
			or not unaffordable_hover.texture.resource_path.ends_with("tile_unaffordable.png") \
			or SmokeHelpers.linear_luminance(unaffordable_hover.modulate_color) \
				<= SmokeHelpers.linear_luminance(Color.WHITE):
		_fail("unaffordable hover is not a lifted unaffordable frame: %s" % unaffordable_hover)
		return false
	if _plate_texture_path(theme.get_stylebox("pressed", "UnaffordableTile")) \
			!= "res://assets/ui/frostline/ui/tile_selected.png":
		_fail("armed unaffordable tile does not paint the armed frame")
		return false
	# Keyboard focus is its own ring outside the plate, bright enough to read
	# over the flat, hover, armed, locked and unaffordable plates alike.
	if not focus is StyleBoxFlat:
		_fail("command focus is not a flat ring: %s" % focus.get_class())
		return false
	var ring := focus as StyleBoxFlat
	if ring.draw_center or ring.border_width_left < 2 or ring.border_width_top < 2 \
			or ring.border_width_right < 2 or ring.border_width_bottom < 2 \
			or ring.get_expand_margin(SIDE_LEFT) < 2.0 \
			or SmokeHelpers.linear_luminance(ring.border_color) < 0.8:
		_fail("command focus ring is not a bright outline outside the plate: %s" % ring)
		return false
	# The shared Tile variation still paints the shipped steel frame for the
	# underground drawer; only the command grids opt into the quiet plates.
	var shared_tile := theme.get_stylebox("normal", "Tile")
	if _plate_texture_path(shared_tile) != "res://assets/ui/frostline/ui/tile_normal.png":
		_fail("shared Tile plate changed; the underground drawer still paints the shipped steel tile")
		return false
	# State changes must not move the icon or the price: every command plate
	# keeps the painted Tile's content box.
	for state in ["normal", "hover", "pressed", "disabled"]:
		var plate: StyleBox = theme.get_stylebox(state, "CommandTile")
		for side in [SIDE_LEFT, SIDE_TOP, SIDE_RIGHT, SIDE_BOTTOM]:
			if plate.get_content_margin(side) != shared_tile.get_content_margin(side):
				_fail("command %s plate content margin %s differs from the painted tile %s" % [
					state, plate.get_content_margin(side), shared_tile.get_content_margin(side)])
				return false
	# Live tiles: the grids opt into the quiet treatment (never the shared or
	# any other variation), prices stay readable on it, and the icon art keeps
	# a real contrast step over the quiet field.
	var margin_plate: StyleBox = theme.get_stylebox("normal", "UnaffordableTile")
	for side in [SIDE_LEFT, SIDE_TOP, SIDE_RIGHT, SIDE_BOTTOM]:
		if margin_plate.get_content_margin(side) != shared_tile.get_content_margin(side):
			_fail("unaffordable plate content margin moved the price/icon box")
			return false
	for grid_path in ["HUD/CommandPanel/BuildGrid", "HUD/CommandPanel/TrainGrid"]:
		var grid := get_node_or_null("Main/%s" % grid_path) as Container
		if grid == null:
			_fail("command grid is missing: %s" % grid_path)
			return false
		for node in grid.get_children():
			if not node is Button:
				continue
			var button := node as Button
			if button.theme_type_variation != &"CommandTile" \
					and button.theme_type_variation != &"UnaffordableTile":
				_fail("%s paints the shared %s variation; the command grids must opt into the quiet plates" % [
					button.name, button.theme_type_variation])
				return false
			var cost := button.get_node_or_null("Cost") as RichTextLabel
			if cost == null:
				_fail("%s lost its catalogue price label" % button.name)
				return false
			var price_ratio := SmokeHelpers.contrast_ratio(
				cost.get_theme_color("default_color"), normal_fill)
			if price_ratio < 7.0:
				_fail("%s price contrast on the quiet plate is %.2f:1" % [button.name, price_ratio])
				return false
			var price_band_error := SmokeHelpers.command_price_error(button, button.name)
			if not price_band_error.is_empty():
				_fail(price_band_error)
				return false
			# 2.5:1 over the mean glyph colour is a floor for a metric that
			# averages dark outlines into the icon body; the bright core is
			# far above it.
			var icon_ratio := SmokeHelpers.contrast_ratio(_opaque_mean_color(button.icon), normal_fill)
			if icon_ratio < 2.5:
				_fail("%s icon art contrast on the quiet plate is %.2f:1" % [button.name, icon_ratio])
				return false
	# Live exceptional states at the opening stockpile: FlameTower is a true
	# lock (affordable, research-locked), CrystalLance is unaffordable and
	# locked, so both must resolve their painted frames from real metadata.
	var flame := get_node("Main/HUD/CommandPanel/BuildGrid/FlameTowerButton") as Button
	var lance := get_node("Main/HUD/CommandPanel/BuildGrid/CrystalLanceButton") as Button
	if not flame.disabled or float(flame.get_meta("unlock_points", 0)) <= 0.0:
		_fail("FlameTower is not the live research lock at the opening stockpile")
		return false
	if _plate_texture_path(flame.get_theme_stylebox("disabled")) \
			!= "res://assets/ui/frostline/ui/tile_locked.png":
		_fail("locked FlameTower does not resolve the painted locked frame")
		return false
	if lance.theme_type_variation != &"UnaffordableTile" \
			or _plate_texture_path(lance.get_theme_stylebox("normal")) \
				!= "res://assets/ui/frostline/ui/tile_unaffordable.png" \
			or _plate_texture_path(lance.get_theme_stylebox("disabled")) \
				!= "res://assets/ui/frostline/ui/tile_locked.png":
		_fail("CrystalLance does not present both its unaffordable and locked frames: %s" % [
			lance.theme_type_variation])
		return false
	# The price band is state-independent: the true lock and the live
	# unaffordable action must keep the same readable catalogue price.
	var flame_price := SmokeHelpers.command_price_error(flame, "locked FlameTower")
	if not flame_price.is_empty():
		_fail(flame_price)
		return false
	var lance_price := SmokeHelpers.command_price_error(lance, "unaffordable CrystalLance")
	if not lance_price.is_empty():
		_fail(lance_price)
		return false
	# The price check must tie each amount to its own resource icon, not to
	# any digit run in the label: the synthetic cases below feed the real
	# cost_markup output through the validator, and the live-tile probe moves
	# one charged amount onto an icon the catalogue does not charge — the
	# parsed token set never changes, so only pair validation catches it.
	var even_cost := {"food": 50, "scrap": 50}
	if not SmokeHelpers.price_markup_error(
			HudHelpers.cost_markup(even_cost), even_cost, "synthetic pair").is_empty():
		_fail("price validation rejects matching icon/amount pairs")
		return false
	for wrong_cost in [{"food": 50, "scrap": 150}, {"food": 50}, {"food": 150, "scrap": 50}]:
		if SmokeHelpers.price_markup_error(
				HudHelpers.cost_markup(wrong_cost), even_cost, "synthetic pair").is_empty():
			_fail("price validation misses %s against catalogue %s" % [wrong_cost, even_cost])
			return false
	var split_cost := {"food": 50, "scrap": 30}
	if SmokeHelpers.price_markup_error(
			HudHelpers.cost_markup({"food": 30, "scrap": 50}), split_cost, "synthetic swap").is_empty():
		_fail("price validation misses amounts swapped between icons")
		return false
	var probe_cost: Dictionary = flame.get_meta("cost")
	var probe_label := flame.get_node("Cost") as RichTextLabel
	var tampered := probe_cost.duplicate()
	for resource in ["food", "scrap", "coal", "crystals"]:
		var moved := int(tampered.get(resource, 0))
		if moved > 0:
			tampered.erase(resource)
			for other in ["food", "scrap", "coal", "crystals"]:
				if int(probe_cost.get(other, 0)) <= 0:
					tampered[other] = moved
					break
			break
	var original_text := probe_label.text
	probe_label.text = "[center]%s[/center]" % HudHelpers.cost_markup(tampered)
	var tampered_error := SmokeHelpers.command_price_error(flame, "misiconned FlameTower")
	probe_label.text = original_text
	if tampered_error.is_empty():
		_fail("price validation accepts an amount shown under the wrong resource icon")
		return false
	return true

## HPA-698 fix round: one geometry assertion for the live research card and
## every labelled synthetic boundary state. The shaped label lines and the
## milestone row's own content must fit their fixed rectangles, and the four
## card rows must not overlap each other.
func _research_geometry_error(card: Control, bar: ProgressBar, label: Label,
		markers: RichTextLabel, analyze: Button, where: String) -> String:
	if card == null or bar == null or label == null or markers == null or analyze == null:
		return "%s research controls are missing" % where
	if label.get_theme_font_size("font_size") < 16 \
			or markers.get_theme_font_size("normal_font_size") < 13 \
			or bar.size.y < 28:
		return "%s research type/progress is too small: label=%d px markers=%d px bar=%.1f px" % [
			where, label.get_theme_font_size("font_size"),
			markers.get_theme_font_size("normal_font_size"), bar.size.y]
	var card_rect := card.get_global_rect()
	var label_rect := label.get_global_rect()
	var markers_rect := markers.get_global_rect()
	var bar_rect := bar.get_global_rect()
	var analyze_rect := analyze.get_global_rect()
	if not card_rect.encloses(label_rect) or not card_rect.encloses(markers_rect) \
			or not card_rect.encloses(bar_rect) or not card_rect.encloses(analyze_rect):
		return "%s research readout leaves its card: label=%s markers=%s bar=%s analyze=%s card=%s" % [
			where, label_rect, markers_rect, bar_rect, analyze_rect, card_rect]
	var lines := label.text.split("\n")
	var label_width := 0.0
	for line in lines:
		label_width = maxf(label_width, label.get_theme_font("font").get_string_size(
			line, HORIZONTAL_ALIGNMENT_LEFT, -1,
			label.get_theme_font_size("font_size")).x)
	var label_height := label.get_theme_font("font").get_height(
		label.get_theme_font_size("font_size")) * lines.size()
	if label_width > label_rect.size.x + 0.5 or label_height > label_rect.size.y + 0.5:
		return "%s research label shaped text does not fit: text=%sx%s rect=%s" % [
			where, label_width, label_height, label_rect.size]
	if markers.get_content_width() > markers_rect.size.x - 20.0 \
			or markers.get_content_height() > markers_rect.size.y + 0.5:
		return "%s research milestones clip their own content: content=%s rect=%s" % [
			where, Vector2(markers.get_content_width(), markers.get_content_height()),
			markers_rect.size]
	if label_rect.intersects(analyze_rect) or markers_rect.intersects(analyze_rect) \
			or label_rect.intersects(markers_rect) or bar_rect.intersects(label_rect) \
			or bar_rect.intersects(analyze_rect) or markers_rect.intersects(bar_rect):
		return "%s research card rows overlap: bar=%s label=%s markers=%s analyze=%s" % [
			where, bar_rect, label_rect, markers_rect, analyze_rect]
	return ""

## Exact milestone readout for a probe: every catalogue short name/value must
## be present, and exactly the expected set carries the non-colour check mark.
func _research_reached_error(text: String, points: float, expected_marks: int,
		flame_points: int, lance_points: int, wave_points: int) -> String:
	var thresholds := {
		"Flame": flame_points,
		"Lance": lance_points,
		"Wave": wave_points,
	}
	var marks := 0
	for short in thresholds:
		var threshold := int(thresholds[short])
		if not text.contains("%s %d" % [short, threshold]):
			return "milestone %s %d is missing from %s" % [short, threshold, text]
		var should_reach := points >= float(threshold)
		var has_mark := text.contains("✓ %s %d" % [short, threshold])
		if should_reach and not has_mark:
			return "reached milestone %s has no check mark: %s" % [short, text]
		if not should_reach and has_mark:
			return "unreached milestone %s is marked reached: %s" % [short, text]
		if has_mark:
			marks += 1
	if marks != expected_marks:
		return "expected %d reached milestones, found %d: %s" % [expected_marks, marks, text]
	return ""

## Test-local display grouping for exact expected label text; independent of
## the HUD's formatter so a format change cannot silently bless itself.
func _grouped_int(value: int) -> String:
	var digits := str(value)
	var grouped := ""
	for index in digits.length():
		if index > 0 and (digits.length() - index) % 3 == 0:
			grouped += ","
		grouped += digits[index]
	return grouped

func _eta_text(seconds: float) -> String:
	var total := maxi(ceili(seconds), 0)
	return "%d:%02d" % [total / 60, total % 60]

## The probe's synthetic points temporarily unlock research-gated build tiles
## through the production refresh. Put the live opening lock state back before
## the capture so the frame never presents a fabricated unlock as its subject.
func _restore_build_locks(points: float) -> void:
	for button in get_node("Main/HUD/CommandPanel/BuildGrid").get_children():
		if button is Button:
			var required := float(button.get_meta("unlock_points", 0))
			button.disabled = required > 0.0 and points < required

## HPA-698 fix round: rendered focus evidence where a real renderer exists.
## The focus style expands beyond the plate, so its bright border must appear
## in the 4 px band just outside the focused rect; the unfocused comparison
## tile's same band is the card's dark field. Headless runs cannot read pixels
## and print an explicit skip instead of treating the semantic checks as proof.
func _rendered_focus_error(focused: Control, unfocused: Control, where: String) -> String:
	if DisplayServer.get_name() == "headless":
		print("RENDERED_FOCUS_SKIP %s (headless viewport has no readable pixels)" % where)
		return ""
	await RenderingServer.frame_post_draw
	var image := get_viewport().get_texture().get_image()
	if image == null or image.is_empty():
		return "%s rendered-focus check could not read the viewport image" % where
	var focus_count := _outside_band_bright_pixels(image, focused.get_global_rect())
	var unfocused_count := _outside_band_bright_pixels(image, unfocused.get_global_rect())
	print("RENDERED_FOCUS %s focused_band=%d unfocused_band=%d" % [
		where, focus_count, unfocused_count])
	if focus_count < 80 or focus_count <= unfocused_count * 2:
		return "%s visible focus ring is missing or not distinct: focused band=%d, unfocused band=%d" % [
			where, focus_count, unfocused_count]
	return ""

## Count near-white pixels in the 4 px band just outside a control's rect. The
## ring is drawn by StyleBoxFlat's 2 px expand margin plus a 2 px border.
func _outside_band_bright_pixels(image: Image, rect: Rect2) -> int:
	var count := 0
	# The control rect is in logical viewport pixels; a HiDPI viewport texture
	# may be larger. Scale the probe rect to the image's own pixel grid.
	var viewport_size := get_viewport().get_visible_rect().size
	var scale := Vector2(image.get_width(), image.get_height()) / viewport_size
	var image_rect := Rect2(rect.position * scale, rect.size * scale)
	var left := int(round(image_rect.position.x))
	var right := int(round(image_rect.end.x))
	var top := int(round(image_rect.position.y))
	var bottom := int(round(image_rect.end.y))
	for x in range(left + 4, right - 4):
		for offset in [1, 2, 3, 4]:
			count += _bright_pixel(image, x, top - offset)
			count += _bright_pixel(image, x, bottom + offset)
	for y in range(top + 4, bottom - 4):
		for offset in [1, 2, 3, 4]:
			count += _bright_pixel(image, left - offset, y)
			count += _bright_pixel(image, right + offset, y)
	return count

func _bright_pixel(image: Image, x: int, y: int) -> int:
	if x < 0 or y < 0 or x >= image.get_width() or y >= image.get_height():
		return 0
	var color := image.get_pixel(x, y)
	return 1 if minf(color.r, minf(color.g, color.b)) > 0.65 else 0

## HPA-698 final review: command feedback (rejected orders, armed targets) is
## drawn before the bottom selection card, so it must sit wholly above it.
func _assert_command_status_above_selection() -> bool:
	var status := get_node_or_null("Main/HUD/CommandStatus") as Label
	var selection_card := get_node_or_null("Main/HUD/SelectionPanel") as Panel
	if status == null or selection_card == null:
		_fail("CommandStatus or SelectionPanel is missing for the layout check")
		return false
	var status_rect := status.get_global_rect()
	var card_rect := selection_card.get_global_rect()
	if not status.is_visible_in_tree() or status.text.is_empty() \
			or not Rect2(Vector2.ZERO, Vector2(TEST_VIEWPORT_SIZE)).encloses(status_rect) \
			or status_rect.intersects(card_rect) or status_rect.end.y > card_rect.position.y:
		_fail("CommandStatus is not wholly above the selection card: status=%s card=%s" % [
			status_rect, card_rect])
		return false
	# The dark-edge treatment must not move or resize the feedback surface.
	if status_rect.position != Vector2(216.0, 478.0) \
			or status_rect.size != Vector2(504.0, 24.0):
		_fail("CommandStatus geometry changed: %s" % status_rect)
		return false
	var readability := _command_status_readability_error(status, "opening hint")
	if not readability.is_empty():
		_fail(readability)
		return false
	return true

## The feedback label is 12 px light ink drawn straight over the battlefield;
## the shared Label theme supplies ink only, so bare glyphs reach about 1.1:1
## where the string crosses snow. Require the engine-drawn Label outline to be
## that scoped dark edge, and require the glyphs plus the edge to fit the
## label's own rectangle so the treatment can neither clip nor reach a control.
func _command_status_readability_error(status: Label, where: String) -> String:
	if status == null:
		return "%s: CommandStatus is missing" % where
	var outline_size := status.get_theme_constant("outline_size")
	if outline_size < 3:
		return "%s: CommandStatus dark edge is %d px, not a thick outline" % [
			where, outline_size]
	var outline := status.get_theme_color("font_outline_color")
	if outline.a < 0.85 or outline.get_luminance() > 0.15:
		return "%s: CommandStatus outline is not a dark, opaque edge: %s" % [where, outline]
	var ink_contrast := SmokeHelpers.contrast_ratio(
		status.get_theme_color("font_color"), outline)
	if ink_contrast < 7.0:
		return "%s: CommandStatus ink/outline contrast is %.2f:1" % [where, ink_contrast]
	if status.mouse_filter != Control.MOUSE_FILTER_IGNORE:
		return "%s: CommandStatus no longer ignores the mouse" % where
	var shaped := SmokeHelpers.shaped_text_size(status)
	var edge := float(outline_size) * 2.0
	if shaped.x + edge > status.size.x or shaped.y + edge > status.size.y:
		return "%s: CommandStatus text %.1fx%.1f plus a %d px edge does not fit its %.1fx%.1f rect" % [
			where, shaped.x, shaped.y, outline_size, status.size.x, status.size.y]
	return ""

## HPA-698 visual follow-up: the empty selection card is a quiet placeholder,
## not a second title bar. The oversized Header name dominated the lower panel
## before anything was selected, so the empty state carries one body-size line
## that must fit its own rectangle. Selecting a unit/building still owns the
## Header name.
func _assert_empty_selection_hierarchy() -> bool:
	var name_label := get_node_or_null("Main/HUD/SelectionPanel/SelectionName") as Label
	var hint := get_node_or_null("Main/HUD/SelectionPanel/SelectionLabel") as Label
	if name_label == null or hint == null:
		_fail("empty selection labels are missing under HUD/SelectionPanel")
		return false
	if not name_label.text.is_empty():
		_fail("empty selection renders the heavy title \"%s\" instead of a quiet body hint" % [
			name_label.text])
		return false
	if hint.text != "Nothing selected":
		_fail("empty selection hint reads \"%s\"" % [hint.text])
		return false
	var font_size := hint.get_theme_font_size("font_size")
	if font_size > 14:
		_fail("empty selection hint is %d px; the empty state stays at body size" % [font_size])
		return false
	var shaped := SmokeHelpers.shaped_text_size(hint)
	if shaped.x > hint.size.x + 0.5 or shaped.y > hint.size.y + 0.5:
		_fail("empty selection hint %.1fx%.1f does not fit its %.1fx%.1f rectangle" % [
			shaped.x, shaped.y, hint.size.x, hint.size.y])
		return false
	return true

## HPA-698 presentation contract: one project-wide Frostline theme using the
## bundled Barlow faces (never a system-font assumption), the 15 HPA-695 icons
## imported at 128 px into the existing icon tree, and the UI-kit/screen
## plates the HUD styles with. Asset presence alone is not a rendering check;
## the checkpoint's rendered-frame review covers that separately.
func _assert_frostline_assets() -> bool:
	var theme_path := str(ProjectSettings.get_setting("gui/theme/custom", ""))
	var theme := load(theme_path) as Theme if not theme_path.is_empty() else null
	if theme == null or theme.default_font == null \
			or str(theme.default_font.resource_path) != "res://assets/fonts/Barlow-Regular.ttf":
		_fail("project gui/theme/custom is not the bundled Barlow theme: \"%s\"" % theme_path)
		return false
	# get_stylebox() returns the engine fallback stylebox when the property is
	# absent, so only has_stylebox() can detect a missing UnaffordableTile
	# normal style.
	if not theme.has_stylebox("normal", "UnaffordableTile"):
		_fail("project theme is missing the unaffordable-tile Button variation")
		return false
	# The round order buttons must scale the complete circular source image.
	# Nine-slice margins larger than the rendered button collapse the centre
	# and leave only transparent source corners (the original Task 4 bug).
	var round_texture := load("res://assets/ui/frostline/ui/round_button.png") as Texture2D
	for state in ["normal", "hover", "pressed", "disabled"]:
		if not theme.has_stylebox(state, "OrderButton"):
			_fail("project theme is missing the OrderButton/%s round style" % state)
			return false
		var order_style := theme.get_stylebox(state, "OrderButton") as StyleBoxTexture
		if order_style == null or order_style.texture != round_texture \
				or not is_zero_approx(order_style.texture_margin_left) \
				or not is_zero_approx(order_style.texture_margin_top) \
				or not is_zero_approx(order_style.texture_margin_right) \
				or not is_zero_approx(order_style.texture_margin_bottom):
			_fail("OrderButton/%s does not scale the full round image (margins %s/%s/%s/%s)" % [
				state, order_style.texture_margin_left if order_style != null else -1,
				order_style.texture_margin_top if order_style != null else -1,
				order_style.texture_margin_right if order_style != null else -1,
				order_style.texture_margin_bottom if order_style != null else -1])
			return false
	var license := FileAccess.get_file_as_string("res://assets/fonts/OFL.txt")
	if not license.contains("SIL Open Font License"):
		_fail("bundled Barlow OFL license text is missing")
		return false
	for icon_path in [
		"res://assets/ui/icons/actions/attack_move.png",
		"res://assets/ui/icons/actions/gather.png",
		"res://assets/ui/icons/actions/move.png",
		"res://assets/ui/icons/actions/pause.png",
		"res://assets/ui/icons/actions/rally.png",
		"res://assets/ui/icons/actions/stop.png",
		"res://assets/ui/icons/status/health.png",
		"res://assets/ui/icons/status/idle_workers.png",
		"res://assets/ui/icons/status/locked.png",
		"res://assets/ui/icons/status/mouse_left.png",
		"res://assets/ui/icons/status/mouse_right.png",
		"res://assets/ui/icons/status/mouse_wheel.png",
		"res://assets/ui/icons/status/population.png",
		"res://assets/ui/icons/status/research_rate.png",
		"res://assets/ui/icons/status/time.png",
	]:
		var icon := load(icon_path) as Texture2D
		if icon == null or icon.get_width() != 128 or icon.get_height() != 128:
			_fail("HPA-695 icon is missing or not the 128 px import size: %s" % icon_path)
			return false
	for plate_path in [
		"res://assets/ui/frostline/ui/bar_frame.png",
		"res://assets/ui/frostline/ui/frost_overlay.png",
		"res://assets/ui/frostline/ui/plate.png",
		"res://assets/ui/frostline/ui/portrait_bezel.png",
		"res://assets/ui/frostline/ui/round_button.png",
		"res://assets/ui/frostline/ui/socket.png",
		"res://assets/ui/frostline/ui/tile_locked.png",
		"res://assets/ui/frostline/ui/tile_normal.png",
		"res://assets/ui/frostline/ui/tile_selected.png",
		"res://assets/ui/frostline/ui/tile_unaffordable.png",
		"res://assets/ui/frostline/screens/defeat_banner.png",
		"res://assets/ui/frostline/screens/emblem.png",
		"res://assets/ui/frostline/screens/start_key_art.png",
		"res://assets/ui/frostline/screens/victory_banner.png",
	]:
		if load(plate_path) as Texture2D == null:
			_fail("Frostline UI-kit texture is missing: %s" % plate_path)
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

## Structure import check (HPA-291, narrowed to the retained Wall/Gate
## GLBs by HPA-697): the PackedScene must load and instantiate a nonempty
## mesh, carry no gameplay nodes, sit inside the normalized ±0.5 footprint
## with a grounded base, and resolve its diffuse texture from the committed
## res://assets/structures/ PNG. The GLB itself must carry each image as
## exactly one source — a committed-texture uri, never an embedded
## bufferView — or Godot's importer warns.
func _assert_structure_import(path: String) -> bool:
	if not ResourceLoader.exists(path):
		_fail("structure import is missing: %s" % path)
		return false
	var packed := load(path) as PackedScene
	if packed == null:
		_fail("structure import did not load as a PackedScene: %s" % path)
		return false
	var model := packed.instantiate() as Node3D
	if model == null:
		_fail("structure import did not instantiate a Node3D: %s" % path)
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
	if bounds.position.y > 0.01:
		_fail("%s art floats %.4f above the ground plane" % [path, bounds.position.y])
		return false
	for node in meshes:
		var mesh := node as MeshInstance3D
		if mesh.mesh == null or mesh.mesh.get_surface_count() == 0:
			_fail("%s imported an empty mesh" % path)
			return false
		for surface in mesh.mesh.get_surface_count():
			var material := mesh.get_active_material(surface) as StandardMaterial3D
			var texture := material.albedo_texture if material != null else null
			if texture == null or not texture.resource_path.begins_with("res://assets/structures/"):
				_fail("%s mesh material does not resolve a committed structures texture: %s" % [
					path, texture.resource_path if texture != null else "missing"])
				return false
			if not material.vertex_color_use_as_albedo:
				_fail("%s mesh surface %d does not use its vertex colors as albedo" % [path, surface])
				return false
	# Raw GLB source bytes exist only under an editor binary: an export
	# remaps the .glb to its imported PackedScene inside the PCK, where
	# FileAccess can no longer read the original file. OS.has_feature
	# ("editor") is true for any project run by an editor binary — the
	# case for every source smoke — and false in exported templates;
	# Engine.is_editor_hint() cannot distinguish because this scene does
	# not run inside the editor.
	if OS.has_feature("editor") and not _assert_structure_source(path):
		return false
	model.free()
	return true

## Painted-texture import check (HPA-697): the runtime texture must load
## with generated mipmaps, an alpha channel, and a CPU-readable uncompressed
## (lossless) image, and — under an editor binary, where raw sources are
## readable — base-mip pixels byte-identical to the committed PNG. Exports
## remap textures into the PCK, so the source half is gated like the GLB
## byte audit below.
func _assert_painted_import(path: String) -> bool:
	if not ResourceLoader.exists(path):
		_fail("painted texture import is missing: %s" % path)
		return false
	var texture := load(path) as Texture2D
	if texture == null:
		_fail("painted texture did not load as a Texture2D: %s" % path)
		return false
	var image := texture.get_image()
	if image == null:
		_fail("painted texture carries no CPU-readable image: %s" % path)
		return false
	if image.is_compressed():
		_fail("painted texture import is not lossless: %s format=%s" % [path, image.get_format()])
		return false
	if not image.has_mipmaps():
		_fail("painted texture import generated no mipmaps: %s" % path)
		return false
	if not image.detect_alpha():
		_fail("painted texture lost its alpha channel: %s" % path)
		return false
	if OS.has_feature("editor"):
		var source := Image.new()
		var source_error := source.load_png_from_buffer(FileAccess.get_file_as_bytes(path))
		if source_error != OK or source.get_size() != image.get_size() \
				or source.get_data() != image.get_data().slice(0, image.get_mipmap_offset(1)):
			_fail("imported painted texture diverges from its committed source: %s" % path)
			return false
	return true

## Source-byte half of _assert_structure_import, reachable only under an
## editor binary (see the call-site gate): parses the committed GLB's own
## header and JSON chunk. A packaged build never reaches this — the PCK
## holds the imported PackedScene the runtime checks above exercise.
func _assert_structure_source(path: String) -> bool:
	var glb := FileAccess.open(path, FileAccess.READ)
	if glb == null or glb.get_32() != 0x46546C67:
		_fail("%s is not a binary glTF" % path)
		return false
	glb.get_32()
	glb.get_32()
	var json_length := glb.get_32()
	if glb.get_32() != 0x4E4F534A:
		_fail("%s first glTF chunk is not JSON" % path)
		return false
	var gltf: Variant = JSON.parse_string(glb.get_buffer(json_length).get_string_from_utf8())
	if gltf == null:
		_fail("%s glTF JSON chunk did not parse" % path)
		return false
	for image in (gltf as Dictionary).get("images", []):
		var has_uri := (image as Dictionary).has("uri")
		if has_uri == (image as Dictionary).has("bufferView"):
			_fail("%s image must carry exactly one source: %s" % [path, image])
			return false
		var texture_path := path.get_base_dir().path_join(str((image as Dictionary).get("uri", "")))
		if has_uri and (not texture_path.begins_with("res://assets/structures/") \
				or not FileAccess.file_exists(texture_path)):
			_fail("%s image uri does not resolve a committed structures texture: %s" % [path, texture_path])
			return false
	return true

func _mesh_triangles(model: Node3D) -> Array:
	var tool := MeshDataTool.new()
	var triangles := []
	for node in model.find_children("*", "MeshInstance3D", true, false):
		var mesh_node := node as MeshInstance3D
		if mesh_node == null or mesh_node.mesh == null:
			continue
		var to_model: Transform3D = model.global_transform.affine_inverse() * mesh_node.global_transform
		for surface in mesh_node.mesh.get_surface_count():
			if tool.create_from_surface(mesh_node.mesh, surface) != OK:
				return []
			for face in tool.get_face_count():
				var triangle := AABB(to_model * tool.get_vertex(tool.get_face_vertex(face, 0)), Vector3.ZERO)
				triangle = triangle.expand(to_model * tool.get_vertex(tool.get_face_vertex(face, 1)))
				triangle = triangle.expand(to_model * tool.get_vertex(tool.get_face_vertex(face, 2)))
				triangles.append(triangle)
	return triangles

func _assert_gate_corridor(path: String) -> bool:
	var packed := load(path) as PackedScene
	var model := packed.instantiate() as Node3D if packed != null else null
	if model == null:
		_fail("gate import did not instantiate: %s" % path)
		return false
	add_child(model)
	var corridors := [
		AABB(Vector3(-0.5, 0.085, -0.42), Vector3(1.0, 0.915, 0.84)),
		AABB(Vector3(-0.42, 0.085, -0.5), Vector3(0.84, 0.915, 1.0)),
	]
	var triangles := _mesh_triangles(model)
	if triangles.is_empty():
		_fail("gate import yielded no readable mesh triangles: %s" % path)
		return false
	for triangle in triangles:
		for corridor in corridors:
			if corridor.intersects(triangle):
				_fail("gate traversal corridor blocked: %s in %s" % [triangle, corridor])
				return false
	model.free()
	return true

func _elevated_reach(triangles: Array, start: AABB, goal: AABB) -> bool:
	var queue := []
	var visited := {}
	for i in triangles.size():
		if triangles[i].intersects(start):
			queue.append(i)
			visited[i] = true
	while not queue.is_empty():
		var i: int = queue.pop_back()
		var triangle: AABB = triangles[i]
		if triangle.intersects(goal):
			return true
		for j in triangles.size():
			if not visited.has(j) and triangle.intersects(triangles[j]):
				visited[j] = true
				queue.append(j)
	return false

func _assert_wall_gate_contact(wall_path: String, gate_path: String) -> bool:
	var edge_spans := {}
	var triangles := {}
	for path in [wall_path, gate_path]:
		var packed := load(path) as PackedScene
		var model := packed.instantiate() as Node3D if packed != null else null
		if model == null:
			_fail("structure import did not instantiate: %s" % path)
			return false
		add_child(model)
		triangles[path] = _mesh_triangles(model)
		var spans := {"x+": [], "x-": [], "z+": [], "z-": []}
		for triangle in triangles[path]:
			if triangle.end.y <= 0.085:
				continue
			if absf(triangle.end.x - 0.5) < 0.001:
				spans["x+"].append(Vector4(triangle.position.z, triangle.end.z, triangle.position.y, triangle.end.y))
			if absf(triangle.position.x + 0.5) < 0.001:
				spans["x-"].append(Vector4(triangle.position.z, triangle.end.z, triangle.position.y, triangle.end.y))
			if absf(triangle.end.z - 0.5) < 0.001:
				spans["z+"].append(Vector4(triangle.position.x, triangle.end.x, triangle.position.y, triangle.end.y))
			if absf(triangle.position.z + 0.5) < 0.001:
				spans["z-"].append(Vector4(triangle.position.x, triangle.end.x, triangle.position.y, triangle.end.y))
		model.free()
		edge_spans[path] = spans
	for edge in ["x+", "x-", "z+", "z-"]:
		var contact := false
		for wall_span in edge_spans[wall_path][edge]:
			for gate_span in edge_spans[gate_path][edge]:
				var lateral := minf(wall_span.y, gate_span.y) - maxf(wall_span.x, gate_span.x)
				var vertical := minf(wall_span.w, gate_span.w) - maxf(0.085, maxf(wall_span.z, gate_span.z))
				if lateral > 0.0 and vertical > 0.0:
					contact = true
		if not contact:
			_fail("wall and gate share no elevated edge contact on edge %s" % edge)
			return false
	var band := AABB(Vector3(-0.17, 0.2, -0.17), Vector3(0.34, 0.2, 0.34))
	var elevated := []
	for triangle in triangles[wall_path]:
		if triangle.end.y > 0.085:
			elevated.append(triangle.grow(0.001))
	for corner in [Vector2(0.4, 0.4), Vector2(0.4, -0.5), Vector2(-0.5, 0.4), Vector2(-0.5, -0.5)]:
		var anchor := AABB(Vector3(corner.x, 0.085, corner.y), Vector3(0.1, 0.915, 0.1))
		if not _elevated_reach(elevated, anchor, band):
			_fail("wall corner %s has no continuous elevated path to the central band" % corner)
			return false
	return true

func _seam_spans(triangles: Array, axis: String, plane: float, from_below: bool) -> Array:
	var spans := []
	for triangle in triangles:
		if triangle.end.y <= 0.085:
			continue
		if axis == "x":
			if from_below and absf(triangle.end.x - plane) < 0.001:
				spans.append(Vector4(triangle.position.z, triangle.end.z, triangle.position.y, triangle.end.y))
			if not from_below and absf(triangle.position.x - plane) < 0.001:
				spans.append(Vector4(triangle.position.z, triangle.end.z, triangle.position.y, triangle.end.y))
		else:
			if from_below and absf(triangle.end.z - plane) < 0.001:
				spans.append(Vector4(triangle.position.x, triangle.end.x, triangle.position.y, triangle.end.y))
			if not from_below and absf(triangle.position.z - plane) < 0.001:
				spans.append(Vector4(triangle.position.x, triangle.end.x, triangle.position.y, triangle.end.y))
	return spans

func _spans_touch(below_spans: Array, above_spans: Array) -> bool:
	for below in below_spans:
		for above in above_spans:
			var lateral := minf(below.y, above.y) - maxf(below.x, above.x)
			var vertical := minf(below.w, above.w) - maxf(0.085, maxf(below.z, above.z))
			if lateral > 0.0 and vertical > 0.0:
				return true
	return false

func _assert_wall_gate_run(view_scene: PackedScene) -> bool:
	var catalogue: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {})
	for axis in ["x", "z"]:
		var views: Array[Node3D] = []
		var cursor := 0.0
		var seams := []
		for index in 3:
			var kind := "Gate" if index == 1 else "Wall"
			var spec: Dictionary = catalogue.get(kind, {})
			var wide := float(spec.get("width", 0))
			var tall := float(spec.get("height", 0))
			if wide <= 0.0 or tall <= 0.0:
				_fail("catalogue exposes no %s footprint for the live run" % kind)
				return false
			var view := view_scene.instantiate() as Node3D
			view.set_meta("building_kind", kind)
			var along := cursor + (wide if axis == "x" else tall) * 0.5
			view.scale = Vector3(wide, 1.0, tall)
			if axis == "x":
				view.position = Vector3(along, 0.0, 0.5)
			else:
				view.position = Vector3(8.5, 0.0, along)
			cursor += wide if axis == "x" else tall
			seams.append(cursor)
			add_child(view)
			views.append(view)
		await get_tree().process_frame
		var triangles := []
		for view in views:
			view.set_state(true, false)
			var run_triangles := SmokeHelpers.world_triangles(view)
			if run_triangles.is_empty():
				_fail("live %s view produced no mesh triangles for the %s-axis run" % [
					view.get_meta("building_kind", "?"), axis])
				return false
			triangles.append(run_triangles)
		for seam in 2:
			if not _spans_touch(
					_seam_spans(triangles[seam], axis, seams[seam], true),
					_seam_spans(triangles[seam + 1], axis, seams[seam], false)):
				_fail("live Wall-Gate-Wall run has no elevated contact across the %s=%.1f seam" % [axis, seams[seam]])
				return false
		for view in views:
			view.free()
	return true

func _roof_pick_point(view: Node3D, camera: Camera3D, viewport_size: Vector2, main: Node) -> Vector2:
	var model := view.get_node_or_null("Model") as Node3D
	if model == null:
		return Vector2.INF
	var screen_center := camera.unproject_position(view.global_position)
	# Painted kinds sample opaque artwork texels through the live sprite
	# transform instead of GLB roof faces; retained Wall/Gate GLBs keep the
	# mesh triangle scan below.
	var sprites := model.find_children("*", "SpriteBase3D", true, false)
	if not sprites.is_empty():
		return _sprite_pick_point(view, sprites[0] as SpriteBase3D, camera, viewport_size, main)
	var roof_faces: Array[Array] = []
	for node in model.find_children("*", "MeshInstance3D", true, false):
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
	roof_faces.sort_custom(func(a, b): return a[3].y > b[3].y)
	for face in roof_faces:
		var farthest_vertex: Vector3 = face[0]
		var farthest_distance := -1.0
		for vertex_index in 3:
			var vertex := face[vertex_index] as Vector3
			var distance := screen_center.distance_to(camera.unproject_position(vertex))
			if distance > farthest_distance:
				farthest_distance = distance
				farthest_vertex = vertex
		var interior := (face[3] as Vector3) + (farthest_vertex - (face[3] as Vector3)) * 0.75
		var candidate := camera.unproject_position(interior)
		if not Rect2(Vector2.ZERO, viewport_size).has_point(candidate):
			continue
		if screen_center.distance_to(candidate) <= main.CLICK_RADIUS + 5.0:
			continue
		var projected_triangle := PackedVector2Array([
				camera.unproject_position(face[0] as Vector3),
				camera.unproject_position(face[1] as Vector3),
				camera.unproject_position(face[2] as Vector3)])
		if not Geometry2D.is_point_in_polygon(candidate, projected_triangle):
			continue
		if (main.call("_nearest_view", "unit_views", candidate, 1) as Node3D) != null:
			continue
		if main.call("_building_at", candidate, 1) == view:
			return candidate
	return Vector2.INF

## Painted-silhouette pick points (HPA-697): project opaque artwork texels
## on a coarse grid through the live sprite transform, keeping the GLB
## roof path's guards — inside the viewport, beyond the legacy center
## radius, no unit on the point, and the alpha-aware building picker must
## resolve the point to this view. Farthest valid point wins.
func _sprite_pick_point(view: Node3D, sprite: SpriteBase3D, camera: Camera3D, viewport_size: Vector2, main: Node) -> Vector2:
	var view_script: Script = view.get_script()
	var mask: BitMap = view_script.alpha_mask(sprite.texture)
	if mask == null:
		return Vector2.INF
	var screen_center := camera.unproject_position(view.global_position)
	var size := mask.get_size()
	var best := Vector2.INF
	var best_distance := -1.0
	for row in range(0, int(size.y), 16):
		for col in range(0, int(size.x), 16):
			if not mask.get_bitv(Vector2i(col, row)):
				continue
			var world: Vector3 = sprite.global_transform * view_script.sprite_local_from_pixel(sprite, Vector2(col, row))
			var candidate := camera.unproject_position(world)
			if not Rect2(Vector2.ZERO, viewport_size).has_point(candidate) \
					or screen_center.distance_to(candidate) <= main.CLICK_RADIUS + 5.0 \
					or (main.call("_nearest_view", "unit_views", candidate, 1) as Node3D) != null \
					or (main.call("_building_at", candidate, 1) as Node3D) != view:
				continue
			var distance := screen_center.distance_to(candidate)
			if distance > best_distance:
				best = candidate
				best_distance = distance
	return best

func _radius_steal_pick_point(view: Node3D, neighbor: Node3D, camera: Camera3D, viewport_size: Vector2, main: Node) -> Vector2:
	var model := view.get_node_or_null("Model") as Node3D
	var neighbor_model := neighbor.get_node_or_null("Model") as Node3D
	if model == null or neighbor_model == null:
		return Vector2.INF
	var view_rect: Rect2 = main.call("_projected_art_rect", view, model)
	var neighbor_rect: Rect2 = main.call("_projected_art_rect", neighbor, neighbor_model)
	var view_screen := camera.unproject_position(view.global_position)
	var neighbor_screen := camera.unproject_position(neighbor.global_position)
	var hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	# Painted kinds: opaque texel samples on the live sprite plane carry the
	# same rect/HUD/unit/margin guards; retained Wall/Gate GLBs keep the
	# upward roof-face interior-point scan below.
	var sprites := model.find_children("*", "SpriteBase3D", true, false)
	if not sprites.is_empty():
		var view_script: Script = view.get_script()
		var sprite := sprites[0] as SpriteBase3D
		var mask: BitMap = view_script.alpha_mask(sprite.texture)
		if mask == null:
			return Vector2.INF
		var mask_size := mask.get_size()
		var sprite_best := Vector2.INF
		var sprite_best_margin := 0.0
		for row in range(0, int(mask_size.y), 16):
			for col in range(0, int(mask_size.x), 16):
				if not mask.get_bitv(Vector2i(col, row)):
					continue
				var world: Vector3 = sprite.global_transform * view_script.sprite_local_from_pixel(sprite, Vector2(col, row))
				var candidate := camera.unproject_position(world)
				if not Rect2(Vector2.ZERO, viewport_size).has_point(candidate) \
						or not view_rect.has_point(candidate) \
						or neighbor_rect.has_point(candidate) \
						or hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(candidate)) \
						or (main.call("_nearest_view", "unit_views", candidate, 1) as Node3D) != null:
					continue
				var neighbor_distance := neighbor_screen.distance_to(candidate)
				if neighbor_distance > main.CLICK_RADIUS:
					continue
				var margin := view_screen.distance_to(candidate) - neighbor_distance
				if margin > sprite_best_margin:
					sprite_best = candidate
					sprite_best_margin = margin
		return sprite_best
	var roof_faces: Array[Array] = []
	for node in model.find_children("*", "MeshInstance3D", true, false):
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
				roof_faces.append([a, b, c])
	roof_faces.sort_custom(func(a, b): return (a[0] + a[1] + a[2]).y > (b[0] + b[1] + b[2]).y)
	var best := Vector2.INF
	var best_margin := 0.0
	for face in roof_faces:
		var centroid: Vector3 = (face[0] + face[1] + face[2]) / 3.0
		var projected_triangle := PackedVector2Array([
				camera.unproject_position(face[0] as Vector3),
				camera.unproject_position(face[1] as Vector3),
				camera.unproject_position(face[2] as Vector3)])
		var interior_points: Array[Vector3] = [centroid]
		for vertex_index in 3:
			var vertex := face[vertex_index] as Vector3
			interior_points.append(centroid + (vertex - centroid) * 0.5)
			interior_points.append(centroid + (vertex - centroid) * 0.8)
		for vertex_index in 3:
			var edge_mid := ((face[vertex_index] as Vector3) + (face[(vertex_index + 1) % 3] as Vector3)) * 0.5
			interior_points.append(centroid + (edge_mid - centroid) * 0.9)
		for world_point in interior_points:
			var candidate := camera.unproject_position(world_point)
			if not Geometry2D.is_point_in_polygon(candidate, projected_triangle) \
					or not Rect2(Vector2.ZERO, viewport_size).has_point(candidate) \
					or not view_rect.has_point(candidate) \
					or neighbor_rect.has_point(candidate) \
					or hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(candidate)) \
					or (main.call("_nearest_view", "unit_views", candidate, 1) as Node3D) != null:
				continue
			var neighbor_distance := neighbor_screen.distance_to(candidate)
			if neighbor_distance > main.CLICK_RADIUS:
				continue
			var margin := view_screen.distance_to(candidate) - neighbor_distance
			if margin > best_margin:
				best = candidate
				best_margin = margin
	return best

## HPA-697 Step 2 overlap pick points: scan the painted view's opaque art
## pixels for one where `unit` wins or loses to `view` under the live
## `_unit_pick_at` / `_building_beats_unit` rules (`want_unit_first`).
func _overlap_pick_point(view: Node3D, unit: Node3D, want_unit_first: bool, camera: Camera3D, viewport_size: Vector2, main: Node) -> Vector2:
	var model := view.get_node_or_null("Model") as Node3D
	if model == null:
		return Vector2.INF
	var sprites := model.find_children("*", "SpriteBase3D", true, false)
	if sprites.is_empty():
		return Vector2.INF
	var sprite := sprites[0] as SpriteBase3D
	var view_script: Script = view.get_script()
	var mask: BitMap = view_script.alpha_mask(sprite.texture)
	if mask == null:
		return Vector2.INF
	var hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	var unit_screen := camera.unproject_position(unit.global_position)
	var mask_size := mask.get_size()
	var best := Vector2.INF
	var best_distance := -1.0
	for row in range(0, int(mask_size.y), 8):
		for col in range(0, int(mask_size.x), 8):
			if not mask.get_bitv(Vector2i(col, row)):
				continue
			var world: Vector3 = sprite.global_transform * view_script.sprite_local_from_pixel(sprite, Vector2(col, row))
			var candidate := camera.unproject_position(world)
			if not Rect2(Vector2.ZERO, viewport_size).has_point(candidate) \
					or hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(candidate)):
				continue
			var hit: Dictionary = main.call("_building_hit_at", candidate, 0)
			if hit.get("view") != view or is_inf(float(hit.get("depth", INF))):
				continue
			if (main.call("_unit_pick_at", candidate, 1) as Node3D) != unit:
				continue
			var origin := camera.project_ray_origin(candidate)
			var direction := camera.project_ray_normal(candidate)
			var building_beats := bool(main.call("_building_beats_unit", hit, unit, origin, direction))
			if want_unit_first and building_beats:
				continue
			if not want_unit_first and not building_beats:
				continue
			var unit_distance := unit_screen.distance_to(candidate)
			if best_distance < 0.0 or unit_distance < best_distance:
				best = candidate
				best_distance = unit_distance
	return best

## Hop one worker to a cell-centred spot through the real bridge move path,
## waiting for sim acceptance, the arrival radius and a settled Idle task.
func _hop_worker(unit: Node3D, target: Vector2, tolerance: float, why: String) -> bool:
	if not GrusBridge.set_sim_speed(20.0):
		_fail("could not accelerate the sim for the %s hop" % why)
		return false
	var revision := int(GrusBridge.command_feedback_revision())
	if not GrusBridge.move_units(PackedInt32Array([int(unit.get_meta("unit_id", -1))]), target):
		_fail("%s move was not queued" % why)
		return false
	if not await _wait_until(_accepted_since.bind(revision), 10.0, "%s move was not accepted by the sim" % why):
		return false
	var unit_id := int(unit.get_meta("unit_id", -1))
	if not await _wait_until(func():
			var view := _unit_view(unit_id)
			return view != null \
				and view.global_position.distance_to(Vector3(target.x, 0.0, target.y)) <= tolerance \
				and _unit_idle(view),
			30.0, "the %s worker never arrived near %s" % [why, target]):
		return false
	# Let the hand-rolled per-tick view sync catch up with the settled sim
	# position before any screen-space scan or click derives from the view.
	for _settle in 3:
		await get_tree().process_frame
	if not GrusBridge.set_sim_speed(1.0):
		_fail("could not restore sim speed after the %s hop" % why)
		return false
	return true

## Hop one worker through the real bridge move path to the first candidate
## cell whose move the sim accepts, then wait for arrival, settle and Idle.
## Used where the target ground's walkability is not knowable in advance, so
## a rejected goal must try the next cell instead of failing the leg.
func _hop_worker_to_first_cell(unit: Node3D, cells: Array[Vector2], tolerance: float, why: String) -> Vector2:
	var unit_id := int(unit.get_meta("unit_id", -1))
	if not GrusBridge.set_sim_speed(20.0):
		_fail("could not accelerate the sim for the %s hop" % why)
		return Vector2.INF
	var chosen := Vector2.INF
	for cell in cells:
		var revision := int(GrusBridge.command_feedback_revision())
		if not GrusBridge.move_units(PackedInt32Array([unit_id]), cell):
			continue
		for _frame in int(10.0 * Engine.physics_ticks_per_second):
			if _accepted_since(revision):
				chosen = cell
				break
			if int(GrusBridge.command_feedback_revision()) > revision:
				break
			await get_tree().physics_frame
		if chosen != Vector2.INF:
			break
	if chosen == Vector2.INF:
		_fail("%s move was never accepted for any candidate cell" % why)
		return Vector2.INF
	if not await _wait_until(func():
			var view := _unit_view(unit_id)
			return view != null \
				and view.global_position.distance_to(Vector3(chosen.x, 0.0, chosen.y)) <= tolerance \
				and _unit_idle(view),
			30.0, "the %s worker never arrived near %s" % [why, chosen]):
		return Vector2.INF
	for _settle in 3:
		await get_tree().process_frame
	if not GrusBridge.set_sim_speed(1.0):
		_fail("could not restore sim speed after the %s hop" % why)
		return Vector2.INF
	return chosen

func _clear_click_point(viewport_size: Vector2, main: Node) -> Vector2:
	var hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	for row in [viewport_size.y - 30.0, viewport_size.y - 60.0, viewport_size.y * 0.5, 40.0]:
		for column in range(20, int(viewport_size.x) - 20, 40):
			var point := Vector2(float(column), row)
			if hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(point)):
				continue
			if (main.call("_building_at", point, 0) as Node3D) != null:
				continue
			if (main.call("_nearest_view", "unit_views", point, 0) as Node3D) != null:
				continue
			return point
	return Vector2.INF

## Scan the screen column above a transit worker's ground projection for a
## Gate-beam pixel in the bug regime: a real art hit whose depth sits
## strictly between the worker's own mesh hit and its +0.15 feet anchor —
## beam nearer than the feet rule's reference but behind the visible body.
## The qualifying band is only a few pixels tall, so the caller re-derives
## the pick right before the verdict click and takes the run's middle row —
## the freshest sample the frame allows, not an interior-margin guess.
## Returns [screen_point, gate_depth, unit_depth] or [] when no such pixel
## exists at this position.
func _beam_overlap_pick(transit_view: Node3D, transit_model: Node3D,
		camera: Camera3D, main: Node, gate_view: Node3D) -> Array:
	var feet_screen: Vector2 = camera.unproject_position(transit_view.global_position)
	var run: Array = []
	for dy in range(4, 40, 3):
		var candidate := feet_screen + Vector2(0.0, -float(dy))
		var ray_o := camera.project_ray_origin(candidate)
		var ray_d := camera.project_ray_normal(candidate)
		var hit: Dictionary = main.call("_building_hit_at", candidate, 1)
		if hit.get("view") != gate_view:
			continue
		var gate_depth := float(hit.get("depth", INF))
		var unit_depth := float(main.call("_model_ray_hit_depth",
				transit_model, ray_o, ray_d))
		if gate_depth >= INF or unit_depth < 0.0 \
				or gate_depth - unit_depth < 0.15:
			continue
		var anchor := (transit_view.global_position \
				+ Vector3(0.0, 0.15, 0.0) - ray_o).dot(ray_d)
		if gate_depth < anchor:
			run.append([candidate, gate_depth, unit_depth])
	if run.is_empty():
		return []
	return run[int(run.size() / 2)]

func _assert_overlapping_building_pick(main: Node, camera: Camera3D, viewport_size: Vector2) -> bool:
	var point := viewport_size * 0.5
	var origin := camera.project_ray_origin(point)
	var direction := camera.project_ray_normal(point)
	var views: Array[Node3D] = []
	# Insert the far view last so a last-hit-wins regression fails.
	for distance in [4.0, 7.0]:
		var view := Node3D.new()
		var model := Node3D.new()
		model.name = "Model"
		var mesh := MeshInstance3D.new()
		mesh.mesh = BoxMesh.new()
		model.add_child(mesh)
		view.add_child(model)
		add_child(view)
		view.global_position = origin + direction * distance
		view.add_to_group("building_views")
		views.append(view)
	var near_first := main.call("_building_at", point, 0) as Node3D
	remove_child(views[0])
	add_child(views[0])
	var near_last := main.call("_building_at", point, 0) as Node3D
	var ok := near_first == views[0] and near_last == views[0]
	for view in views:
		view.free()
	if not ok:
		_fail("overlapping building silhouettes did not pick the foreground model")
	return ok

## Labelled white-box presentation test for unit/building depth picking:
## the game has no foreign buildings and pre-wave runs have no aliens, so
## synthetic views pin the controller's depth-arbitration, occlusion,
## fallback-suppress, and team/fog-filter rules without sending fake ids to
## GrusBridge. Live alien targeting stays in combat_lifecycle_smoke_test.
func _assert_presentation_overlap_pick(main: Node, camera: Camera3D) -> bool:
	var point := get_viewport().get_visible_rect().size * 0.5
	var origin := camera.project_ray_origin(point)
	var direction := camera.project_ray_normal(point)
	var side := (camera.global_transform.basis.x).normalized()
	var views: Array[Node3D] = []
	# Enemy units straddle the box silhouette's ~5.75 depth with distinct
	# projected offsets; the friendly and hidden units sit tighter to the
	# click so a broken filter would win.
	for placement in [[9002, 2, 7.0, 0.2], [9003, 2, 4.5, 0.1], [9004, 1, 3.9, 0.05], [9005, 2, 4.0, 0.05]]:
		var unit := Node3D.new()
		unit.set_meta("team_id", placement[1])
		unit.set_meta("unit_id", placement[0])
		add_child(unit)
		unit.global_position = origin + direction * float(placement[2]) + side * float(placement[3])
		unit.add_to_group("unit_views")
		views.append(unit)
	views[3].visible = false
	# Attach a 3-world-unit sprite under Model so _unit_mesh_hit_at and
	# _building_beats_unit inspect its real mesh: when the click ray meets
	# the unit's Model that mesh depth decides; when the ray hits the lower
	# sprite behind the building (depth ~7.0 > ~5.75), or misses Model and
	# uses the +0.15 y ground anchor, the building still wins.
	var tall_probe := Image.create(16, 96, false, Image.FORMAT_RGBA8)
	tall_probe.fill(Color(0.85, 0.2, 0.2, 1.0))
	var tall_sprite := Sprite3D.new()
	tall_sprite.texture = ImageTexture.create_from_image(tall_probe)
	tall_sprite.pixel_size = 3.0 / 96.0
	tall_sprite.position.y = 1.5
	var tall_model := Node3D.new()
	tall_model.name = "Model"
	tall_model.add_child(tall_sprite)
	views[1].add_child(tall_model)
	var building := Node3D.new()
	building.set_meta("team_id", 2)
	building.set_meta("building_id", 9101)
	var model := Node3D.new()
	model.name = "Model"
	var mesh := MeshInstance3D.new()
	var box := BoxMesh.new()
	# Half-width 0.25 keeps the box an opaque hit for the in-front/behind
	# cases while leaving lateral offsets inside the 20px center radius that
	# miss the silhouette — the only geometry where a genuine fallback-only
	# hit exists at this framing.
	box.size = Vector3(0.5, 1.0, 0.5)
	mesh.mesh = box
	model.add_child(mesh)
	building.add_child(model)
	add_child(building)
	building.global_position = origin + direction * 6.0
	building.add_to_group("building_views")
	views.append(building)

	var enemy: Node3D = main.call("_nearest_enemy_view", point)
	var ok := enemy == views[1]
	if not ok:
		_fail("presentation overlap: ignored in-front enemy unit: view=%s" % str(enemy))
	# With that tall unit behind the opaque silhouette, the ray hits the lower
	# Model at depth ~7.0 > ~5.75, so the building wins — EXCEPT if a second
	# visible enemy unit (`views[0]`) is within 20 px beside it? Park `views[0]`
	# further so only `views[1]` is eligible first, then test the fallback!
	views[0].global_position = origin + direction * 7.0 + side * 5.0
	views[1].global_position = origin + direction * 7.0 + side * 0.1
	enemy = main.call("_nearest_enemy_view", point)
	if enemy != building:
		_fail("presentation overlap: tall unit Model behind opaque silhouette stole the roof: view=%s" % str(enemy))
		ok = false
	# When a hidden mesh-hit unit (`views[1]`) is occluded behind the building
	# while a visible unit (`views[0]`) stands within 20 px in front/beside
	# it, `_unit_pick_at` must fall back to `views[0]` instead of giving the
	# click to the building.
	views[0].global_position = origin + direction * 4.5 + side * 0.2
	enemy = main.call("_nearest_enemy_view", point)
	if enemy != views[0]:
		_fail("presentation overlap: hidden mesh-hit unit behind building masked visible 20px enemy unit: view=%s" % str(enemy))
		ok = false
	# Same fallback-to-visible-unit check for left-click `_select_at` on Team 1.
	views[2].visible = false
	views[0].set_meta("team_id", 1)
	views[1].set_meta("team_id", 1)
	building.set_meta("team_id", 1)
	main.call("_index_views")
	main.selected_ids.clear()
	main.selected_building_id = -1
	main.call("_select_at", point, false)
	if (main.selected_ids as Array) != [9002] or int(main.selected_building_id) != -1:
		_fail("presentation overlap: left-click gave hidden mesh-hit unit click to building instead of visible 20px unit (units=%s building=%s)" % [
			str(main.selected_ids), str(main.selected_building_id)])
		ok = false
	# Left-click `_select_at` with an opaque enemy building hiding a Team-1
	# unit must select neither the hidden unit nor the enemy building.
	views[0].global_position = origin + direction * 7.0 + side * 5.0
	building.set_meta("team_id", 2)
	main.call("_index_views")
	main.selected_ids.clear()
	main.selected_building_id = -1
	main.call("_select_at", point, false)
	if not (main.selected_ids as Array).is_empty() or int(main.selected_building_id) != -1:
		_fail("presentation overlap: left-click selected through opaque enemy building (units=%s building=%s)" % [
			str(main.selected_ids), str(main.selected_building_id)])
		ok = false
	views[2].visible = true
	views[0].set_meta("team_id", 2)
	views[1].set_meta("team_id", 2)
	main.call("_index_views")
	# Restore `views[0]` inside 20 px and park `views[1]` away for the
	# fallback-only enemy building check.
	views[0].global_position = origin + direction * 7.0 + side * 0.2
	views[1].global_position = origin + direction * 7.0 + side * 5.0
	var fallback_ready := false
	for offset in [0.05, 0.1, 0.2, 0.28, 0.32]:
		building.global_position = origin + direction * 6.0 + side * offset
		var fallback_hit: Dictionary = main.call("_building_hit_at", point, 2)
		if fallback_hit.get("view") == building and is_inf(float(fallback_hit.get("depth", INF))):
			fallback_ready = true
			break
	if not fallback_ready:
		_fail("no lateral offset yielded a genuine fallback-only enemy building hit (view with INF depth) for the picker probe")
		ok = false
	enemy = main.call("_nearest_enemy_view", point)
	if enemy != views[0]:
		_fail("presentation overlap: fallback-only enemy building stole the unit: view=%s" % str(enemy))
		ok = false
	# Friendly roof hiding a mesh-only enemy unit — even with a fallback-only
	# or behind-roof enemy building present — must keep the click a move (null).
	building.set_meta("team_id", 1)
	building.global_position = origin + direction * 6.0
	var behind_enemy_building := Node3D.new()
	behind_enemy_building.set_meta("team_id", 2)
	behind_enemy_building.set_meta("building_id", 9103)
	var behind_model := Node3D.new()
	behind_model.name = "Model"
	var behind_mesh := MeshInstance3D.new()
	var behind_box := BoxMesh.new()
	behind_box.size = Vector3(0.5, 1.0, 0.5)
	behind_mesh.mesh = behind_box
	behind_model.add_child(behind_mesh)
	behind_enemy_building.add_child(behind_model)
	add_child(behind_enemy_building)
	behind_enemy_building.global_position = origin + direction * 7.5
	behind_enemy_building.add_to_group("building_views")
	views.append(behind_enemy_building)
	var roof_enemy := Node3D.new()
	roof_enemy.set_meta("team_id", 2)
	roof_enemy.set_meta("unit_id", 9102)
	var roof_model := Node3D.new()
	roof_model.name = "Model"
	var roof_mesh := MeshInstance3D.new()
	var roof_box := BoxMesh.new()
	roof_box.size = Vector3(5.0, 1.0, 5.0)
	roof_mesh.mesh = roof_box
	roof_model.add_child(roof_mesh)
	roof_enemy.add_child(roof_model)
	add_child(roof_enemy)
	roof_enemy.global_position = origin + direction * 8.0 + side * 1.8
	roof_enemy.add_to_group("unit_views")
	views.append(roof_enemy)
	views[0].global_position = origin + direction * 7.0 + side * 5.0
	if camera.unproject_position(roof_enemy.global_position).distance_to(point) <= 20.0:
		_fail("the friendly-roof probe's enemy feet are inside the 20 px fallback radius")
		ok = false
	elif main.call("_unit_mesh_hit_at", point, 2).get("view") != roof_enemy:
		_fail("the friendly-roof probe's enemy mesh does not cover the click point")
		ok = false
	else:
		enemy = main.call("_nearest_enemy_view", point)
		if enemy != null:
			_fail("presentation overlap: friendly roof did not suppress enemy unit/behind-building pick: view=%s" % str(enemy))
			ok = false
	# Hidden enemy views never win _nearest_enemy_view.
	building.visible = false
	behind_enemy_building.visible = false
	roof_enemy.visible = false
	views[0].global_position = origin + direction * 7.0 + side * 0.2
	views[0].visible = false
	if main.call("_nearest_enemy_view", point) != null:
		_fail("presentation overlap: fog-hidden enemy views remained pickable")
		ok = false
	for view in views:
		view.free()
	main.selected_ids.clear()
	main.selected_building_id = -1
	main.call("_index_views")
	return ok

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
	if not _assert_frostline_assets():
		return
	if not _assert_action_icon_paths(get_node("Main") as Node3D):
		return
	if not await _assert_command_tile_treatment():
		return
	if not _assert_command_status_above_selection():
		return
	if not _assert_empty_selection_hierarchy():
		return
	if not _assert_overlapping_building_pick(get_node("Main"), camera, viewport_size):
		return
	if not _assert_presentation_overlap_pick(get_node("Main"), camera):
		return
	# Painted combat-tower muzzles anchor on the sprite plane; unstyled views
	# fall back to world-Y offsets.
	var building_scene_muzzle := load("res://scenes/building_view.tscn") as PackedScene
	for weapon in [["GunTurret", 0.62], ["FlameTower", 0.9], ["CrystalLance", 1.8]]:
		var painted_muzzle_view := building_scene_muzzle.instantiate() as Node3D
		painted_muzzle_view.set_meta("building_kind", weapon[0])
		painted_muzzle_view.scale = Vector3(2.0, 1.0, 2.0)
		painted_muzzle_view.position = Vector3(3.0, 0.0, 5.0)
		add_child(painted_muzzle_view)
		painted_muzzle_view.set_state(true, false)
		await get_tree().process_frame
		var muzzle_sprite: SpriteBase3D = (painted_muzzle_view.get_node("Model") as Node3D).find_children("*", "SpriteBase3D", true, false)[0]
		var painted_muzzle: Vector3 = get_node("Main").call("_attacker_muzzle_position", painted_muzzle_view, "building")
		var plane_offset := absf((painted_muzzle - muzzle_sprite.global_position).dot(muzzle_sprite.global_basis.z))
		if plane_offset > 0.001:
			_fail("%s painted tracer muzzle sits %.4f units off the sprite plane" % [weapon[0], plane_offset])
			painted_muzzle_view.free()
			return
		painted_muzzle_view.free()

		var muzzle_probe := Node3D.new()
		add_child(muzzle_probe)
		muzzle_probe.position = Vector3(3.0, 4.0, 5.0)
		muzzle_probe.set_meta("building_kind", weapon[0])
		var muzzle: Vector3 = get_node("Main").call("_attacker_muzzle_position", muzzle_probe, "building")
		if not muzzle.is_equal_approx(muzzle_probe.global_position + Vector3(0.0, weapon[1], 0.0)):
			_fail("%s unstyled fallback tracer origin does not match its fallback height" % weapon[0])
			muzzle_probe.free()
			return
		muzzle_probe.free()

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
	# First-run controls aid: the Start overlay must surface the controls strip
	# before gameplay opens. The strip renders the shipped mouse/action cues
	# inline, so it is a RichTextLabel whose parsed text still carries every
	# instruction; tokens match case-insensitively against that text and A/S
	# must appear as standalone words, not substrings.
	var controls_node := get_node_or_null("Main/HUD/SessionPanel/ControlsLabel") as Control
	if controls_node == null or not controls_node.is_visible_in_tree():
		_fail("HUD/SessionPanel/ControlsLabel is missing or not visible before Start")
		return
	if not controls_node is RichTextLabel:
		_fail("ControlsLabel does not carry the existing control cues as a RichTextLabel: %s" % [
			controls_node.get_class()])
		return
	var controls_label := controls_node as RichTextLabel
	var controls_text := controls_label.get_parsed_text().to_lower()
	for token in ["right click", "minimap", "esc"]:
		if not controls_text.contains(token):
			_fail("ControlsLabel is missing '%s': %s" % [token, controls_label.get_parsed_text()])
			return
	var control_words := controls_text.replace("\n", " ").split(" ", false)
	for token in ["a", "s"]:
		if not token in control_words:
			_fail("ControlsLabel is missing standalone token '%s': %s" % [
				token, controls_label.get_parsed_text()])
			return
	# Every documented action/key mapping must survive, not just a few tokens:
	# a same-length wrong key (Underground: Tab -> Underground: Halt) would
	# otherwise pass. The parsed text is whitespace-normalised so the painted
	# separators and inline-image placeholders cannot hide a changed mapping.
	var control_mappings := controls_text.replace("\n", " ").replace("·", " ")
	while control_mappings.contains("  "):
		control_mappings = control_mappings.replace("  ", " ")
	control_mappings = control_mappings.strip_edges()
	for mapping in [
			"select: left click / drag / shift",
			"command: right click",
			"attack-move: a then right click",
			"stop: s",
			"pause: esc",
			"underground: tab",
			"camera: wheel / middle-drag",
			"minimap: click to recenter"]:
		if not control_mappings.contains(mapping):
			_fail("ControlsLabel lost the documented mapping '%s': %s" % [
				mapping, controls_label.get_parsed_text()])
			return
	if not controls_label.bbcode_enabled:
		_fail("ControlsLabel does not render its inline control cues")
		return
	var cue_alts := {
		"res://assets/ui/icons/status/mouse_left.png": "Left mouse button",
		"res://assets/ui/icons/status/mouse_right.png": "Right mouse button",
		"res://assets/ui/icons/status/mouse_wheel.png": "Mouse wheel",
		"res://assets/ui/icons/actions/pause.png": "Pause action",
	}
	var cue_alt_error := _controls_cue_alt_error(controls_label, cue_alts)
	if not cue_alt_error.is_empty():
		_fail(cue_alt_error)
		return
	for cue_path in cue_alts:
		if not controls_label.text.contains(cue_path):
			_fail("ControlsLabel lost the existing control cue %s" % cue_path)
			return
	# HPA-698 Start key art, emblem and noninteractive atmosphere: the
	# backdrop is visible outside Playing, every decorative overlay uses
	# mouse IGNORE, the frost overlay is warm-hidden at 15/10, and the
	# always-on vignette carries a real generated gradient.
	var session_backdrop := get_node_or_null("Main/HUD/SessionBackdrop") as Control
	var start_art := get_node_or_null("Main/HUD/SessionBackdrop/StartArt") as TextureRect
	var result_banner := get_node_or_null("Main/HUD/SessionBackdrop/ResultBanner") as TextureRect
	var session_emblem := get_node_or_null("Main/HUD/SessionPanel/SessionEmblem") as TextureRect
	var session_title := get_node_or_null("Main/HUD/SessionPanel/SessionTitle") as Label
	var vignette := get_node_or_null("Main/HUD/Vignette") as TextureRect
	var frost_overlay := get_node_or_null("Main/HUD/Atmosphere/FrostOverlay") as TextureRect
	if session_backdrop == null or start_art == null or result_banner == null \
			or session_emblem == null or session_title == null \
			or vignette == null or frost_overlay == null:
		_fail("Start/Result atmosphere nodes are missing")
		return
	if not session_backdrop.visible or not start_art.visible \
			or start_art.texture == null \
			or not start_art.texture.resource_path.ends_with("screens/start_key_art.png") \
			or not session_emblem.visible or session_emblem.texture == null \
			or not session_emblem.texture.resource_path.ends_with("screens/emblem.png"):
		_fail("Start key art or emblem is missing before Start")
		return
	# The mockup Start lockup is emblem-and-wordmark led: both must be larger
	# than the former 96 px emblem / 60 px title while staying on screen.
	if session_emblem.get_global_rect().size.x < 128.0 \
			or session_emblem.get_global_rect().size.y < 128.0 \
			or not session_title.visible \
			or session_title.get_theme_font_size("font_size") < 72 \
			or not Rect2(Vector2.ZERO, Vector2(TEST_VIEWPORT_SIZE)).encloses(
				session_title.get_global_rect()):
		_fail("Start title/emblem are not large and visible: emblem=%s title_size=%d" % [
			session_emblem.get_global_rect().size, session_title.get_theme_font_size("font_size")])
		return
	# HPA-698 visual fidelity Start layout: three painted-icon objectives, a
	# compact horizontal controls strip and the bright shared primary action,
	# all fully on screen and non-overlapping.
	var session_panel := get_node_or_null("Main/HUD/SessionPanel") as Panel
	var objectives := get_node_or_null("Main/HUD/SessionPanel/Objectives") as VBoxContainer
	var start_button := get_node_or_null("Main/HUD/SessionPanel/StartButton") as Button
	var resume_button := get_node_or_null("Main/HUD/SessionPanel/ResumeButton") as Button
	var restart_button := get_node_or_null("Main/HUD/SessionPanel/RestartButton") as Button
	if session_panel == null or objectives == null or start_button == null \
			or resume_button == null or restart_button == null:
		_fail("Start objectives or session action controls are missing")
		return
	if not objectives.is_visible_in_tree() or objectives.get_child_count() != 3:
		_fail("Start does not show exactly three objectives")
		return
	var viewport_rect := Rect2(Vector2.ZERO, Vector2(TEST_VIEWPORT_SIZE))
	for row in objectives.get_children():
		var objective_icon := row.get_node_or_null("Icon") as TextureRect
		var objective_title := row.get_node_or_null("Text/Title") as Label
		var objective_detail := row.get_node_or_null("Text/Detail") as Label
		if objective_icon == null or objective_icon.texture == null \
				or not objective_icon.is_visible_in_tree() \
				or objective_title == null or objective_title.text.strip_edges().is_empty() \
				or objective_detail == null or objective_detail.text.strip_edges().is_empty():
			_fail("Start objective row is not icon-led with title/detail: %s" % row.name)
			return
	var catalogue_target := int((GrusBridge.catalogue_snapshot().get("research", {}) as Dictionary).get("target_points", 0))
	var opening_prep := int(roundf(float(GrusBridge.wave_snapshot().get("timer_total_seconds", 0.0))))
	var research_detail := objectives.get_child(2).get_node("Text/Detail") as Label
	var survive_detail := objectives.get_child(0).get_node("Text/Detail") as Label
	if catalogue_target <= 0 or not research_detail.text.replace(",", "").contains(str(catalogue_target)):
		_fail("Research objective does not derive its target from the catalogue: %s" % research_detail.text)
		return
	if opening_prep <= 0 or not survive_detail.text.contains("%d:%02d" % [opening_prep / 60, opening_prep % 60]):
		_fail("Survive objective does not derive the initial preparation from the wave snapshot: %s" % survive_detail.text)
		return
	if controls_label.get_global_rect().size.y > 56.0 \
			or controls_label.get_global_rect().size.x < 800.0:
		_fail("ControlsLabel is not a horizontal strip: %s" % controls_label.get_global_rect())
		return
	# The documented instructions must stay legible and fully inside their
	# painted strip: a 13 px floor plus the RichTextLabel's own two-line
	# layout (inline cue images included, no wrapping) against the strip's
	# content area. An empty or hidden strip cannot measure as content.
	if controls_label.get_theme_font_size("normal_font_size") < 13:
		_fail("ControlsLabel font is below the 13 px legibility floor: %d" % [
			controls_label.get_theme_font_size("normal_font_size")])
		return
	var controls_style := controls_label.get_theme_stylebox("normal") as StyleBox
	if controls_style == null:
		_fail("ControlsLabel has no painted strip stylebox")
		return
	var controls_text_size := Vector2(
		controls_label.get_content_width(), controls_label.get_content_height())
	if controls_text_size.x < 600.0 or controls_text_size.y < 24.0:
		_fail("ControlsLabel did not measure its two instruction lines: %s" % controls_text_size)
		return
	var controls_region := controls_label.get_global_rect().size - Vector2(
		controls_style.get_content_margin(SIDE_LEFT) + controls_style.get_content_margin(SIDE_RIGHT),
		controls_style.get_content_margin(SIDE_TOP) + controls_style.get_content_margin(SIDE_BOTTOM))
	if controls_text_size.x > controls_region.x + 0.5 or controls_text_size.y > controls_region.y + 0.5:
		_fail("ControlsLabel text does not fit its allocated region: text=%s region=%s" % [
			controls_text_size, controls_region])
		return
	for control in [objectives, start_button, controls_label]:
		if not viewport_rect.encloses(control.get_global_rect()):
			_fail("Start control %s is offscreen: %s" % [control.name, control.get_global_rect()])
			return
	if objectives.get_global_rect().intersects(start_button.get_global_rect()) \
			or objectives.get_global_rect().intersects(controls_label.get_global_rect()) \
			or start_button.get_global_rect().intersects(controls_label.get_global_rect()):
		_fail("Start objectives/action/controls strip overlap")
		return
	var start_button_rect := start_button.get_global_rect()
	if start_button_rect.size.x < 280.0 or start_button_rect.size.y < 56.0 \
			or not start_button.is_visible_in_tree() or start_button.disabled:
		_fail("Start action is not a visible, reachable call to action: %s" % start_button_rect)
		return
	var start_style := start_button.get_theme_stylebox("normal") as StyleBoxFlat
	var restart_style := restart_button.get_theme_stylebox("normal") as StyleBoxFlat
	if start_style == null or start_style.bg_color.v < 0.5 \
			or restart_style == null or restart_style.bg_color != start_style.bg_color:
		_fail("Start does not carry the shared bright Frostline action style")
		return
	var resume_style := resume_button.get_theme_stylebox("normal") as StyleBoxFlat
	if resume_style == null or resume_style.bg_color != start_style.bg_color \
			or resume_style.border_color != start_style.border_color \
			or resume_button.get_theme_font_size("font_size") < 22:
		_fail("Resume does not carry the shared bright primary action style")
		return
	# The shared plate is loud, so every primary action must carry the readable
	# dark ink and a distinct outside focus ring — the real Start screen is
	# where the boot-time label has to hold up.
	var primary_error := SmokeHelpers.primary_ink_focus_error(
		[start_button, resume_button, restart_button], "Start")
	if not primary_error.is_empty():
		_fail(primary_error)
		return
	var start_capture := await SmokeHelpers.capture_frame(get_tree(), "smoke-start")
	if not start_capture.is_empty():
		_fail(start_capture)
		return
	if result_banner.visible or frost_overlay.visible or vignette.texture == null or not vignette.visible:
		_fail("Result banner/frost/vignette visibility is wrong at the warm Start screen")
		return
	for path in ["Main/HUD/SessionBackdrop", "Main/HUD/SessionBackdrop/StartArt",
			"Main/HUD/SessionBackdrop/Dim", "Main/HUD/SessionBackdrop/ResultBanner",
			"Main/HUD/Vignette", "Main/HUD/Atmosphere", "Main/HUD/Atmosphere/FrostOverlay",
			"Main/HUD/IncomingGlow", "Main/HUD/IncomingGlow/North"]:
		var overlay := get_node_or_null(path) as Control
		if overlay == null or overlay.mouse_filter != Control.MOUSE_FILTER_IGNORE:
			_fail("decorative overlay %s is missing or intercepts mouse input" % path)
			return
	# The Start overlay keeps the first wave timer frozen until the player
	# starts the one-base survival match.
	if not GrusBridge.start_match():
		_fail("start_match rejected the Start -> Playing transition")
		return
	if str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("start_match did not reach Playing")
		return
	if not await _wait_until(func(): return not session_backdrop.visible and vignette.visible,
			5.0, "Playing did not hide the Start backdrop or lost the always-on vignette"):
		return
	# Wave timer geometry from the Rust constants: wave 1 opens on the
	# authored preparation interval and the live remaining can never exceed
	# the snapshot's own total.
	var opening_wave: Dictionary = GrusBridge.wave_snapshot()
	if int(opening_wave.get("next_wave", -1)) != 1 \
			or typeof(opening_wave.get("timer_total_seconds")) != TYPE_FLOAT \
			or not is_equal_approx(float(opening_wave.get("timer_total_seconds", -1.0)), 300.0) \
			or float(opening_wave.get("seconds_remaining", -1.0)) <= 0.0 \
			or float(opening_wave.get("seconds_remaining", 0.0)) > float(opening_wave.get("timer_total_seconds", 0.0)) + 1.0:
		_fail("Playing wave timer total/remaining geometry is wrong: %s" % [opening_wave])
		return
	await get_tree().process_frame
	if controls_label.is_visible_in_tree():
		_fail("ControlsLabel stayed visible after the match reached Playing")
		return
	# HPA-698 map-edge provenance: only an opt-in real-render capture pans
	# west past the authored ground edge, through the real middle-drag path,
	# and restores the exact framing the rest of the journey projects through.
	# A headless or uncaptured run must not move the camera at all. Restoring
	# the logical position is not enough: the renderer reads the physics-
	# interpolated camera transform, so without reset_physics_interpolation
	# later projections stay on the panned frame (minimap.gd does the same
	# after a jump). unit_one stands still here, so its projected point is a
	# stable pixel-level probe of the rendered framing.
	var edge_camera_position := camera.position
	var edge_probe_point := unit_one.global_position
	var edge_probe_screen := camera.unproject_position(edge_probe_point)
	if SmokeHelpers.capture_enabled():
		_mouse_drag(Vector2(200.0, 300.0), Vector2(800.0, 300.0), MOUSE_BUTTON_MIDDLE)
		await get_tree().process_frame
		if camera.position.is_equal_approx(edge_camera_position):
			_fail("map-edge capture pan did not move the camera")
			return
		var edge_capture := await SmokeHelpers.capture_frame(get_tree(), "smoke-map-edge")
		if not edge_capture.is_empty():
			_fail(edge_capture)
			return
		camera.position = edge_camera_position
		camera.reset_physics_interpolation()
		await get_tree().process_frame
	if not camera.global_transform.is_equal_approx(default_camera_transform) \
			or not camera.position.is_equal_approx(edge_camera_position):
		_fail("map-edge capture did not restore the logical camera framing: %s" % camera.position)
		return
	if not await _wait_until(func():
			return camera.get_global_transform_interpolated().is_equal_approx(default_camera_transform) \
				and camera.unproject_position(edge_probe_point).distance_to(edge_probe_screen) < 0.5,
			2.0, "map-edge capture left the rendered camera projection stale",
			func():
				return "logical=%s interpolated=%s projection=%s expected=%s" % [
					camera.global_transform, camera.get_global_transform_interpolated(),
					camera.unproject_position(edge_probe_point), edge_probe_screen]):
		return

	var command_status := get_node("Main/HUD/CommandStatus") as Label
	# Empty ground outside every re-laid-out HUD zone: the old viewport-bottom
	# point now sits under the bottom selection card.
	var clear_screen := Vector2(viewport_size.x * 0.5 + 260.0, viewport_size.y * 0.5 - 60.0)

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
	var research_markers := get_node_or_null("Main/HUD/ResearchPanel/ResearchMarkers") as RichTextLabel
	var wave_hud := get_node("Main/HUD/WaveLabel") as Control
	if research_panel == null or research_bar == null or research_label == null or research_icon == null \
			or analyze_button == null or research_markers == null or wave_hud == null:
		_fail("research HUD nodes are missing under HUD/ResearchPanel")
		return
	# HPA-698 research markers are catalogue-derived: both tower unlock
	# thresholds and the wave-growth milestone, with their planted icons.
	var marker_catalogue: Dictionary = GrusBridge.catalogue_snapshot()
	var marker_buildings: Dictionary = marker_catalogue.get("buildings", {})
	var flame_points := int(marker_buildings.get("FlameTower", {}).get("unlock_points", 0))
	var lance_points := int(marker_buildings.get("CrystalLance", {}).get("unlock_points", 0))
	var wave_points := int(marker_catalogue.get("research", {}).get("wave_growth_points", 0))
	var marker_text := research_markers.get_parsed_text()
	if flame_points <= 0 or lance_points <= 0 or wave_points <= 0 \
			or not marker_text.contains(str(flame_points)) \
			or not marker_text.contains(str(lance_points)) \
			or not marker_text.contains(str(wave_points)) \
			or not research_markers.text.contains("res://assets/ui/icons/buildings/flame_tower.png") \
			or not research_markers.text.contains("res://assets/ui/icons/buildings/crystal_lance.png") \
			or not research_markers.text.contains("res://assets/ui/icons/status/incoming_zone.png"):
		_fail("research markers do not project the catalogue tower/wave thresholds: %s" % [research_markers.text])
		return
	# HPA-698 finding 1: every milestone marker is a named image, not a bare
	# glyph — the existing label is its catalogue-worded alternative.
	var marker_alt_error := SmokeHelpers.inline_image_alt_error(research_markers, {
		"res://assets/ui/icons/buildings/flame_tower.png": "FlameTower",
		"res://assets/ui/icons/buildings/crystal_lance.png": "CrystalLance",
		"res://assets/ui/icons/status/incoming_zone.png": "Wave growth",
	}, "ResearchMarkers")
	if not marker_alt_error.is_empty():
		_fail(marker_alt_error)
		return
	# The milestone row's hover explanation must be reachable from the real
	# pointer, not hidden behind a mouse-ignoring child of the research card.
	var expected_marker_tooltip := "FlameTower at %d research · CrystalLance at %d research · Wave growth at %d research" % [
		flame_points, lance_points, wave_points]
	var marker_tooltip := research_markers.get_tooltip(
		research_markers.get_global_rect().get_center() - research_markers.global_position)
	if research_markers.mouse_filter == Control.MOUSE_FILTER_IGNORE \
			or marker_tooltip != expected_marker_tooltip:
		_fail("research milestone row tooltip is unreachable or incomplete: %s" % marker_tooltip)
		return
	var marker_hover := InputEventMouseMotion.new()
	marker_hover.position = research_markers.get_global_rect().get_center()
	get_tree().root.push_input(marker_hover)
	await get_tree().process_frame
	await get_tree().process_frame
	if get_viewport().gui_get_hovered_control() != research_markers:
		_fail("research milestone row is not the real hover target")
		return
	if not _has_visible_icon(get_node("Main/HUD"), "res://assets/ui/icons/status/research_progress.png"):
		_fail("research HUD is missing status/research_progress.png")
		return
	# The compact top-centre research card sits fully inside the viewport and
	# contains its Analyze button — never over the northwest corridor's or
	# expansion salvage's projected cells, which must stay clickable under the
	# default framing. The salvage point derives from its live view so the
	# probe tracks the authored layout.
	var research_rect := research_panel.get_global_rect()
	if not Rect2(Vector2.ZERO, viewport_size).encloses(research_rect):
		_fail("research panel escapes the viewport: %s" % research_rect)
		return
	# HPA-698 fix round: one geometry assertion covers the live card and the
	# labelled presentation-only boundary probes below. It measures the shaped
	# label lines and milestone content against the fixed column, not only the
	# opening value.
	var research_geometry := _research_geometry_error(research_panel, research_bar,
		research_label, research_markers, analyze_button, "live opening research card")
	if not research_geometry.is_empty():
		_fail(research_geometry)
		return
	# Explicitly labelled presentation-only boundary probes across the bounded
	# research domain: 4-digit near-target/target values, the longest legitimate
	# cold-base ETA (9,000 points / 0.5 s), and 0/1/2/3 reached milestones. No
	# command, spend, sim or Rust value changes. Main._process pauses so the
	# real-render capture sees the probe; the live refresh restores it below.
	var research_hud := get_node("Main/HUD")
	if not research_hud.has_method("refresh_research_controls"):
		_fail("HUD does not expose the research refresh used by the boundary probes")
		return
	var research_target := int(round(research_bar.max_value))
	if research_target <= 0 or flame_points <= 0 or lance_points <= 0 or wave_points <= 0:
		_fail("catalogue research target/thresholds are missing for the boundary probes")
		return
	var research_probes := [
		{"name": "near-target", "points": float(research_target - 1),
			"eta": 2.0, "reached": 3},
		{"name": "target", "points": float(research_target), "eta": 0.0, "reached": 3},
		{"name": "longest cold-base ETA", "points": 0.0, "eta": 18000.0, "reached": 0},
		{"name": "Flame reached", "points": float(flame_points),
			"eta": 13500.0, "reached": 1},
		{"name": "Flame+Lance reached", "points": float(lance_points),
			"eta": 9000.0, "reached": 2},
		{"name": "all reached", "points": float(wave_points),
			"eta": 4500.0, "reached": 3, "capture": true},
	]
	var research_main := get_node("Main") as Node3D
	var live_research: Dictionary = GrusBridge.research_snapshot()
	research_main.set_process(false)
	var probe_error := ""
	for probe in research_probes:
		var points := float(probe["points"])
		var eta := float(probe["eta"])
		var percent := floori(clampf(points, 0.0, float(research_target)) / float(research_target) * 100.0)
		var expected_label := "Research %s/%s\n%d%% · ETA %s" % [
			_grouped_int(floori(points)), _grouped_int(research_target), percent, _eta_text(eta)]
		research_hud.refresh_research_controls({"points": points, "eta_seconds": eta}, true, true)
		if research_label.text != expected_label:
			probe_error = "synthetic %s research label is %s; expected %s" % [
				probe["name"], research_label.text, expected_label]
			break
		var reached_error := _research_reached_error(research_markers.get_parsed_text(),
			points, int(probe["reached"]), flame_points, lance_points, wave_points)
		if not reached_error.is_empty():
			probe_error = "synthetic %s %s" % [probe["name"], reached_error]
			break
		var probe_geometry := _research_geometry_error(research_panel, research_bar,
			research_label, research_markers, analyze_button,
			"synthetic %s research card" % probe["name"])
		if not probe_geometry.is_empty():
			probe_error = probe_geometry
			break
		if bool(probe.get("capture", false)):
			# Keep the capture's build-grid research locks on the live opening
			# state while only the research card carries the probe points; no
			# gameplay command or resource changes, and Main restores the whole
			# HUD on the next live frame.
			_restore_build_locks(float(live_research.get("points", 0.0)))
			var probe_capture := await SmokeHelpers.capture_frame(get_tree(),
				"research-all-reached-probe")
			if not probe_capture.is_empty():
				probe_error = probe_capture
				break
	research_main.set_process(true)
	if not probe_error.is_empty():
		_fail(probe_error)
		return
	# Restore and verify the live read before the journey continues. The probe
	# values must be gone and the displayed point total must track the live
	# snapshot on the restored frame (one point of tick-boundary tolerance).
	await get_tree().process_frame
	await get_tree().process_frame
	var restored_points := -1
	var restored_first_line := research_label.text.split("\n")[0]
	if restored_first_line.begins_with("Research "):
		var restored_digits := restored_first_line.trim_prefix(
			"Research ").split("/")[0].replace(",", "")
		if restored_digits.is_valid_int():
			restored_points = int(restored_digits)
	var live_after_points := floori(float(
		GrusBridge.research_snapshot().get("points", -1.0)))
	if research_markers.get_parsed_text().count("✓") != 0 \
			or research_label.text.split("\n").size() != 2 \
			or not research_label.text.contains(_grouped_int(research_target)) \
			or not research_label.text.contains("ETA ") \
			or restored_points < 0 or live_after_points < 0 \
			or absi(restored_points - live_after_points) > 1:
		_fail("live research read was not restored after the boundary probes: label=%s live=%d markers=%s" % [
			research_label.text, live_after_points, research_markers.get_parsed_text()])
		return
	var restored_geometry := _research_geometry_error(research_panel, research_bar,
		research_label, research_markers, analyze_button, "restored live research card")
	if not restored_geometry.is_empty():
		_fail(restored_geometry)
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
		_fail("research panel covers the expansion salvage at %s: %s (camera logical=%s interpolated=%s default=%s)" % [
			salvage_screen, research_rect, camera.global_transform,
			camera.get_global_transform_interpolated(), default_camera_transform])
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

	# HPA-698 opening framing: a closer default view makes the real starting
	# core and workers larger, while the player must still wheel back out to
	# the former coverage. This leg runs before any unit command: its two
	# opt-in captures share the fixture, camera transform and selection
	# state; only the real wheel zoom (and the live countdown digits that
	# advance while it settles) separates them.
	var framing_capture := await SmokeHelpers.capture_frame(get_tree(), "framing-default")
	if not framing_capture.is_empty():
		_fail(framing_capture)
		return
	if not is_equal_approx(camera.size, INITIAL_CAMERA_SIZE):
		_fail("initial camera size is %.1f, not the %.1f opening framing" % [
			camera.size, INITIAL_CAMERA_SIZE])
		return
	var framing_main := get_node("Main")
	var framing_hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	var starting_workers: Array[Node3D] = [unit_one, unit_two, unit_three, _unit_view(4)]
	for worker in starting_workers:
		if worker == null or int(worker.get_meta("team_id", -1)) != 1:
			_fail("a starting worker view is missing at the opening framing")
			return
		var worker_screen := camera.unproject_position(worker.global_position)
		if camera.is_position_behind(worker.global_position) \
				or not Rect2(Vector2.ZERO, viewport_size).has_point(worker_screen) \
				or framing_hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(worker_screen)):
			_fail("starting worker %s is not visible above the HUD at the opening framing: %s" % [
				worker.get_meta("unit_id", -1), worker_screen])
			return
	var core_screen := camera.unproject_position(research_center_view.global_position)
	if camera.is_position_behind(research_center_view.global_position) \
			or not Rect2(Vector2.ZERO, viewport_size).has_point(core_screen) \
			or framing_hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(core_screen)):
		_fail("starting Research Center is not visible above the HUD at the opening framing: %s" % core_screen)
		return
	# Selectable means the real click path, not a direct state poke.
	_mouse_click(core_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(framing_main.selected_building_id) == 1 and framing_main.selected_ids.is_empty(),
			5.0, "left-click on the starting Research Center selected nothing at the opening framing"):
		return
	for worker in starting_workers:
		var worker_id := int(worker.get_meta("unit_id", -1))
		_mouse_click(camera.unproject_position(worker.global_position), MOUSE_BUTTON_LEFT)
		if not await _wait_until(func():
				return framing_main.selected_ids.size() == 1 \
					and int(framing_main.selected_ids[0]) == worker_id \
					and int(framing_main.selected_building_id) == -1,
				5.0, "left-click on starting worker %d did not select it at the opening framing" % worker_id):
			return
	# HPA-698 opening access: every live starting safe source stays on screen,
	# clear of the HUD, and commandable through the real right-click path at
	# the authored framing. Membership comes from the boot-explored live views
	# (the fixture's six authored safe sources are what team 1 has explored
	# before any scouting), never an id or coordinate table; a worker is still
	# selected here, so the command panel is at its real in-match size.
	var opening_safe_sources: Array[Node3D] = []
	for node in get_tree().get_nodes_in_group("resource_views"):
		var source := node as Node3D
		if source != null and source.is_visible_in_tree():
			opening_safe_sources.append(source)
	if opening_safe_sources.size() != 6:
		_fail("expected the six explored starting safe sources at the opening framing, found %d" % [
			opening_safe_sources.size()])
		return
	var opening_clearance_report := PackedStringArray()
	for source in opening_safe_sources:
		var clearance := SmokeHelpers.default_view_clearance(
			camera, get_node("Main/HUD"), source.global_position)
		opening_clearance_report.append("id=%d %s screen=%s clear=%.1f" % [
			int(source.get_meta("resource_id", -1)), str(source.get_meta("resource_kind", "")),
			camera.unproject_position(source.global_position), clearance])
		if clearance <= 0.0:
			_fail("starting safe source %d is hidden behind the HUD at the opening framing: %s" % [
				int(source.get_meta("resource_id", -1)), "; ".join(opening_clearance_report)])
			return
	# A real gather on each source proves the click reaches the world and Rust
	# acquires that exact source — an accepted Move (a pick miss) also reports
	# "Command accepted", so the authoritative worker task must name the live
	# source id. S then stops the last order so no gather task leaks into the
	# later fixture and stockpile legs.
	var opening_worker_id := int(framing_main.selected_ids[0])
	for source in opening_safe_sources:
		var source_id := int(source.get_meta("resource_id", -1))
		var gather_revision := int(GrusBridge.command_feedback_revision())
		_mouse_click(camera.unproject_position(source.global_position), MOUSE_BUTTON_RIGHT)
		if not await _wait_until(_accepted_since.bind(gather_revision), 5.0,
				"right-click on starting safe source %d was not accepted at the opening framing (screen %s)" % [
					source_id, camera.unproject_position(source.global_position)]):
			return
		if not await _wait_until(func():
				return int(GrusBridge.unit_task_snapshot(opening_worker_id).get("source", -1)) == source_id,
				5.0, "right-click on starting safe source %d was accepted but worker %d did not acquire it: %s" % [
					source_id, opening_worker_id,
					GrusBridge.unit_task_snapshot(opening_worker_id)]):
			return
	_key_tap(KEY_S)
	if not await _wait_until(func():
			return str(GrusBridge.unit_task_snapshot(opening_worker_id).get("task", "")) == "Idle",
			5.0, "the opening safe-source gather probe did not stop worker %d" % opening_worker_id):
		return
	print("SMOKE_OPENING_SAFE_SOURCES %s" % " | ".join(opening_clearance_report))
	var framing_clear := _clear_click_point(viewport_size, framing_main)
	if framing_clear == Vector2.INF:
		_fail("no empty pixel exists to clear selection at the opening framing")
		return
	_mouse_click(framing_clear, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(framing_main.selected_building_id) == -1 \
				and (framing_main.selected_ids as Array).is_empty(),
			5.0, "the opening-framing deselection click did not clear the selection"):
		return
	# Three real wheel steps from 54.0 reach the former 72.0 view and beyond;
	# zooming must not pan or rotate the authored opening transform.
	for _step in 3:
		_mouse_click(viewport_size * 0.5, MOUSE_BUTTON_WHEEL_DOWN)
		await get_tree().process_frame
	if camera.size < PRIOR_CAMERA_SIZE:
		_fail("three wheel-out steps from %.1f reached only %.1f, short of the prior %.1f view" % [
			INITIAL_CAMERA_SIZE, camera.size, PRIOR_CAMERA_SIZE])
		return
	if not camera.global_transform.is_equal_approx(default_camera_transform):
		_fail("wheel zoom-out moved the opening camera transform: %s" % camera.global_transform)
		return
	# The captured frame can trail the camera.size property by a few frames
	# after buffered wheel input: a 3-frame settle still captured the 66.0
	# view, while 10 frames captured the final 72.0 view. Settle frames so the
	# opt-in capture shows the final zoom, not a transitional one.
	for _frame in 10:
		await get_tree().process_frame
	var zoomout_capture := await SmokeHelpers.capture_frame(get_tree(), "framing-zoomout")
	if not zoomout_capture.is_empty():
		_fail(zoomout_capture)
		return
	for _step in 3:
		_mouse_click(viewport_size * 0.5, MOUSE_BUTTON_WHEEL_UP)
		await get_tree().process_frame
	if not camera.global_transform.is_equal_approx(default_camera_transform) \
			or not is_equal_approx(camera.size, default_camera_size):
		_fail("wheel zoom never restored the opening framing: size=%.1f transform=%s" % [
			camera.size, camera.global_transform])
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

	# HPA-291 catalogue structure imports: every kind's GLB sits at the
	# same to_snake_case-derived path building_view.gd resolves, so the
	# hookup probe below proves the site -> finished swap even for kinds
	# this short run never builds. The generic site is checked separately
	# as the only non-catalogue asset.
	var building_view_scene := load("res://scenes/building_view.tscn") as PackedScene
	var retry_view := building_view_scene.instantiate() as Node3D
	retry_view.set_script(RetryBuildingView)
	retry_view.set_meta("building_kind", "Depot")
	add_child(retry_view)
	retry_view._process(0.0)
	if not retry_view.is_processing() or retry_view.get_node_or_null("Model") != null:
		_fail("building view stopped processing after a failed model attachment")
		return
	retry_view._process(0.0)
	if retry_view.is_processing() or retry_view.get_node_or_null("Model") == null:
		_fail("building view did not retry its failed model attachment")
		return
	retry_view.failed_once = false
	retry_view.set_state(true, false)
	if not retry_view.is_processing():
		_fail("building view did not retry a failed completion swap")
		return
	retry_view._process(0.0)
	var retry_model := retry_view.get_node_or_null("Model") as Node3D
	var retry_error := SmokeHelpers.building_model_error(retry_view, "Depot", true)
	if retry_view.is_processing() or retry_model == null or not retry_error.is_empty():
		_fail("building view did not finish the retried completion swap: %s" % retry_error)
		return
	retry_view.free()
	var pending_view := building_view_scene.instantiate() as Node3D
	pending_view.set_script(RetryBuildingView)
	pending_view.set_meta("building_kind", "Depot")
	add_child(pending_view)
	pending_view._process(0.0)
	pending_view.set_state(true, false)
	var pending_model := pending_view.get_node_or_null("Model") as Node3D
	var pending_error := SmokeHelpers.building_model_error(pending_view, "Depot", true)
	if pending_view.is_processing() or pending_model == null or not pending_error.is_empty():
		_fail("a successful completion push did not finish the pending initial attachment")
		return
	pending_view.free()
	# HPA-697 painted-texture imports: every non-Wall/Gate catalogue kind's
	# PNG sits at the same to_snake_case-derived path building_view.gd
	# resolves, audited for mipmaps, alpha, lossless CPU-readable pixels and
	# source identity; Wall/Gate keep their GLB import checks. The shared
	# site texture is the only non-catalogue art.
	if not _assert_painted_import("res://assets/structures/painted/construction_site.png"):
		return
	for kind in GrusBridge.catalogue_snapshot().get("buildings", {}):
		var kind_name := str(kind)
		if kind_name == "Wall" or kind_name == "Gate":
			if not _assert_structure_import("res://assets/structures/%s.glb" % kind_name.to_snake_case()):
				return
		else:
			if not _assert_painted_import("res://assets/structures/painted/%s.png" % kind_name.to_snake_case()):
				return
		var hookup_view := building_view_scene.instantiate() as Node3D
		hookup_view.set_meta("building_kind", kind_name)
		add_child(hookup_view)
		await get_tree().process_frame
		var site_error := SmokeHelpers.building_model_error(hookup_view, kind_name, false)
		if site_error.is_empty():
			hookup_view.set_state(true, false)
			site_error = SmokeHelpers.building_model_error(hookup_view, kind_name, true)
			if not site_error.is_empty():
				site_error = "no finished-presentation swap: %s" % site_error
		else:
			site_error = "no construction-site attach: %s" % site_error
		if not site_error.is_empty():
			_fail("%s view failed the site/finished hookup: %s" % [kind_name, site_error])
			hookup_view.free()
			return
		hookup_view.free()
	# HPA-697 review fix: an opt-in, staged in-world render PROBE of the
	# approved Crystal Lance art on a real 2x2 catalogue transform. No
	# journey can research-unlock the lance, so this is presentation-only
	# staging through the same hookup contract as the loop above, driven
	# only by the view's public set_state/set_health_ratio API — the frame
	# documents the shipped PNG's in-world silhouette, alpha edges, depth
	# ordering and health-bar seat; it is not gameplay evidence of a
	# purchased lance. The staged legs also assert the world bar seats on
	# each stage's real art top (site and finished) on this multicell
	# footprint, in BuildingView-local space — never the transparent quad
	# top, never a world-space write.
	var lance_spec: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {}).get("CrystalLance", {})
	var lance_width := float(lance_spec.get("width", 0))
	var lance_depth := float(lance_spec.get("height", 0))
	if lance_width != 2.0 or lance_depth != 2.0:
		_fail("catalogue exposes no 2x2 CrystalLance footprint for the in-world probe")
		return
	var probe_screen := viewport_size * Vector2(0.5, 0.3)
	var probe_origin := camera.project_ray_origin(probe_screen)
	var probe_direction := camera.project_ray_normal(probe_screen)
	if probe_direction.y >= -0.0001:
		_fail("the lance-probe camera ray never reaches the ground")
		return
	var probe_ground := probe_origin + probe_direction * (-probe_origin.y / probe_direction.y)
	var lance_hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	var lance_anchor := Vector2i(-1, -1)
	for radius in range(0, 10):
		for y in range(-radius, radius + 1):
			for x in range(-radius, radius + 1):
				if maxi(absi(x), absi(y)) != radius:
					continue
				var candidate := Vector2i(floori(probe_ground.x) + x, floori(probe_ground.z) + y)
				# A valid 2x2 Bunkhouse preview proves the pair is free,
				# buildable ground; the probe view is presentation-only.
				var probe_preview: Dictionary = GrusBridge.placement_preview(1, "Bunkhouse", candidate.x, candidate.y)
				if not bool(probe_preview.get("valid", false)):
					continue
				var probe_center_screen := camera.unproject_position(Vector3(float(candidate.x) + lance_width * 0.5, 0.0, float(candidate.y) + lance_depth * 0.5))
				if not Rect2(Vector2.ZERO, viewport_size).has_point(probe_center_screen) \
						or lance_hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(probe_center_screen)):
					continue
				lance_anchor = candidate
				break
			if lance_anchor.x >= 0:
				break
		if lance_anchor.x >= 0:
			break
	if lance_anchor.x < 0:
		_fail("no on-screen 2x2 ground pair exists for the CrystalLance in-world probe")
		return
	var lance_probe := building_view_scene.instantiate() as Node3D
	lance_probe.set_meta("building_kind", "CrystalLance")
	lance_probe.scale = Vector3(lance_width, 1.0, lance_depth)
	lance_probe.position = Vector3(float(lance_anchor.x) + lance_width * 0.5, 0.0, float(lance_anchor.y) + lance_depth * 0.5)
	add_child(lance_probe)
	# Real views get Node3D.visible from the bridge's fog-driven visibility
	# pass; a standalone probe never does, so mirror the explored case here
	# (the anchor scan proved the cells are buildable ground beside the
	# city, i.e. fog-explored).
	lance_probe.visible = true
	await get_tree().process_frame
	var probe_view_script: Script = lance_probe.get_script()
	for staged_complete in [false, true]:
		if bool(staged_complete):
			lance_probe.set_state(true, false)
			await get_tree().process_frame
		var stage_error := SmokeHelpers.building_model_error(lance_probe, "CrystalLance", bool(staged_complete))
		if not stage_error.is_empty():
			_fail("the lance probe view failed its %s stage: %s" % ["finished" if bool(staged_complete) else "site", stage_error])
			lance_probe.free()
			return
		var stage_sprite: SpriteBase3D = (lance_probe.get_node("Model") as Node3D).find_children("*", "SpriteBase3D", true, false)[0]
		var stage_top: Vector2 = probe_view_script.opaque_top_pixels(stage_sprite.texture)
		if stage_top == Vector2.INF:
			_fail("the lance probe %s texture exposes no opaque pixel for the bar seat" % ["finished" if bool(staged_complete) else "site"])
			lance_probe.free()
			return
		var stage_seat: Vector3 = lance_probe.global_transform.affine_inverse() \
				* (stage_sprite.global_transform * probe_view_script.sprite_local_from_pixel(stage_sprite, stage_top))
		if not (lance_probe.get_node("HealthBar") as MeshInstance3D).position.is_equal_approx(Vector3(stage_seat.x, stage_seat.y + 0.15, stage_seat.z)) \
				or (bool(staged_complete) and stage_top.y <= 0.0):
			_fail("the lance probe %s health bar did not seat above the art's top opaque pixel (top row %s)" % ["finished" if bool(staged_complete) else "site", stage_top])
			lance_probe.free()
			return
	lance_probe.set_health_ratio(0.45)
	if not (lance_probe.get_node("HealthBar") as MeshInstance3D).visible:
		_fail("the damaged lance probe did not show its world health bar")
		lance_probe.free()
		return
	# Depth context for the opt-in frame only: park one worker just behind
	# the sprite (the probe cells are free ground — the view is not a real
	# building) so the still reads the worker through the lattice art, then
	# restore its prior spot. The whole maneuver is capture-scoped — in a
	# headless or uncaptured run the two hops would burn march seconds for a
	# frame nothing records, and the restore cannot unburn them.
	if SmokeHelpers.capture_enabled():
		var probe_worker: Node3D = null
		var probe_worker_home := Vector3.INF
		for unit_node in get_tree().get_nodes_in_group("unit_views"):
			var candidate_worker := unit_node as Node3D
			if candidate_worker != null and int(candidate_worker.get_meta("team_id", -1)) == 1:
				probe_worker = candidate_worker
				probe_worker_home = candidate_worker.global_position
				break
		if probe_worker == null:
			_fail("no Team-1 worker remains for the lance probe overlap frame")
			lance_probe.free()
			return
		var behind_cells: Array[Vector2] = [
			Vector2(float(lance_anchor.x) + 1.0, float(lance_anchor.y) + 0.6),
			Vector2(float(lance_anchor.x) + 1.4, float(lance_anchor.y) + 1.0),
			Vector2(float(lance_anchor.x) + 0.6, float(lance_anchor.y) + 1.2),
			Vector2(float(lance_anchor.x) + 1.0, float(lance_anchor.y) - 0.5),
		]
		if await _hop_worker_to_first_cell(probe_worker, behind_cells, 0.55, "lance-probe-overlap") == Vector2.INF:
			lance_probe.free()
			return
		var lance_capture := await SmokeHelpers.capture_frame(get_tree(), "lance-in-world-probe")
		if not lance_capture.is_empty():
			_fail(lance_capture)
			lance_probe.free()
			return
		if not await _hop_worker(probe_worker,
				Vector2(probe_worker_home.x, probe_worker_home.z), 1.0, "lance-probe-restore"):
			lance_probe.free()
			return
	lance_probe.free()
	if not _assert_structure_import("res://assets/structures/wall.glb") \
			or not _assert_structure_import("res://assets/structures/gate.glb"):
		return
	if not _assert_gate_corridor("res://assets/structures/gate.glb"):
		return
	if not _assert_wall_gate_contact("res://assets/structures/wall.glb", "res://assets/structures/gate.glb"):
		return
	if not await _assert_wall_gate_run(building_view_scene):
		return
	# The audit marker only means the checks passed, so it prints after the
	# last one — a bad PNG or GLB must never produce it before the fail.
	print("SMOKE_STRUCTURE_AUDIT source_bytes=%s" % ("editor" if OS.has_feature("editor") else "skipped_packaged"))

	# The view contract behind the live journeys: a state pushed before the
	# kind metadata caches and drives the first attach; identical pushes and
	# cold toggles never re-swap the attached model; _process stops once
	# styled while set_state keeps answering later pushes.
	var early_view := building_view_scene.instantiate() as Node3D
	add_child(early_view)
	early_view.set_state(true, true)
	await get_tree().process_frame
	await get_tree().process_frame
	if not early_view.is_processing() or early_view.get_node_or_null("Model") != null:
		_fail("a view whose kind metadata is still delayed stopped awaiting styling: processing=%s model=%s" % [
			early_view.is_processing(), early_view.get_node_or_null("Model")])
		return
	early_view.set_meta("building_kind", "Depot")
	await get_tree().process_frame
	var early_model := early_view.get_node_or_null("Model") as Node3D
	var cold_tint: Variant = early_view.get_script().get("COLD_TINT")
	if not (cold_tint is Color):
		_fail("building view has no shared cold tint constant")
		return
	if early_model == null:
		_fail("state pushed before kind metadata did not style the completed Depot: model missing")
		return
	var early_sprites := early_model.find_children("*", "SpriteBase3D", true, false)
	if early_sprites.is_empty():
		_fail("the completed Depot did not attach a painted sprite under its Model")
		return
	for node in early_sprites:
		if (node as SpriteBase3D).modulate != (cold_tint as Color):
			_fail("the early cold push did not tint the attached Depot sprite")
			return
	if early_view.is_processing():
		_fail("building view still polls _process after metadata styling")
		return
	for _push in 3:
		early_view.set_state(true, true)
	if early_view.get_node("Model") != early_model:
		_fail("a repeated identical set_state re-swapped the Depot model")
		return
	early_view.set_state(true, false)
	if early_view.get_node("Model") != early_model:
		_fail("a cold -> warm push re-swapped the Depot model")
		return
	for node in early_sprites:
		if (node as SpriteBase3D).modulate != Color.WHITE:
			_fail("a warm push left the cold tint on the Depot sprite")
			return
	early_view.set_state(true, true)
	for node in early_sprites:
		if (node as SpriteBase3D).modulate != (cold_tint as Color):
			_fail("a later cold push did not restore the shared cold tint")
			return
	early_view.free()

	# The site→finished swap probe (HPA-697): a standalone Depot view at the
	# real bridge footprint transform must style the shared site sprite,
	# swap the whole Model node exactly once on completion, and both sprites
	# must answer a real camera ray through the alpha-aware picker — opaque
	# art hits, transparent corners miss.
	var site_view := building_view_scene.instantiate() as Node3D
	site_view.set_meta("building_kind", "Depot")
	var depot_spec: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {}).get("Depot", {})
	var depot_width := float(depot_spec.get("width", 0))
	var depot_depth := float(depot_spec.get("height", 0))
	if depot_width <= 0.0 or depot_depth <= 0.0:
		_fail("catalogue exposes no Depot footprint for the swap probe")
		return
	site_view.scale = Vector3(depot_width, 1.0, depot_depth)
	site_view.position = Vector3(4.5, 0.0, 4.5)
	add_child(site_view)
	await get_tree().process_frame
	var site_model := site_view.get_node_or_null("Model") as Node3D
	var site_style_error := SmokeHelpers.building_model_error(site_view, "Depot", false)
	if site_model == null or site_view.is_processing() or not site_style_error.is_empty():
		_fail("a fresh Depot view did not style the site sprite then stop processing: %s" % site_style_error)
		return
	var swap_controller := get_node("Main") as Node3D
	var view_script: Script = site_view.get_script()
	var ray_depth := func(sprite: SpriteBase3D, pixel: Vector2) -> float:
		var world: Vector3 = sprite.global_transform * view_script.sprite_local_from_pixel(sprite, pixel)
		var screen := camera.unproject_position(world)
		return float(swap_controller.call("_model_ray_hit_depth", sprite.get_parent(),
				camera.project_ray_origin(screen), camera.project_ray_normal(screen)))
	var site_sprite: SpriteBase3D = site_model.find_children("*", "SpriteBase3D", true, false)[0]
	var opaque_top: Vector2 = view_script.opaque_top_pixels(site_sprite.texture)
	if opaque_top == Vector2.INF or float(ray_depth.call(site_sprite, opaque_top + Vector2(0.0, 0.5))) < 0.0:
		_fail("camera-ray alpha picking missed the site sprite's opaque art")
		return
	var transparent_pixel := Vector2.INF
	for pixel in [Vector2(0, 0), Vector2(511, 0), Vector2(0, 511), Vector2(511, 511), Vector2(255, 0)]:
		if not view_script.pixel_opaque(site_sprite.texture, pixel):
			transparent_pixel = pixel
			break
	if transparent_pixel == Vector2.INF:
		_fail("site sprite exposes no transparent corner for the alpha-miss probe")
		return
	if float(ray_depth.call(site_sprite, transparent_pixel)) >= 0.0:
		_fail("camera-ray picking counted a transparent site-sprite pixel")
		return
	site_view.set_state(true, false)
	var finished_model := site_view.get_node_or_null("Model") as Node3D
	var finished_error := SmokeHelpers.building_model_error(site_view, "Depot", true)
	if finished_model == null or finished_model == site_model or not finished_error.is_empty():
		_fail("a post-styling set_state did not swap the site sprite for the Depot sprite: %s" % finished_error)
		return
	var finished_sprite: SpriteBase3D = finished_model.find_children("*", "SpriteBase3D", true, false)[0]
	if float(ray_depth.call(finished_sprite, view_script.opaque_top_pixels(finished_sprite.texture) + Vector2(0.0, 0.5))) < 0.0:
		_fail("building picking lost the Depot's opaque art after the swap")
		return
	site_view.set_state(true, false)
	if site_view.get_node("Model") != finished_model:
		_fail("the completion swap fired more than once")
		return
	var twin_view := building_view_scene.instantiate() as Node3D
	twin_view.set_meta("building_kind", "Depot")
	add_child(twin_view)
	await get_tree().process_frame
	twin_view.set_state(true, true)
	var twin_model := twin_view.get_node_or_null("Model") as Node3D
	if twin_model == null:
		_fail("the repeated same-kind view attached no model")
		return
	var twin_tint: Variant = twin_view.get_script().get("COLD_TINT")
	for node in twin_model.find_children("*", "SpriteBase3D", true, false):
		if (node as SpriteBase3D).modulate != (twin_tint as Color):
			_fail("a repeated same-kind view does not read the one shared cold tint")
			return
	twin_view.free()
	site_view.free()
	# Finding 14 & 15: a BuildingView whose kind lands before its footprint
	# scale updates its holder counter-scale when the root transform changes,
	# and its painted quad pitch matches the live Camera3D pitch.
	var rescaled_view := building_view_scene.instantiate() as Node3D
	rescaled_view.set_meta("building_kind", "ResearchCenter")
	rescaled_view.position = Vector3(12.0, 0.0, 12.0)
	add_child(rescaled_view)
	await get_tree().process_frame
	rescaled_view.set_state(true, false)
	rescaled_view.scale = Vector3(4.0, 1.0, 4.0)
	# Querying _view_art_bounds on a freshly attached Sprite3D before
	# _im_update runs must not permanently cache the 1x1 fallback.
	var uncached_probe := building_view_scene.instantiate() as Node3D
	uncached_probe.set_meta("building_kind", "ResearchCenter")
	uncached_probe.scale = Vector3(4.0, 1.0, 4.0)
	uncached_probe.position = Vector3(12.0, 0.0, 12.0)
	add_child(uncached_probe)
	uncached_probe.set_state(true, false)
	uncached_probe._process(0.0)
	var early_bounds: AABB = get_node("Main").call("_view_art_bounds", uncached_probe, uncached_probe.get_node("Model"))
	if not early_bounds.size.is_finite():
		_fail("early _view_art_bounds returned invalid bounds")
		uncached_probe.free()
		rescaled_view.free()
		return
	await get_tree().process_frame
	var settled_bounds: AABB = get_node("Main").call("_view_art_bounds", uncached_probe, uncached_probe.get_node("Model"))
	if settled_bounds.size.y < 2.0:
		_fail("early _view_art_bounds cached the 1x1 fallback over the 4x4 sprite: %s" % settled_bounds)
		uncached_probe.free()
		rescaled_view.free()
		return
	uncached_probe.free()
	var rescaled_error := SmokeHelpers.view_art_error(rescaled_view)
	if not rescaled_error.is_empty():
		_fail("building view did not recompute holder counter-scale on root transform change: %s" % rescaled_error)
		rescaled_view.free()
		return
	var rescaled_sprite: SpriteBase3D = (rescaled_view.get_node("Model") as Node3D).find_children("*", "SpriteBase3D", true, false)[0]
	if not rescaled_sprite.global_basis.z.is_equal_approx(camera.global_basis.z):
		_fail("painted sprite pitch %s does not match Camera3D pitch %s" % [rescaled_sprite.global_basis.z, camera.global_basis.z])
		rescaled_view.free()
		return
	rescaled_view.free()
	# Same editor-binary gate as the GLB byte audit above: an export
	# compiles building_view.gd to .gdc inside the PCK, so a source-text
	# poll is meaningless there — and an unreadable file must fail, never
	# pass vacuously.
	if OS.has_feature("editor"):
		var view_source := FileAccess.get_file_as_string("res://scripts/building_view.gd")
		if view_source.is_empty():
			_fail("building_view.gd source is unreadable under the editor binary")
			return
		if view_source.contains("GrusBridge"):
			_fail("building_view.gd reads the bridge — state must arrive only via set_state pushes")
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

	# HPA-698 selection detail: one live worker shows exact current/max HP
	# derived from its live view ratio and the Rust catalogue max_health, a
	# worker task line and the Rust carry limit. A mixed selection must hide
	# every per-unit number rather than invent an aggregate.
	var selection_panel := get_node_or_null("Main/HUD/SelectionPanel") as Panel
	var selection_name := get_node_or_null("Main/HUD/SelectionPanel/SelectionName") as Label
	var selection_health := get_node_or_null("Main/HUD/SelectionPanel/HealthBar") as ProgressBar
	var selection_health_label := get_node_or_null("Main/HUD/SelectionPanel/HealthLabel") as Label
	var selection_task := get_node_or_null("Main/HUD/SelectionPanel/SelectionLabel") as Label
	var carry_bar := get_node_or_null("Main/HUD/SelectionPanel/CarryBar") as ProgressBar
	var carry_label := get_node_or_null("Main/HUD/SelectionPanel/CarryLabel") as RichTextLabel
	if selection_panel == null or selection_name == null or selection_health == null \
			or selection_health_label == null or selection_task == null \
			or carry_bar == null or carry_label == null:
		_fail("selection detail nodes are missing under HUD/SelectionPanel")
		return
	var worker_entry: Dictionary = GrusBridge.catalogue_snapshot().get("units", {}).get("Worker", {})
	var worker_max_health := int(worker_entry.get("max_health", 0))
	var worker_health := clampi(roundi(float(unit_one.call("health_ratio")) * worker_max_health), 0, worker_max_health)
	if worker_max_health <= 0:
		_fail("catalogue exposes no Worker max_health for the selection detail")
		return
	if not await _wait_until(func():
			return selection_health.visible \
				and is_equal_approx(selection_health.max_value, float(worker_max_health)) \
				and is_equal_approx(selection_health.value, float(worker_health)) \
				and selection_name.text.to_lower().contains("worker") \
				and selection_health_label.text.contains("%d / %d" % [worker_health, worker_max_health]) \
				and carry_bar.max_value == float(GrusBridge.catalogue_snapshot().get("carry_limit", -1)),
			5.0, "single Worker selection does not render catalogue-backed HP/carry detail"):
		return
	if not selection_panel.tooltip_text.contains("HP: %d" % worker_max_health) \
			or not selection_panel.tooltip_text.contains("Train: %ds" % int(worker_entry.get("seconds", 0))):
		_fail("Worker selection tooltip is missing its catalogue HP/train stats: %s" % [selection_panel.tooltip_text])
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
	var alien_gather_probe_done := false
	# Click points must reach _unhandled_input: skip any spot a HUD Control
	# would consume (top bar, minimap, panels, buttons).
	var hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
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
		# A neighbouring enemy must not swallow a worker's gather click:
		# park a synthetic team-2 unit ~12 px from this click — inside the
		# 20 px fallback radius but carrying no Model the pick ray could
		# hit — so only the radius fallback can ever nominate it. A
		# regression that lets a fallback pick consume the click queues a
		# rejected attack and reads "Attack command queued" here instead.
		if not alien_gather_probe_done:
			alien_gather_probe_done = true
			var alien := Node3D.new()
			alien.set_meta("team_id", 2)
			alien.set_meta("unit_id", 9301)
			alien.add_to_group("unit_views")
			add_child(alien)
			var alien_ground = controller.call("_ground_target", click_at + Vector2(12.0, 0.0))
			if alien_ground == null:
				alien.free()
				_fail("the alien fallback probe found no ground point beside the resource click")
				return
			alien.global_position = Vector3(alien_ground.x, 0.0, alien_ground.y)
			command_status.text = "<no command issued>"
			var alien_feedback := int(GrusBridge.command_feedback_revision())
			var alien_press := InputEventMouseButton.new()
			alien_press.button_index = MOUSE_BUTTON_RIGHT
			alien_press.position = click_at
			alien_press.pressed = true
			get_tree().root.push_input(alien_press)
			var alien_release := InputEventMouseButton.new()
			alien_release.button_index = MOUSE_BUTTON_RIGHT
			alien_release.position = click_at
			alien_release.pressed = false
			get_tree().root.push_input(alien_release)
			alien.free()
			if command_status.text != "Gather command queued":
				_fail("a fallback-only alien within 20 px swallowed a worker gather right-click: %s" % command_status.text)
				return
			if not await _wait_until(_accepted_since.bind(alien_feedback), 10.0,
					"the alien-adjacent gather click was not accepted by the sim"):
				return
			for probe_worker_id in controller.selected_ids:
				var probe_task: Dictionary = GrusBridge.unit_task_snapshot(probe_worker_id)
				if int(probe_task.get("source", -1)) != resource_id:
					_fail("the alien-adjacent gather click tasked worker %d with %s" % [probe_worker_id, probe_task])
					return
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
	await get_tree().process_frame
	if selection_health.visible or carry_bar.visible \
			or not selection_name.text.contains("×2"):
		_fail("mixed selection invented aggregate HP/carry numbers: name=%s hp=%s carry=%s" % [
			selection_name.text, selection_health.visible, carry_bar.visible])
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
	var stop_cell := Vector2i(floori(stop_target.x), floori(stop_target.z))
	var stop_worker_id := int(unit_two.get_meta("unit_id", -1))
	_mouse_click(camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	var stop_start := unit_two.global_position
	for _frame in range(8):
		await get_tree().physics_frame
	# S must interrupt a live march: a click that never issued the move
	# leaves the worker parked and the settle check below passes on a no-op
	# key, and a worker that already reached the target passes on a finished
	# march — both read as a working stop. Require the order demonstrably in
	# flight (moved, still short of the goal) before the key goes down.
	if unit_two.global_position.distance_to(stop_start) < 0.2 \
			or unit_two.global_position.distance_to(stop_target) < 2.0:
		_fail("the stop probe had no live march for S to interrupt (moved=%.2f left=%.2f)" % [
			unit_two.global_position.distance_to(stop_start),
			unit_two.global_position.distance_to(stop_target)])
		return
	# A reserved click cell gets a neighbouring destination slot, so reaching
	# the assigned goal short of the clicked point would still read as a
	# stop. The accepted goal must be the clicked cell, and idle (which
	# excludes MoveOrder owners) must be false while that order is live.
	var stop_task: Dictionary = GrusBridge.unit_task_snapshot(stop_worker_id)
	if int(stop_task.get("move_goal_x", -1)) != stop_cell.x \
			or int(stop_task.get("move_goal_y", -1)) != stop_cell.y:
		_fail("the stop probe's accepted move goal is not the clicked cell: %s vs %s" % [
			str(stop_task), str(stop_cell)])
		return
	if _unit_idle(unit_two):
		_fail("the stop probe worker was idle before S — no live order to interrupt")
		return
	# A live march only proves S had something to interrupt; the label
	# verdict is the one signal a dead key cannot fake — a no-op press
	# leaves the right-click's "Move command queued" standing, and neither
	# arrival nor a mid-march replan ever writes "Stop command queued".
	# push_input dispatches synchronously so the verdict is read before the
	# sim's own accept overwrites the label through the feedback mirror.
	var stop_press := InputEventKey.new()
	stop_press.keycode = KEY_S
	stop_press.pressed = true
	get_tree().root.push_input(stop_press)
	var stop_release := InputEventKey.new()
	stop_release.keycode = KEY_S
	stop_release.pressed = false
	get_tree().root.push_input(stop_release)
	if str(command_status.text) != "Stop command queued":
		_fail("the stop probe's S press did not dispatch through _issue_stop: %s" % command_status.text)
		return
	for _frame in range(2):
		await get_tree().physics_frame
	# The hand-rolled view interpolation can lag the settled sim by several
	# ticks when windowed rendering is slow; sample only after the view
	# converges so the check below tests the stopped order, not the lag.
	var settle_position := unit_two.global_position
	var settled_samples := 0
	for _frame in range(60):
		await get_tree().process_frame
		if unit_two.global_position.distance_to(settle_position) < 0.001:
			settled_samples += 1
			if settled_samples >= 6:
				break
		else:
			settled_samples = 0
			settle_position = unit_two.global_position
	var stopped_position := unit_two.global_position
	for _frame in range(15):
		await get_tree().physics_frame
	if unit_two.global_position.distance_to(stopped_position) > 0.05:
		_fail("Stop input did not settle the active move order")
		return
	# Stop-specific acceptance: idle (which excludes MoveOrder owners) must be
	# true and the goal gone once S lands. Arrival also clears both, but the
	# pre-key goal pin plus the ≥0.4 shortfall below reject a worker that got
	# there on its own.
	if not _unit_idle(unit_two):
		_fail("Stop left the worker busy — the move order survived S")
		return
	if int(GrusBridge.unit_task_snapshot(stop_worker_id).get("move_goal_x", -1)) != -1:
		_fail("Stop left a move goal on the worker")
		return
	# Settled is not stopped: a worker that simply finished the march would
	# pass the checks above on a dead key. The S-application window (key
	# dispatch plus at most a couple of 20 Hz sim ticks) can carry it at
	# most ≈0.6 cells past the pre-check sample, so anything within 0.4 of
	# the goal means the march completed — the key interrupted nothing.
	if unit_two.global_position.distance_to(stop_target) < 0.4:
		_fail("the stopped worker reached its target — S interrupted no live march")
		return

	# HPA-698 modal orders: one controller-owned _armed_order string, armed by
	# a real order-tile click, dispatched by the next real right-click and
	# consumed exactly once. An armed Move must outrank the contextual Gather
	# pick, and Cancel/Esc clear without dispatching or pausing.
	var order_grid := get_node_or_null("Main/HUD/CommandPanel/OrderGrid") as Container
	if order_grid == null:
		_fail("HUD/CommandPanel/OrderGrid is missing")
		return
	var order_buttons := {}
	for order_name in ["MoveButton", "StopButton", "AttackMoveButton", "GatherButton", "RallyButton", "CancelButton"]:
		var order_button := order_grid.get_node_or_null(order_name) as Button
		if order_button == null:
			_fail("order tile %s is missing from OrderGrid" % order_name)
			return
		order_buttons[order_name] = order_button
	for spec in [["MoveButton", "move"], ["StopButton", "stop"], ["AttackMoveButton", "attack_move"],
			["GatherButton", "gather"], ["RallyButton", "rally"], ["CancelButton", "cancel"]]:
		var button: Button = order_buttons[spec[0]]
		var icon_name: String = "cancel" if str(spec[1]) == "cancel" else str(spec[1])
		if spec[1] != "cancel" \
				and (button.icon == null or not button.icon.resource_path.ends_with("actions/%s.png" % icon_name)):
			_fail("%s does not use the painted actions/%s icon" % [spec[0], icon_name])
			return
		if button.tooltip_text.is_empty():
			_fail("%s has no action tooltip" % spec[0])
			return
		if button.size.x < 56.0 or absf(button.size.x - button.size.y) > 0.5:
			_fail("%s does not render the round order plate square: %s" % [spec[0], button.size])
			return
	var fallback_resource := Vector2.INF
	var fallback_resource_id := -1
	var hud_surfaces := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	for resource in get_tree().get_nodes_in_group("resource_views"):
		if not resource.is_visible_in_tree():
			continue
		var resource_screen := camera.unproject_position(resource.global_position)
		if not Rect2(Vector2.ZERO, viewport_size).has_point(resource_screen) \
				or hud_surfaces.any(func(rect: Rect2) -> bool: return rect.has_point(resource_screen)):
			continue
		fallback_resource = resource_screen
		fallback_resource_id = int(resource.get_meta("resource_id", -1))
		break
	if fallback_resource == Vector2.INF:
		_fail("no HUD-clear visible resource for the armed-order fallback probe")
		return
	var move_button: Button = order_buttons["MoveButton"]
	_mouse_click(camera.unproject_position(unit_two.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	SmokeHelpers.push_mouse_click(get_tree(), move_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(get_node("Main").get("_armed_order")) != "move" or not move_button.button_pressed:
		_fail("Move tile did not arm the one controller order state or render pressed")
		return
	var move_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "Move command queued" \
			or str(get_node("Main").get("_armed_order")) != "":
		_fail("armed Move was not consumed by the ground right-click: %s" % command_status.text)
		return
	if not await _wait_until(_accepted_since.bind(move_revision), 10.0,
			"armed Move on open ground was not accepted by the sim"):
		return
	# With Move armed against a resource art target the modal order still
	# wins: the status must be Move, never the contextual Gather fallback.
	SmokeHelpers.push_mouse_click(get_tree(), move_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	SmokeHelpers.push_mouse_click(get_tree(), fallback_resource, MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "Move command queued" \
			or str(get_node("Main").get("_armed_order")) != "":
		_fail("armed Move fell back to Gather or was not consumed by the resource right-click: %s" % command_status.text)
		return
	# Attack-move tile: same modal dispatch through the existing bridge path.
	var attack_button: Button = order_buttons["AttackMoveButton"]
	SmokeHelpers.push_mouse_click(get_tree(), attack_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(get_node("Main").get("_armed_order")) != "attack_move" or not attack_button.button_pressed \
			or move_button.button_pressed:
		_fail("Attack-move tile did not replace the armed order or light only its own tile")
		return
	SmokeHelpers.push_mouse_click(get_tree(), camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "Attack-move queued" \
			or str(get_node("Main").get("_armed_order")) != "":
		_fail("armed Attack-move did not consume into the attack-move command: %s" % command_status.text)
		return
	# Gather tile: requires selected workers and a real resource target.
	_mouse_click(camera.unproject_position(unit_one.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_one).visible:
		_fail("could not reselect Worker 1 for the armed Gather tile")
		return
	var gather_button: Button = order_buttons["GatherButton"]
	SmokeHelpers.push_mouse_click(get_tree(), gather_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(get_node("Main").get("_armed_order")) != "gather" or not gather_button.button_pressed:
		_fail("Gather tile did not arm gather")
		return
	SmokeHelpers.push_mouse_click(get_tree(), fallback_resource, MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "Gather command queued" \
			or str(get_node("Main").get("_armed_order")) != "":
		_fail("armed Gather did not consume into a gather command: %s" % command_status.text)
		return
	# Armed Gather against empty ground rejects: the click still consumes the
	# arming, dispatches no bridge command, and never falls back to Move.
	SmokeHelpers.push_mouse_click(get_tree(), gather_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(get_node("Main").get("_armed_order")) != "gather":
		_fail("Gather tile did not re-arm for the invalid-target regression")
		return
	var invalid_gather_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "Gather command rejected" \
			or str(get_node("Main").get("_armed_order")) != "" \
			or int(GrusBridge.command_feedback_revision()) != invalid_gather_revision:
		_fail("armed Gather on empty ground did not reject without a command or Move fallback: %s" % command_status.text)
		return
	await get_tree().process_frame
	if gather_button.button_pressed:
		_fail("rejected armed Gather left its tile lit")
		return
	# Cancel clears the armed order without a bridge command or a pause.
	if not await _wait_until(_unit_idle.bind(unit_two), 30.0,
			"unit 2 never settled after the armed-order probes"):
		return
	_mouse_click(camera.unproject_position(unit_two.global_position), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	SmokeHelpers.push_mouse_click(get_tree(), move_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	var cancel_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), order_buttons["CancelButton"].get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if str(get_node("Main").get("_armed_order")) != "" \
			or str(command_status.text) != "Order canceled" \
			or int(GrusBridge.command_feedback_revision()) != cancel_revision:
		_fail("Cancel did not clear the armed order without dispatching a command")
		return
	await get_tree().process_frame
	if move_button.button_pressed or str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("Cancel left its armed tile lit or paused the session")
		return
	# Esc clears the armed order before it can pause the Playing session.
	SmokeHelpers.push_mouse_click(get_tree(), attack_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	_key_tap(KEY_ESCAPE)
	await get_tree().process_frame
	if str(get_node("Main").get("_armed_order")) != "" \
			or str(GrusBridge.session_snapshot().get("phase", "")) != "Playing":
		_fail("Esc did not clear the armed order before pausing")
		return
	# Stop tile executes immediately through the existing stop path. The
	# verdict is pinned to the stop dispatch itself: the settled feedback
	# must be an acceptance (not just any later revision bump) and unit two
	# must converge to the stationary gate — ordinary arrival at an old
	# goal must never read as this probe's pass.
	var stop_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), order_buttons["StopButton"].get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if str(command_status.text) != "Stop command queued" \
			or str(get_node("Main").get("_armed_order")) != "":
		_fail("Stop tile did not execute immediately: %s" % command_status.text)
		return
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > stop_revision \
				and str(GrusBridge.command_feedback()).begins_with("Command accepted"),
			5.0, "Stop command never settled an accepted authoritative verdict"):
		return
	if not await _wait_until(_unit_idle.bind(unit_two), 5.0,
			"unit 2 never settled after the Stop dispatch"):
		return
	# Losing the selection clears a modal order instead of leaving it armed.
	SmokeHelpers.push_mouse_click(get_tree(), move_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func(): return int(get_node("Main").get("selected_building_id")) == -1 \
			and (get_node("Main").get("selected_ids") as Array).is_empty() \
			and str(get_node("Main").get("_armed_order")) == "",
			5.0, "selection loss left a modal order armed"):
		return
	SmokeHelpers.push_mouse_click(get_tree(), camera.unproject_position(unit_two.global_position), MOUSE_BUTTON_LEFT)
	if not await _wait_until(func(): return _ring(unit_two).visible,
			5.0, "Worker 2 did not reselect before the placement/order crossover probes"):
		return

	# HPA-698 build tiles mirror the controller-owned placement kind, not
	# transient focus: the armed tile stays lit through focus and selection
	# changes, moves to a newly armed kind, and clears on Cancel without a
	# command.
	var crossover_main := get_node("Main") as Node3D
	var bunkhouse_tile := get_node("Main/HUD/CommandPanel/BuildGrid/BunkhouseButton") as Button
	var wall_tile := get_node("Main/HUD/CommandPanel/BuildGrid/WallButton") as Button
	# HPA-698 command-tile states through the real control tree: the idle
	# field is the quiet plate, a true lock keeps the painted locked frame,
	# and a real click on that lock must not arm or dispatch anything.
	var flame_tile := get_node("Main/HUD/CommandPanel/BuildGrid/FlameTowerButton") as Button
	var locked_card := flame_tile.get_theme_stylebox("disabled") as StyleBoxTexture
	if bunkhouse_tile.disabled or bunkhouse_tile.theme_type_variation != &"CommandTile" \
			or not flame_tile.disabled or locked_card == null or locked_card.texture == null \
			or not locked_card.texture.resource_path.ends_with("tile_locked.png"):
		_fail("command tiles do not present the quiet idle plate and painted lock: bunkhouse=%s disabled=%s flame_disabled=%s lock_plate=%s" % [
			bunkhouse_tile.theme_type_variation, bunkhouse_tile.disabled,
			flame_tile.disabled, locked_card])
		return
	# The normal-state frame keeps its one test-set focus removed: a ring on an
	# unrelated control would misread as a command-tile focus state.
	if SmokeHelpers.capture_enabled():
		get_viewport().gui_release_focus()
	var normal_tile_capture := await SmokeHelpers.capture_frame(get_tree(), "command-tiles-normal")
	if not normal_tile_capture.is_empty():
		_fail(normal_tile_capture)
		return
	# Pointer-hover evidence on the quiet plate: real motion over the still
	# affordable Depot tile, captured after the frame settles.
	if SmokeHelpers.capture_enabled():
		var depot_tile := get_node("Main/HUD/CommandPanel/BuildGrid/DepotButton") as Button
		var hover_motion := InputEventMouseMotion.new()
		hover_motion.position = depot_tile.get_global_rect().get_center()
		get_tree().root.push_input(hover_motion)
		await get_tree().process_frame
		await get_tree().process_frame
	var hover_tile_capture := await SmokeHelpers.capture_frame(get_tree(), "command-tiles-hover")
	if not hover_tile_capture.is_empty():
		_fail(hover_tile_capture)
		return
	var locked_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), flame_tile.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "" \
			or int(GrusBridge.command_feedback_revision()) != locked_revision \
			or not _ring(unit_two).visible:
		_fail("locked build tile dispatched a command, armed placement or dropped the selection")
		return
	if not await _arm_defense(crossover_main, bunkhouse_tile, "Bunkhouse"):
		return
	move_button.grab_focus()
	await get_tree().process_frame
	await get_tree().process_frame
	if not bunkhouse_tile.button_pressed or wall_tile.button_pressed:
		_fail("armed Bunkhouse tile lost or shared its placement state after focus moved")
		return
	# Focus must read as its own bright ring outside the plate — never as the
	# armed cyan frame or the quiet hover edge — while the Bunkhouse stays the
	# one armed tile. HPA-698 fix round: Wall's focus is reached with real
	# keyboard navigation, not grab_focus. Return focus to the armed Bunkhouse
	# (the real click's focus was moved by the placement check above), then
	# walk the build grid with real KEY_RIGHT events.
	bunkhouse_tile.grab_focus()
	await get_tree().process_frame
	var focus_owner: Control = get_viewport().gui_get_focus_owner()
	for _step in 4:
		if focus_owner == wall_tile:
			break
		_key_tap(KEY_RIGHT)
		await get_tree().process_frame
		focus_owner = get_viewport().gui_get_focus_owner()
	var focus_ring := wall_tile.get_theme_stylebox("focus") as StyleBoxFlat
	if focus_owner != wall_tile \
			or focus_ring == null or focus_ring.draw_center \
			or focus_ring.border_width_left < 2 \
			or focus_ring.get_expand_margin(SIDE_LEFT) < 2.0 \
			or SmokeHelpers.linear_luminance(focus_ring.border_color) < 0.8 \
			or not bunkhouse_tile.button_pressed or wall_tile.button_pressed \
			or str(crossover_main.get("_placement_kind")) != "Bunkhouse":
		_fail("real keyboard navigation did not reach Wall with a bright ring while Bunkhouse stayed armed: owner=%s ring=%s placement=%s" % [
			focus_owner, focus_ring, str(crossover_main.get("_placement_kind"))])
		return
	# HPA-698 readability follow-up: the price band stays legible in the armed
	# and keyboard-focused tile states, not only the normal field.
	var armed_price := SmokeHelpers.command_price_error(bunkhouse_tile, "armed Bunkhouse")
	if not armed_price.is_empty():
		_fail(armed_price)
		return
	var focused_price := SmokeHelpers.command_price_error(wall_tile, "focused Wall")
	if not focused_price.is_empty():
		_fail(focused_price)
		return
	# HPA-698 fix round: where a real renderer exists, prove the configured
	# focus ring is actually painted outside the Wall plate while an unfocused
	# tile's same outside band stays dark. Headless has no viewport pixels; the
	# windowed run prints the counts. The focused tile's price band was checked
	# above, so the ring and the price are evidence of the same state.
	var depot_compare := get_node("Main/HUD/CommandPanel/BuildGrid/DepotButton") as Button
	var rendered_focus := await _rendered_focus_error(wall_tile, depot_compare,
		"keyboard-focused Wall")
	if not rendered_focus.is_empty():
		_fail(rendered_focus)
		return
	# Opt-in labelled evidence of the real keyboard-navigation focus state.
	var keyboard_focus_capture := await SmokeHelpers.capture_frame(get_tree(),
		"command-tiles-keyboard-focus")
	if not keyboard_focus_capture.is_empty():
		_fail(keyboard_focus_capture)
		return
	# Rendered evidence case: the controller's real long armed hint must keep
	# its scoped dark edge and fit while it crosses snowy ground above the card.
	var armed_status := crossover_main.get("command_status") as Label
	if armed_status == null \
			or armed_status.text != "Placing Bunkhouse — left-click to place, right-click or Esc to cancel":
		_fail("armed hint is not the controller's real placement text: %s" % [
			armed_status.text if armed_status != null else "<missing>"])
		return
	var armed_readability := _command_status_readability_error(armed_status, "armed hint")
	if not armed_readability.is_empty():
		_fail(armed_readability)
		return
	var armed_tile_capture := await SmokeHelpers.capture_frame(get_tree(), "command-tiles-armed")
	if not armed_tile_capture.is_empty():
		_fail(armed_tile_capture)
		return
	_key_tap(KEY_1)
	await get_tree().process_frame
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "Bunkhouse" \
			or not bunkhouse_tile.button_pressed or not _ring(unit_one).visible:
		_fail("real selection transition cleared the armed build tile or its placement")
		return
	var tile_transition_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), wall_tile.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "Wall" \
			or not wall_tile.button_pressed or bunkhouse_tile.button_pressed \
			or int(GrusBridge.command_feedback_revision()) != tile_transition_revision:
		_fail("build-tile transition left the previous tile lit or dispatched a command")
		return
	SmokeHelpers.push_mouse_click(get_tree(), order_buttons["CancelButton"].get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "" or wall_tile.button_pressed \
			or bunkhouse_tile.button_pressed or command_status.text != "Placement canceled" \
			or int(GrusBridge.command_feedback_revision()) != tile_transition_revision:
		_fail("Cancel left an armed build tile lit or dispatched a command")
		return

	# HPA-698 targeting modes are mutually exclusive. A build tile and an
	# armed order may not coexist, in either entry order, and keyboard A must
	# leave placement before arming Attack-move. The next right-click must
	# execute the modal order rather than merely canceling placement.
	if not _ring(unit_two).visible:
		_fail("Worker 2 is not selected before the placement/order crossover probes")
		return
	SmokeHelpers.push_mouse_click(get_tree(), bunkhouse_tile.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "Bunkhouse" or str(crossover_main.get("_armed_order")) != "":
		_fail("build tile did not enter placement exclusively: placement=%s order=%s" % [
			crossover_main.get("_placement_kind"), crossover_main.get("_armed_order")])
		return
	SmokeHelpers.push_mouse_click(get_tree(), move_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "" or str(crossover_main.get("_armed_order")) != "move":
		_fail("Move after Build left both targeting modes active: placement=%s order=%s" % [
			crossover_main.get("_placement_kind"), crossover_main.get("_armed_order")])
		return
	var crossover_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "Move command queued" \
			or str(crossover_main.get("_placement_kind")) != "" \
			or str(crossover_main.get("_armed_order")) != "":
		_fail("Build→Move right-click canceled placement instead of executing Move: %s" % command_status.text)
		return
	if not await _wait_until(_accepted_since.bind(crossover_revision), 10.0,
			"Build→Move crossover was not accepted by the sim"):
		return
	# Reverse: arming Move first, then entering placement must clear the order.
	SmokeHelpers.push_mouse_click(get_tree(), move_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	SmokeHelpers.push_mouse_click(get_tree(), bunkhouse_tile.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "Bunkhouse" or str(crossover_main.get("_armed_order")) != "":
		_fail("Build after Move left a latent armed order: placement=%s order=%s" % [
			crossover_main.get("_placement_kind"), crossover_main.get("_armed_order")])
		return
	var placement_cancel_revision := int(GrusBridge.command_feedback_revision())
	command_status.text = "<placement right-click>"
	SmokeHelpers.push_mouse_click(get_tree(), camera.unproject_position(stop_target), MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "<placement right-click>" \
			or str(crossover_main.get("_placement_kind")) != "" \
			or str(crossover_main.get("_armed_order")) != "" \
			or int(GrusBridge.command_feedback_revision()) != placement_cancel_revision:
		_fail("placement right-click dispatched a latent order or changed feedback")
		return
	# Keyboard A while placement is active must cancel placement first.
	SmokeHelpers.push_mouse_click(get_tree(), bunkhouse_tile.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	_key_tap(KEY_A)
	await get_tree().process_frame
	if str(crossover_main.get("_placement_kind")) != "" or str(crossover_main.get("_armed_order")) != "attack_move":
		_fail("keyboard A did not leave placement before arming attack-move: placement=%s order=%s" % [
			crossover_main.get("_placement_kind"), crossover_main.get("_armed_order")])
		return
	var crossover_attack_revision := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), camera.unproject_position(target), MOUSE_BUTTON_RIGHT)
	if str(command_status.text) != "Attack-move queued" \
			or str(crossover_main.get("_armed_order")) != "":
		_fail("A armed attack-move did not win the placement crossover: %s" % command_status.text)
		return
	# The worker is a non-combatant, so the authoritative outcome of this
	# intentional Attack-move is the typed NotCombatant reject — still a
	# legitimate command that advances the feedback revision. Settle that
	# verdict before the panel-leak probes capture their own baseline,
	# otherwise a fixed tick during either panel check looks exactly like UI
	# leakage. (An accepted unit attack-move is covered by the Guard journey in
	# combat_lifecycle_smoke_test.)
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > crossover_attack_revision \
				and int(GrusBridge.last_reject_code()) == REJECT_NOT_COMBATANT \
				and str(GrusBridge.command_feedback()).contains("NotCombatant"),
			10.0, "A armed attack-move crossover feedback did not settle"):
		return

	# The obsolete InputShield rectangle is gone: the full-width top bar's own
	# STOP panel is the shield, and clicking its empty face must never select a
	# world object or queue a command.
	if get_node_or_null("Main/HUD/InputShield") != null:
		_fail("obsolete HUD InputShield still exists beside the full-width top bar")
		return
	var top_bar := get_node_or_null("Main/HUD/EconomyPanel") as Control
	if top_bar == null or not top_bar.is_visible_in_tree() \
			or not is_equal_approx(top_bar.get_global_rect().size.x, viewport_size.x):
		_fail("HUD top bar is missing or not the full-width STOP surface")
		return
	var feedback_before_ui := int(GrusBridge.command_feedback_revision())
	_mouse_click(top_bar.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	if not _ring(unit_two).visible or int(GrusBridge.command_feedback_revision()) != feedback_before_ui:
		_fail("top-bar click leaked into world controls")
		return

	# ResearchPanel shares the upper band with the full-width top bar; its
	# STOP filter must likewise keep panel clicks from clearing selection or
	# queuing orders.
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
	# The unreachable verdict arrives via the bridge feedback poll, so wait
	# on the settled status rather than a fixed frame count that slow
	# windowed rendering can burn inside one rendered frame.
	if not await _wait_until(func():
			return command_status.text.to_lower().contains("unreachable"),
			10.0, "blocked destination did not surface unreachable feedback"):
		return

	var cadence_start := unit_three.global_position
	if not GrusBridge.move_units(PackedInt32Array([3]), Vector2(36.5, 44.5)):
		_fail("20 Hz cadence probe move was rejected")
		return
	var cadence_started_ms := Time.get_ticks_msec()
	var previous_visual := unit_three.global_position
	var previous_visual_ms := cadence_started_ms
	var step_deltas: Array[float] = []
	var step_elapsed_s: Array[float] = []
	var visual_changes := 0
	var frames_observed := 0
	while Time.get_ticks_msec() - cadence_started_ms < 1000:
		await get_tree().process_frame
		frames_observed += 1
		var visual_position := unit_three.global_position
		var visual_step := visual_position.distance_to(previous_visual)
		if visual_step > 0.0001:
			visual_changes += 1
			var visual_ms := Time.get_ticks_msec()
			step_deltas.append(visual_step)
			step_elapsed_s.append(float(visual_ms - previous_visual_ms) / 1000.0)
		previous_visual = visual_position
		previous_visual_ms = Time.get_ticks_msec()

	var cadence_distance := unit_three.global_position.distance_to(cadence_start)
	if cadence_distance < 4.0 or cadence_distance > 8.0:
		_fail("authoritative simulation is not running near 20 Hz: speed-6 unit moved %.2f units in one second" % cadence_distance)
		return
	# Interpolated presentation must track real time: each visual step covers
	# ~the measured speed × wall-clock Δt, plus at most one 20 Hz tick of
	# quantization and 25% tolerance. An absolute step bound reads as a
	# fault on slow (software-rendered) hosts even though pacing is correct
	# there (the old fixed 0.25 failed 12 of 13 saturated-host runs per the
	# original comment), while a frozen-then-jumping or sim-leading view
	# still trips the excess.
	var worst_excess := 0.0
	var max_subtick_step := 0.0
	for i in range(step_deltas.size()):
		worst_excess = maxf(worst_excess,
			step_deltas[i] - (cadence_distance * step_elapsed_s[i] + cadence_distance * 0.05) * 1.25)
		if step_elapsed_s[i] <= 0.032:
			max_subtick_step = maxf(max_subtick_step, step_deltas[i])
	if worst_excess > 0.25 or visual_changes < 2:
		_fail("20 Hz presentation is not interpolated: worst step excess %.3f across %d visual changes" % [worst_excess, visual_changes])
		return

	# Pacing alone cannot reject interpolation loss: its per-step allowance
	# includes a full 20 Hz tick of travel, so a view that snaps the whole
	# tick displacement (speed 6 → 0.30 units per tick, no lerp) always slips
	# under it. Require most sub-tick frames to move when mean_frame_s < 0.045
	# and reject full-tick jumps (> 0.25) on sub-tick intervals (<= 32 ms);
	# multi-tick software rendering (mean_frame_s >= 0.045) stays on the
	# wall-clock pacing bound above.
	var window_elapsed_s := float(Time.get_ticks_msec() - cadence_started_ms) / 1000.0
	var mean_frame_s := window_elapsed_s / maxf(1.0, float(frames_observed))
	print("GRUS_CADENCE_MOTION frames=%d changes=%d mean_frame_s=%.4f worst_excess=%.3f max_subtick_step=%.3f" % [
		frames_observed, visual_changes, mean_frame_s, worst_excess, max_subtick_step])
	if (mean_frame_s < 0.025 and visual_changes * 5 < frames_observed * 3) \
			or (mean_frame_s >= 0.025 and mean_frame_s < 0.045 \
				and (visual_changes * 5 < frames_observed * 4 or max_subtick_step > 0.25)):
		_fail("20 Hz view snaps between ticks without interpolation: %d of %d rendered frames showed motion (mean frame %.4fs, max sub-tick step %.3f)" % [
			visual_changes, frames_observed, mean_frame_s, max_subtick_step])
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
	# Mockup framing: the interactive map fills a readable 4:3 frame at the
	# bottom left; its normalized click mapping keeps one cell per pixel
	# semantic (see _minimap_click_point and minimap.gd).
	if minimap.size.x < 180.0 or minimap.size.y < 135.0 \
			or absf(minimap.size.x / minimap.size.y - 128.0 / 96.0) > 0.01:
		_fail("minimap is not the readable 4:3 framed map: %s" % minimap.size)
		return

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
	var click_at := _minimap_click_point(minimap, Vector2(20, 80))
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
	# input. The minimap leg left the camera on the southwest corner where the
	# re-laid-out bottom cards now cover Worker 1; recenter through the same
	# real minimap path first, then click the unit at the clear viewport centre.
	SmokeHelpers.push_mouse_click(get_tree(),
		_minimap_click_point(minimap, Vector2(unit_one.global_position.x, unit_one.global_position.z)),
		MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
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

	# HPA-291/HPA-697: an unfinished building renders the shared painted
	# construction-site sprite until Rust pushes complete — the freshest
	# Gate (8s build) is still mid-construction here.
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
	var gate_site_error := SmokeHelpers.building_model_error(freshest_gate, "Gate", false)
	if bool(GrusBridge.building_snapshot(freshest_gate_id).get("complete", false)) \
				or not gate_site_error.is_empty():
		_fail("unfinished Gate does not show the generic construction site: %s" % gate_site_error)
		return
	# Opt-in real-render evidence of the shared construction-site sprite.
	var site_capture := await SmokeHelpers.capture_frame(get_tree(), "construction-site")
	if not site_capture.is_empty():
		_fail(site_capture)
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
	# The Esc key flushes with the next process frame, so wait on the
	# placement mode clearing rather than a fixed physics-frame count that
	# slow windowed rendering can burn before the flush.
	if not await _wait_until(func():
			return str(main.get("_placement_kind")) == "",
			10.0, "Esc did not cancel Gate placement"):
		return
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
	# The wave-growth marker is the Rust `unlock_points(75%)` milestone:
	# 6,750 of the 9,000-point victory target, between the two tower
	# unlocks.
	var wave_growth_points := int(research_catalogue.get("wave_growth_points", 0))
	if wave_growth_points != 6750:
		_fail("catalogue wave_growth_points is not the Rust wave-boost milestone: %s" % [research_catalogue])
		return
	if not analyze_button.text.contains(str(int(analyze_cost.get("crystals", -1)))) \
			or not analyze_button.text.contains(str(analyze_gain)):
		_fail("Analyze button does not render the catalogue cost/gain: %s" % [analyze_button.text])
		return

	# The two advanced towers fill the last cells of the 5x2 BuildGrid and
	# bind their catalogue unlock thresholds; every other kind binds zero.
	var build_grid := get_node("Main/HUD/CommandPanel/BuildGrid") as GridContainer
	if build_grid.columns != 5 or build_grid.get_child_count() != 9:
		_fail("BuildGrid is not the 5x2 layout with the two advanced towers: cols=%d children=%d" % [build_grid.columns, build_grid.get_child_count()])
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
		_minimap_click_point(minimap, Vector2(rc_view.global_position.x, rc_view.global_position.z)),
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
	# The same single-selection HP contract applies to a live building: the
	# view ratio times the catalogue max_health, never a stored duplicate.
	var rc_entry: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {}).get("ResearchCenter", {})
	var rc_max_health := int(rc_entry.get("max_health", 0))
	var rc_health := clampi(roundi(float(rc_view.call("health_ratio")) * rc_max_health), 0, rc_max_health)
	if not await _wait_until(func():
			return selection_health.visible \
				and is_equal_approx(selection_health.max_value, float(rc_max_health)) \
				and is_equal_approx(selection_health.value, float(rc_health)) \
				and selection_name.text.to_lower().contains("research") \
				and selection_health_label.text.contains("%d / %d" % [rc_health, rc_max_health]) \
				and selection_panel.tooltip_text.contains("HP: %d" % rc_max_health) \
				and selection_panel.tooltip_text.contains("Heat:"),
			5.0, "single Research Center selection does not render catalogue-backed HP/heat detail"):
		return
	# Opt-in real-render evidence: the selected painted Research Center with
	# its HUD HP/heat detail over the recentred world art.
	var selected_capture := await SmokeHelpers.capture_frame(get_tree(), "selected-building")
	if not selected_capture.is_empty():
		_fail(selected_capture)
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
	var silhouette_point := _roof_pick_point(rc_view, camera, viewport_size, main)
	if silhouette_point == Vector2.INF:
		_fail("Research Center roof mesh exposes no upward triangle projecting beyond the old center radius")
		return
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
	# Deselect on open ground: Analyze intentionally remains enabled while
	# Playing with a living Research Center, and a real click still queues the
	# command for the controller-resolved live core. With zero Crystals the
	# sim's typed InsufficientResources verdict is the only honest outcome.
	_mouse_click(clear_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == -1 \
				and main.selected_ids.is_empty() and not analyze_button.disabled,
			5.0, "Analyze disabled after deselection while Playing with a live Research Center"):
		return
	var crystals_before_deselected := int(GrusBridge.economy_snapshot().get("crystals", -1))
	var points_before_deselected := float(GrusBridge.research_snapshot().get("points", -1.0))
	var revision_before_deselected := int(GrusBridge.command_feedback_revision())
	SmokeHelpers.push_mouse_click(get_tree(), analyze_button.get_global_rect().get_center(), MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(GrusBridge.command_feedback_revision()) > revision_before_deselected,
			10.0, "selection-free Analyze press produced no sim verdict"):
		return
	if crystals_before_deselected < analyze_price:
		if int(GrusBridge.last_reject_code()) != 8 \
				or str(GrusBridge.command_feedback()) != "Command rejected (InsufficientResources)":
			_fail("selection-free zero-Crystal Analyze did not surface code 8: code=%d feedback=%s" % [
				int(GrusBridge.last_reject_code()), GrusBridge.command_feedback()])
			return
		if int(GrusBridge.economy_snapshot().get("crystals", -1)) != crystals_before_deselected:
			_fail("selection-free rejected Analyze changed the Crystal ledger")
			return
		if float(GrusBridge.research_snapshot().get("points", -1.0)) - points_before_deselected >= float(analyze_gain):
			_fail("selection-free rejected Analyze banked the point reward")
			return
	elif not str(GrusBridge.command_feedback()).begins_with("Command accepted"):
		_fail("selection-free affordable Analyze was not accepted: %s" % GrusBridge.command_feedback())
		return

	var roof_bunkhouse: Node3D = null
	var roof_bunkhouse_id := -1
	var resume_bunkhouse: Node3D = null
	var resume_bunkhouse_id := -1
	for node in get_tree().get_nodes_in_group("building_views"):
		var view := node as Node3D
		if str(view.get_meta("building_kind", "")) != "Bunkhouse":
			continue
		var id := int(view.get_meta("building_id", -1))
		if bool(GrusBridge.building_snapshot(id).get("complete", false)):
			roof_bunkhouse = view
			roof_bunkhouse_id = id
			break
		resume_bunkhouse = view
		resume_bunkhouse_id = id
	if roof_bunkhouse == null and resume_bunkhouse != null:
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
		roof_bunkhouse = resume_bunkhouse
		roof_bunkhouse_id = resume_bunkhouse_id
	if roof_bunkhouse == null:
		_fail("no placed Bunkhouse exists for the roof silhouette pick")
		return
	if not await _wait_until(func():
			return SmokeHelpers.building_model_error(roof_bunkhouse, "Bunkhouse", true).is_empty(),
			10.0, "completed Bunkhouse never displayed its painted sprite"):
		return
	# The 2x2 cabin is too small at the default framing to probe a point
	# past the old 20 px center radius, so frame it like the resource-pick
	# leg: minimap-recenter the view onto it and wheel-zoom to the minimum
	# size. Both are the real input paths the game itself handles.
	var bunkhouse_zoom_start := camera.size
	var bunkhouse_world := roof_bunkhouse.global_position
	var viewport_center := viewport_size * 0.5
	var minimap_point := _minimap_click_point(minimap, Vector2(bunkhouse_world.x, bunkhouse_world.z))
	SmokeHelpers.push_mouse_click(get_tree(), minimap_point, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	var bunkhouse_screen := camera.unproject_position(roof_bunkhouse.global_position)
	if bunkhouse_screen.distance_to(viewport_center) > 60.0:
		_fail("minimap recenter never centered the roof-pick Bunkhouse: %s" % bunkhouse_screen)
		return
	for _step in range(16):
		if camera.size <= controller.MIN_CAMERA_SIZE:
			break
		_mouse_click(viewport_center, MOUSE_BUTTON_WHEEL_UP)
		await get_tree().process_frame
	bunkhouse_screen = camera.unproject_position(roof_bunkhouse.global_position)
	var roof_point := _roof_pick_point(roof_bunkhouse, camera, viewport_size, main)
	if roof_point == Vector2.INF:
		_fail("Bunkhouse painted silhouette exposes no opaque art pixel beyond the old center radius")
		return
	_mouse_click(roof_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == roof_bunkhouse_id \
				and main.selected_ids.is_empty(),
			5.0, "left-click on the Bunkhouse's projected roof did not select its building"):
		return

	# The same geometry-proven pick on the seeded 4x4 Research Center GLB,
	# still at minimum zoom: recenter through the real minimap path, click a
	# live roof triangle beyond the old center radius.
	var rc_roof_world := rc_view.global_position
	var rc_minimap_point := _minimap_click_point(minimap, Vector2(rc_roof_world.x, rc_roof_world.z))
	SmokeHelpers.push_mouse_click(get_tree(), rc_minimap_point, MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	var rc_screen_zoomed := camera.unproject_position(rc_view.global_position)
	if rc_screen_zoomed.distance_to(viewport_center) > 60.0:
		_fail("minimap recenter never centered the Research Center for the roof pick: %s" % rc_screen_zoomed)
		return
	# At this framing at least one visible bounding-box corner must be empty.
	var rc_art_rect: Rect2 = main.call("_projected_art_rect", rc_view, rc_model)
	var empty_corner := Vector2.INF
	for corner in [rc_art_rect.position + Vector2(1, 1),
			Vector2(rc_art_rect.end.x - 1, rc_art_rect.position.y + 1),
			Vector2(rc_art_rect.position.x + 1, rc_art_rect.end.y - 1),
			rc_art_rect.end - Vector2(1, 1)]:
		if Rect2(Vector2.ZERO, viewport_size).has_point(corner) \
				and rc_screen_zoomed.distance_to(corner) > main.CLICK_RADIUS + 5.0 \
				and float(main.call("_model_ray_hit_depth", rc_model,
						camera.project_ray_origin(corner), camera.project_ray_normal(corner))) < 0.0:
			empty_corner = corner
			break
	if empty_corner == Vector2.INF:
		_fail("no visible empty corner exists in the Research Center's projected bounds")
		return
	if main.call("_building_at", empty_corner, 0) == rc_view:
		_fail("empty ground in the Research Center's bounding box selected the Research Center")
		return
	# HPA-697 fix round 1: the transparent corner must also lose to an
	# overlapping eligible unit through a real left-click. Assert the setup
	# first — the corner yields no building hit at all (no opaque art, no
	# 20px fallback) — then hop a worker under the projected bounds and
	# require the click to select only that worker.
	var corner_hit: Dictionary = main.call("_building_hit_at", empty_corner, 0)
	if corner_hit.get("view") != null or not is_inf(float(corner_hit.get("depth", INF))):
		_fail("the Research Center empty corner is not a transparent miss: hit=%s" % str(corner_hit))
		return
	var corner_workers: Array[Node3D] = []
	for unit_node in get_tree().get_nodes_in_group("unit_views"):
		var view := unit_node as Node3D
		if view != null and int(view.get_meta("team_id", -1)) == 1:
			corner_workers.append(view)
	if corner_workers.is_empty():
		_fail("no Team-1 worker remains for the transparent-corner click leg")
		return
	var corner_origin := camera.project_ray_origin(empty_corner)
	var corner_dir := camera.project_ray_normal(empty_corner)
	var corner_ground := corner_origin + corner_dir * (-corner_origin.y / corner_dir.y)
	var corner_cells: Array[Vector2] = []
	for offset in [Vector2.ZERO, Vector2(1, 0), Vector2(-1, 0), Vector2(0, 1), Vector2(0, -1),
			Vector2(2, 0), Vector2(-2, 0), Vector2(0, 2), Vector2(0, -2)]:
		corner_cells.append(Vector2(floori(corner_ground.x) + 0.5, floori(corner_ground.z) + 0.5) + offset)
	var corner_worker: Node3D = corner_workers[0]
	if await _hop_worker_to_first_cell(corner_worker, corner_cells, 0.45, "transparent-corner") == Vector2.INF:
		return
	var corner_unit_screen := camera.unproject_position(corner_worker.global_position)
	var corner_hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	var corner_click := Vector2.INF
	for dy in range(-16, 17, 4):
		for dx in range(-16, 17, 4):
			var candidate := corner_unit_screen + Vector2(float(dx), float(dy))
			if candidate.distance_to(corner_unit_screen) > main.CLICK_RADIUS - 2.0:
				continue
			if not Rect2(Vector2.ZERO, viewport_size).has_point(candidate) \
					or corner_hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(candidate)):
				continue
			var probe_hit: Dictionary = main.call("_building_hit_at", candidate, 0)
			if probe_hit.get("view") != null or not is_inf(float(probe_hit.get("depth", INF))):
				continue
			if not rc_art_rect.has_point(candidate):
				continue
			if float(main.call("_model_ray_hit_depth", rc_model,
					camera.project_ray_origin(candidate), camera.project_ray_normal(candidate))) >= 0.0:
				continue
			if (main.call("_nearest_view", "unit_views", candidate, 1) as Node3D) != corner_worker:
				continue
			corner_click = candidate
			break
		if corner_click != Vector2.INF:
			break
	if corner_click == Vector2.INF:
		_fail("no transparent in-bounds pixel overlaps the hopped worker for the transparent-corner click")
		return
	_mouse_click(corner_click, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return (main.selected_ids as Array).size() == 1 \
				and int(main.selected_ids[0]) == int(corner_worker.get_meta("unit_id", -1)) \
				and int(main.selected_building_id) == -1,
			5.0, "the transparent corner click inside the Research Center's bounds did not select only the overlapping worker (units=%s building=%s)" % [str(main.selected_ids), str(main.selected_building_id)]):
		return
	# Park the spent worker south of the footprint so the roof-pick scans
	# below keep their unit-free pixels.
	var rc_catalogue: Dictionary = GrusBridge.catalogue_snapshot().get("buildings", {}).get("ResearchCenter", {})
	var rc_half_depth := float(rc_catalogue.get("height", 0)) * 0.5
	var park_cells: Array[Vector2] = []
	for south in [2.0, 3.0, 4.0, 5.0]:
		park_cells.append(Vector2(floori(rc_view.global_position.x) + 0.5, floori(rc_view.global_position.z + rc_half_depth + south) + 0.5))
	if await _hop_worker_to_first_cell(corner_worker, park_cells, 0.9, "transparent-corner park") == Vector2.INF:
		return
	var rc_roof_point := _roof_pick_point(rc_view, camera, viewport_size, main)
	if rc_roof_point == Vector2.INF:
		_fail("Research Center painted silhouette exposes no opaque art pixel beyond the old center radius")
		return
	var clear_point := _clear_click_point(viewport_size, main)
	if clear_point == Vector2.INF:
		_fail("no empty pixel exists for the pre-pick deselection at maximum zoom")
		return
	_mouse_click(clear_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == -1 and main.selected_ids.is_empty(),
			5.0, "the pre-pick deselection click did not clear the Bunkhouse selection"):
		return
	_mouse_click(rc_roof_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == 1 \
				and main.selected_ids.is_empty(),
			5.0, "left-click on the Research Center's projected roof did not select building 1"):
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

	# HPA-291/HPA-697 model state leg: every building view carries a Model
	# — the shared painted construction-site sprite exactly while the
	# snapshot reports incomplete, its painted kind sprite once complete
	# except the retained Wall/Gate GLBs. The seeded-complete Research
	# Center guarantees a finished-model proof regardless of how far the
	# construction queue drained before the clear-out move leg;
	# economy_smoke owns the full site→sprite completion-swap journey.
	var model_offender := ["none"]
	if not await _wait_until(func():
			model_offender[0] = "a building view has no Model"
			var finished := 0
			for node in get_tree().get_nodes_in_group("building_views"):
				var view := node as Node3D
				var id := int(view.get_meta("building_id", -1))
				var kind := str(view.get_meta("building_kind", ""))
				if view.get_node_or_null("Model") == null:
					model_offender[0] = "%s %d has no Model" % [kind, id]
					return false
				if view.is_processing():
					model_offender[0] = "%s %d still polls _process after styling" % [kind, id]
					return false
				var complete := bool(GrusBridge.building_snapshot(id).get("complete", false))
				var model_error := SmokeHelpers.building_model_error(view, kind, complete)
				if not model_error.is_empty():
					model_offender[0] = "%s %d complete=%s: %s" % [kind, id, complete, model_error]
					return false
				var art_error := SmokeHelpers.view_art_error(view)
				if not art_error.is_empty():
					model_offender[0] = "%s %d: %s" % [kind, id, art_error]
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
	if not await _wait_until(func():
			var restarted_view := get_tree().get_nodes_in_group("building_views")[0] as Node3D
			return restarted_view != null \
				and SmokeHelpers.building_model_error(restarted_view, "ResearchCenter", true).is_empty(),
			5.0, "restart left the new Research Center without its painted sprite"):
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

	if not GrusBridge.start_match():
		_fail("radius-steal leg could not reopen the match")
		return
	var steal_rc := _building_view(1)
	if steal_rc == null or (steal_rc.get_node_or_null("Model") as Node3D) == null:
		_fail("restarted Research Center view is missing its model for the radius-steal leg")
		return
	var rc_footprint_depth := float(building_catalogue.get("ResearchCenter", {}).get("height", 0))
	if rc_footprint_depth <= 0.0:
		_fail("catalogue exposes no Research Center height for the radius-steal leg")
		return
	var steal_row := floori(steal_rc.global_position.z - rc_footprint_depth * 0.5) - 2
	var anchor_x_min := floori(steal_rc.global_position.x - rc_footprint_depth * 0.5)
	var anchor_candidates: Array[Vector2i] = []
	for x in range(anchor_x_min - 1, anchor_x_min + int(rc_footprint_depth) + 2):
		anchor_candidates.append(Vector2i(x, steal_row))
	var rc_ground_x := steal_rc.global_position.x
	anchor_candidates.sort_custom(func(a: Vector2i, b: Vector2i) -> bool:
		return absf(a.x + 0.5 - rc_ground_x) < absf(b.x + 0.5 - rc_ground_x))
	var wall_anchor := Vector2i(-1, -1)
	for candidate_anchor in anchor_candidates:
		var anchor_preview: Dictionary = GrusBridge.placement_preview(1, "Wall", candidate_anchor.x, candidate_anchor.y)
		if bool(anchor_preview.get("valid", false)):
			wall_anchor = candidate_anchor
			break
	if wall_anchor.x < 0:
		_fail("no valid Wall anchor along the Research Center's north edge")
		return
	if not GrusBridge.place_building(1, "Wall", wall_anchor.x, wall_anchor.y):
		_fail("radius-steal Wall placement was not queued")
		return
	if not GrusBridge.set_sim_speed(20.0):
		_fail("could not accelerate the radius-steal Wall construction")
		return
	var steal_wall_box := [null]
	if not await _wait_until(func():
			for node in get_tree().get_nodes_in_group("building_views"):
				var view := node as Node3D
				if str(view.get_meta("building_kind", "")) != "Wall":
					continue
				var view_model := view.get_node_or_null("Model") as Node3D
				if view_model == null or not view_model.scene_file_path.ends_with("wall.glb"):
					continue
				if bool(GrusBridge.building_snapshot(int(view.get_meta("building_id", -1))).get("complete", false)):
					steal_wall_box[0] = view
					return true
			return false,
			30.0, "the placed Wall never completed with its styled model"):
		return
	var steal_wall := steal_wall_box[0] as Node3D
	var park_ids := PackedInt32Array()
	for unit_node in get_tree().get_nodes_in_group("unit_views"):
		if int(unit_node.get_meta("team_id", -1)) == 1:
			park_ids.append(int(unit_node.get_meta("unit_id")))
	if park_ids.is_empty():
		_fail("no Team-1 workers remain for the radius-steal park")
		return
	var park_target := Vector2(steal_rc.global_position.x - 27.5, steal_rc.global_position.z - 3.5)
	var park_revision := int(GrusBridge.command_feedback_revision())
	if not GrusBridge.move_units(park_ids, park_target):
		_fail("radius-steal worker park move was not queued")
		return
	if not await _wait_until(_accepted_since.bind(park_revision), 10.0,
			"the radius-steal park move was not accepted by the sim"):
		return
	# Stationary-gate regression probe: the 27-cell park march keeps every
	# worker under a live MoveOrder across several 20 Hz ticks even at 20x,
	# and a marching worker keeps WorkerTask::Idle — so an Idle gate that
	# reads the task snapshot alone reports a mover "idle". The gate must
	# consult economy_snapshot's idle set (Worker + Idle + no MoveOrder).
	for unit_node in get_tree().get_nodes_in_group("unit_views"):
		var marching := unit_node as Node3D
		if marching != null and int(marching.get_meta("team_id", -1)) == 1 \
				and _unit_idle(marching):
			_fail("the stationary gate reported worker %d idle mid-march on the radius-steal park" % int(marching.get_meta("unit_id", -1)))
			return
	if not await _wait_until(func():
			for unit_node in get_tree().get_nodes_in_group("unit_views"):
				var unit := unit_node as Node3D
				if unit == null or int(unit.get_meta("team_id", -1)) != 1:
					continue
				if unit.global_position.distance_to(Vector3(park_target.x, 0.0, park_target.y)) > 5.0:
					return false
			return true,
			30.0, "radius-steal workers never parked clear of the Research Center"):
		return
	if not GrusBridge.set_sim_speed(1.0):
		_fail("could not restore sim speed after the radius-steal park")
		return
	SmokeHelpers.push_mouse_click(get_tree(),
		_minimap_click_point(minimap, Vector2(steal_rc.global_position.x, steal_rc.global_position.z)),
		MOUSE_BUTTON_LEFT)
	await get_tree().process_frame
	await get_tree().process_frame
	for _step in range(16):
		if camera.size >= default_camera_size:
			break
		_mouse_click(viewport_center, MOUSE_BUTTON_WHEEL_DOWN)
		await get_tree().process_frame
	if not is_equal_approx(camera.size, default_camera_size):
		_fail("wheel zoom never restored the default camera size for the radius-steal pick: %s" % camera.size)
		return
	var steal_point := _radius_steal_pick_point(steal_rc, steal_wall, camera, viewport_size, main)
	if steal_point == Vector2.INF:
		_fail("no Research Center opaque art pixel lands inside the Wall's center radius while staying outside its art rect")
		return
	var wall_screen := camera.unproject_position(steal_wall.global_position)
	var rc_screen_steal := camera.unproject_position(steal_rc.global_position)
	var radius_hit := main.call("_building_at", steal_point, 1) as Node3D
	if radius_hit != steal_rc:
		_fail("a Research Center roof pixel %.2fpx from the Wall center and %.2fpx from the Research Center center picked %s — the legacy center radius let the neighbor steal the click" % [
			wall_screen.distance_to(steal_point), rc_screen_steal.distance_to(steal_point),
			str(radius_hit.get_meta("building_kind", radius_hit)) if radius_hit != null else "nothing"])
		return
	# Opt-in real-render evidence of the unit/building overlap framing.
	var steal_capture := await SmokeHelpers.capture_frame(get_tree(), "radius-steal-framing")
	if not steal_capture.is_empty():
		_fail(steal_capture)
		return
	var deselect_screen := _clear_click_point(viewport_size, main)
	if deselect_screen == Vector2.INF:
		_fail("no empty pixel exists for the pre-steal deselection")
		return
	_mouse_click(deselect_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == -1 and main.selected_ids.is_empty(),
			5.0, "the pre-steal deselection click did not clear the selection"):
		return
	_mouse_click(steal_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == 1 and main.selected_ids.is_empty(),
			5.0, "left-click on the Research Center roof beside the styled Wall selected building %d units=%s" % [
				int(main.selected_building_id), main.selected_ids]):
		return
	var fallback_screen := _clear_click_point(viewport_size, main)
	if fallback_screen == Vector2.INF:
		_fail("no empty pixel exists for the unstyled-view fallback probe")
		return
	var fallback_origin := camera.project_ray_origin(fallback_screen)
	var fallback_direction := camera.project_ray_normal(fallback_screen)
	if absf(fallback_direction.y) < 0.0001:
		_fail("the fallback-probe camera ray never reaches the ground")
		return
	var fallback_ground := fallback_origin + fallback_direction * (-fallback_origin.y / fallback_direction.y)
	var unready_view := building_view_scene.instantiate() as Node3D
	unready_view.set_meta("team_id", 1)
	unready_view.position = Vector3(fallback_ground.x, 0.0, fallback_ground.z)
	unready_view.visible = true
	add_child(unready_view)
	await get_tree().process_frame
	if (main.call("_building_at", fallback_screen, 0) as Node3D) != unready_view:
		_fail("the center-radius fallback did not pick the unstyled building view")
		return
	var fallback_side := Vector2(signf(viewport_center.x - fallback_screen.x) * (main.CLICK_RADIUS + 5.0), 0.0)
	if (main.call("_building_at", fallback_screen + fallback_side, 0) as Node3D) == unready_view:
		_fail("the unstyled view was picked beyond its center-radius fallback")
		return
	# HPA-697 fix round 1: the fallback must be proven as such — the view is
	# returned with INF depth, never an opaque hit — and a real left-click
	# with an eligible unit on the same pixel must select only that unit.
	var fallback_setup: Dictionary = main.call("_building_hit_at", fallback_screen, 0)
	if fallback_setup.get("view") != unready_view or not is_inf(float(fallback_setup.get("depth", INF))):
		_fail("the unstyled-view probe point is not a genuine fallback hit (view with INF depth): hit=%s" % str(fallback_setup))
		return
	var fallback_workers: Array[Node3D] = []
	for unit_node in get_tree().get_nodes_in_group("unit_views"):
		var view := unit_node as Node3D
		if view != null and int(view.get_meta("team_id", -1)) == 1:
			fallback_workers.append(view)
	if fallback_workers.is_empty():
		_fail("no Team-1 worker remains for the fallback-vs-unit click leg")
		return
	var fallback_cells: Array[Vector2] = []
	for offset in [Vector2.ZERO, Vector2(1, 0), Vector2(-1, 0), Vector2(0, 1), Vector2(0, -1),
			Vector2(2, 0), Vector2(-2, 0), Vector2(0, 2), Vector2(0, -2)]:
		fallback_cells.append(Vector2(floori(fallback_ground.x) + 0.5, floori(fallback_ground.z) + 0.5) + offset)
	var fallback_worker: Node3D = fallback_workers[0]
	if await _hop_worker_to_first_cell(fallback_worker, fallback_cells, 0.45, "fallback-vs-unit") == Vector2.INF:
		return
	var fallback_unit_screen := camera.unproject_position(fallback_worker.global_position)
	if fallback_unit_screen.distance_to(fallback_screen) > main.CLICK_RADIUS - 2.0:
		_fail("the hopped fallback worker's projection left the fallback pixel (%.1fpx)" % fallback_unit_screen.distance_to(fallback_screen))
		return
	_mouse_click(fallback_screen, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return (main.selected_ids as Array).size() == 1 \
				and int(main.selected_ids[0]) == int(fallback_worker.get_meta("unit_id", -1)) \
				and int(main.selected_building_id) == -1,
			5.0, "the 20px fallback building stole the left-click from the overlapping unit (units=%s building=%s)" % [str(main.selected_ids), str(main.selected_building_id)]):
		return
	unready_view.free()

	# HPA-697 Step 2 unit/building overlap legs through real input. The
	# radius-steal park keeps every other worker clear, so each hop leaves
	# exactly one eligible unit near the Research Center's scanned pixels.
	var overlap_workers: Array[Node3D] = []
	for unit_node in get_tree().get_nodes_in_group("unit_views"):
		var view := unit_node as Node3D
		if view != null and int(view.get_meta("team_id", -1)) == 1:
			overlap_workers.append(view)
	overlap_workers.sort_custom(func(a: Node3D, b: Node3D) -> bool:
		return int(a.get_meta("unit_id", -1)) < int(b.get_meta("unit_id", -1)))
	if overlap_workers.size() < 3:
		_fail("fewer than three Team-1 workers remain for the overlap legs")
		return
	var overlap_rc := steal_rc
	var rc_center := overlap_rc.global_position
	# Unit in front of the painted facade keeps the click: hop a worker just
	# south of the footprint and click an opaque pixel deeper than its +0.15
	# body reference.
	var south_spot := Vector2(floori(rc_center.x) + 0.5, floori(rc_center.z + rc_footprint_depth * 0.5 + 1.0) + 0.5)
	if not await _hop_worker(overlap_workers[0], south_spot, 1.6, "overlap-front"):
		return
	var front_point := _overlap_pick_point(overlap_rc, overlap_workers[0], true, camera, viewport_size, main)
	if front_point == Vector2.INF:
		_fail("no opaque Research Center pixel sits deeper than the south worker's body reference")
		return
	if (main.call("_unit_pick_at", front_point, 1, main.call("_building_hit_at", front_point, 0)) as Node3D) != overlap_workers[0]:
		_fail("the front worker left the scanned overlap pick point before the click")
		return
	_mouse_click(front_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(overlap_workers[0].get_meta("unit_id", -1)) in main.selected_ids \
				and int(main.selected_building_id) == -1,
			5.0, "the Research Center facade stole the click from the worker in front of it (units=%s building=%s)" % [str(main.selected_ids), str(main.selected_building_id)]):
		return
	# Opt-in real-render evidence: the selected worker stands in front of the
	# painted facade it just out-competed for this click.
	if SmokeHelpers.capture_enabled():
		var overlap_front_capture := await SmokeHelpers.capture_frame(get_tree(), "overlap-front")
		if not overlap_front_capture.is_empty():
			_fail(overlap_front_capture)
			return
	# Far unit behind the opaque roof loses: hop a second worker just north
	# of the footprint and click an opaque pixel strictly in front of that
	# body reference — the silhouette must win the click.
	var north_spot := Vector2(floori(rc_center.x) + 0.5, floori(rc_center.z - rc_footprint_depth * 0.5 - 1.0) + 0.5)
	if not await _hop_worker(overlap_workers[1], north_spot, 1.6, "overlap-behind"):
		return
	var behind_point := _overlap_pick_point(overlap_rc, overlap_workers[1], false, camera, viewport_size, main)
	if behind_point == Vector2.INF:
		_fail("no opaque Research Center pixel sits in front of the north worker's body reference")
		return
	if (main.call("_unit_pick_at", behind_point, 1) as Node3D) != overlap_workers[1]:
		_fail("the behind worker left the scanned overlap pick point before the click")
		return
	_mouse_click(behind_point, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == int(overlap_rc.get_meta("building_id", -1)) \
				and (main.selected_ids as Array).is_empty(),
			5.0, "the unit behind the opaque roof stole the facade click (units=%s building=%s)" % [str(main.selected_ids), str(main.selected_building_id)]):
		return
	# Opt-in real-render evidence: the opaque painted roof won this click over
	# the worker standing behind it, so the frame shows the selected Research
	# Center with the occluded worker hidden by the art.
	if SmokeHelpers.capture_enabled():
		var overlap_behind_capture := await SmokeHelpers.capture_frame(get_tree(), "overlap-behind")
		if not overlap_behind_capture.is_empty():
			_fail(overlap_behind_capture)
			return
	# The depth contract holds at both authored framings (spec): repeat the
	# behind-roof click at the wheel-zoomed-out view through real input,
	# then restore the default size for the later legs.
	var overlap_default_size := camera.size
	for _step in 3:
		_mouse_click(viewport_size * 0.5, MOUSE_BUTTON_WHEEL_DOWN)
		await get_tree().process_frame
	if camera.size < PRIOR_CAMERA_SIZE:
		_fail("three wheel-out steps reached only %.1f, short of the %.1f zoom-out overlap framing" % [camera.size, PRIOR_CAMERA_SIZE])
		return
	var zoomout_clear := _clear_click_point(viewport_size, main)
	if zoomout_clear == Vector2.INF:
		_fail("no empty pixel exists to deselect before the zoom-out overlap click")
		return
	_mouse_click(zoomout_clear, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == -1 and (main.selected_ids as Array).is_empty(),
			5.0, "the pre-zoom-out deselection click did not clear the selection"):
		return
	var behind_point_zoomout := _overlap_pick_point(overlap_rc, overlap_workers[1], false, camera, viewport_size, main)
	if behind_point_zoomout == Vector2.INF:
		_fail("no opaque Research Center pixel sits in front of the north worker at the zoom-out framing")
		return
	if (main.call("_unit_pick_at", behind_point_zoomout, 1) as Node3D) != overlap_workers[1]:
		_fail("the behind worker left the zoom-out overlap pick point before the click")
		return
	_mouse_click(behind_point_zoomout, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == int(overlap_rc.get_meta("building_id", -1)) \
				and (main.selected_ids as Array).is_empty(),
			5.0, "the zoom-out behind-roof click did not select the Research Center (units=%s building=%s)" % [str(main.selected_ids), str(main.selected_building_id)]):
		return
	# Opt-in real-render evidence: the same opaque-roof-wins verdict at the
	# zoomed-out framing.
	if SmokeHelpers.capture_enabled():
		var overlap_zoomout_capture := await SmokeHelpers.capture_frame(get_tree(), "overlap-behind-zoomout")
		if not overlap_zoomout_capture.is_empty():
			_fail(overlap_zoomout_capture)
			return
	# The spec's overlap contract spans both directions at both framings, so
	# the south worker's facade click must also win while still zoomed out:
	# deselect, then click the same front-of-silhouette pixel class.
	var zoomout_front_clear := _clear_click_point(viewport_size, main)
	if zoomout_front_clear == Vector2.INF:
		_fail("no empty pixel exists to deselect before the zoom-out front-worker click")
		return
	_mouse_click(zoomout_front_clear, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(main.selected_building_id) == -1 and (main.selected_ids as Array).is_empty(),
			5.0, "the pre-zoom-out-front deselection click did not clear the selection"):
		return
	var front_point_zoomout := _overlap_pick_point(overlap_rc, overlap_workers[0], true, camera, viewport_size, main)
	if front_point_zoomout == Vector2.INF:
		_fail("no opaque Research Center pixel sits deeper than the south worker's body reference at the zoom-out framing")
		return
	if (main.call("_unit_pick_at", front_point_zoomout, 1, main.call("_building_hit_at", front_point_zoomout, 0)) as Node3D) != overlap_workers[0]:
		_fail("the front worker left the zoom-out overlap pick point before the click")
		return
	_mouse_click(front_point_zoomout, MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(overlap_workers[0].get_meta("unit_id", -1)) in main.selected_ids \
				and int(main.selected_building_id) == -1,
			5.0, "the Research Center facade stole the zoom-out click from the worker in front of it (units=%s building=%s)" % [str(main.selected_ids), str(main.selected_building_id)]):
		return
	# Opt-in real-render evidence: the front-worker-wins verdict at the
	# zoomed-out framing.
	if SmokeHelpers.capture_enabled():
		var overlap_front_zoomout_capture := await SmokeHelpers.capture_frame(get_tree(), "overlap-front-zoomout")
		if not overlap_front_zoomout_capture.is_empty():
			_fail(overlap_front_zoomout_capture)
			return
	for _step in range(16):
		if is_equal_approx(camera.size, overlap_default_size):
			break
		_mouse_click(viewport_size * 0.5, MOUSE_BUTTON_WHEEL_UP)
		await get_tree().process_frame
	if not is_equal_approx(camera.size, overlap_default_size):
		_fail("wheel zoom never restored the default camera size after the zoom-out overlap leg: %s" % camera.size)
		return
	# A completed Gate's shallow plinth must not take a Worker's click: place
	# a real Gate through the bridge, march a worker into its corridor cell
	# and click the worker's own projection.
	var gate_anchor := Vector2i(-1, -1)
	var gate_hud_rects := SmokeHelpers.hud_rects(get_node("Main/HUD"))
	for dy in range(2, 9):
		for dx in range(-6, 7):
			var candidate := Vector2i(floori(rc_center.x) + dx, floori(rc_center.z + rc_footprint_depth * 0.5) + dy)
			var near_unit := false
			for unit_node in get_tree().get_nodes_in_group("unit_views"):
				var unit_view := unit_node as Node3D
				if unit_view != null and Vector2(unit_view.global_position.x, unit_view.global_position.z) \
						.distance_to(Vector2(float(candidate.x) + 0.5, float(candidate.y) + 0.5)) < 1.5:
					near_unit = true
					break
			if near_unit:
				continue
			var preview: Dictionary = GrusBridge.placement_preview(1, "Gate", candidate.x, candidate.y)
			if not bool(preview.get("valid", false)):
				continue
			var cell_screen := camera.unproject_position(Vector3(float(candidate.x) + 0.5, 0.0, float(candidate.y) + 0.5))
			if not Rect2(Vector2.ZERO, viewport_size).has_point(cell_screen) \
					or gate_hud_rects.any(func(rect: Rect2) -> bool: return rect.has_point(cell_screen)):
				continue
			gate_anchor = candidate
			break
		if gate_anchor.x >= 0:
			break
	if gate_anchor.x < 0:
		_fail("no valid on-screen Gate anchor south of the Research Center for the corridor click")
		return
	if not GrusBridge.place_building(1, "Gate", gate_anchor.x, gate_anchor.y):
		_fail("the corridor-click Gate placement was not queued")
		return
	var gate_box := [null]
	if not await _wait_until(func():
			for node in get_tree().get_nodes_in_group("building_views"):
				var view := node as Node3D
				if str(view.get_meta("building_kind", "")) != "Gate":
					continue
				gate_box[0] = view
				return true
			return false,
			10.0, "the placed Gate never produced a building view"):
		return
	var gate_view := gate_box[0] as Node3D
	var gate_id := int(gate_view.get_meta("building_id", -1))
	if not await _wait_until(func():
			return bool(GrusBridge.building_snapshot(gate_id).get("complete", false)) \
				and SmokeHelpers.building_model_error(gate_view, "Gate", true).is_empty(),
			30.0, "the corridor-click Gate never completed with its styled model"):
		return
	var gate_worker := overlap_workers[2]
	# A Gate cell rejects move goals (the sim only clears it as a waypoint),
	# so stage the worker south of the Gate, march it THROUGH the corridor
	# and stop it inside — then click its own projection like a crossing
	# Worker's click.
	var gate_center := Vector2(gate_view.global_position.x, gate_view.global_position.z)
	var stage_cell := Vector2(float(gate_anchor.x) + 0.5, float(gate_anchor.y) + 2.5)
	if not await _hop_worker(gate_worker, stage_cell, 1.0, "gate-stage"):
		return
	var corridor_revision := int(GrusBridge.command_feedback_revision())
	# The Worker moves 6 cells/s (0.3 per 20 Hz tick): at windowed
	# software-render frame rates the synced view advances further than the
	# 0.3-cell entry window between two rendered samples, so the mid-corridor
	# transit is only catchable at a reduced sim rate. Headless sampling is
	# unaffected either way; the march speed itself never changes.
	if not GrusBridge.set_sim_speed(0.25):
		_fail("could not slow the sim for the gate corridor crossing")
		return
	if not GrusBridge.move_units(PackedInt32Array([int(gate_worker.get_meta("unit_id", -1))]), Vector2(float(gate_anchor.x) + 0.5, float(gate_anchor.y) - 0.5)):
		_fail("the gate crossing move was not queued")
		return
	if not await _wait_until(_accepted_since.bind(corridor_revision), 10.0,
			"the gate crossing move was not accepted by the sim"):
		return
	var gate_worker_id := int(gate_worker.get_meta("unit_id", -1))
	if not await _wait_until(func():
			var view := _unit_view(gate_worker_id)
			return view != null and Vector2(view.global_position.x, view.global_position.z).distance_to(gate_center) <= 0.15,
			10.0, "the crossing worker never entered the Gate corridor"):
		return
	if not GrusBridge.stop_units(PackedInt32Array([gate_worker_id])):
		_fail("the corridor stop was not queued")
		return
	if not await _wait_until(func():
			var view := _unit_view(gate_worker_id)
			return view != null and _unit_idle(view) \
				and Vector2(view.global_position.x, view.global_position.z).distance_to(gate_center) <= 0.45,
			5.0, "the corridor worker never settled inside the Gate cell"):
		return
	for _settle in 3:
		await get_tree().process_frame
	# Opt-in real-render evidence: a Worker stopped inside the finished Gate's
	# corridor — the clickable crossing the depth contract protects.
	if SmokeHelpers.capture_enabled():
		var gate_crossing_capture := await SmokeHelpers.capture_frame(get_tree(), "gate-crossing")
		if not gate_crossing_capture.is_empty():
			_fail(gate_crossing_capture)
			return
	if not GrusBridge.set_sim_speed(1.0):
		_fail("could not restore sim speed after the gate corridor crossing")
		return
	# The +0.15 body anchor is what keeps a crossing Worker selectable: at
	# this framing the retained Gate GLB yields a real plinth hit just
	# behind the anchor, so pin the exact comparison — a loosened anchor or
	# a flipped rule would hand the click to the Gate and fail both this
	# check and the real selection below.
	var anchor_screen: Vector2 = camera.unproject_position(gate_worker.global_position)
	var anchor_hit: Dictionary = main.call("_building_hit_at", anchor_screen, 1)
	var anchor_origin: Vector3 = camera.project_ray_origin(anchor_screen)
	var anchor_dir: Vector3 = camera.project_ray_normal(anchor_screen)
	var anchor_unit_depth: float = (gate_worker.global_position + Vector3(0.0, 0.15, 0.0) - anchor_origin).dot(anchor_dir)
	var anchor_building_depth: float = float(anchor_hit.get("depth", INF))
	if anchor_hit.get("view") != gate_view or anchor_building_depth >= INF:
		_fail("the corridor click no longer yields a real Gate plinth hit for the +0.15 anchor check (view=%s depth=%s)" % [str(anchor_hit.get("view")), str(anchor_building_depth)])
		return
	if anchor_building_depth <= anchor_unit_depth:
		_fail("the Gate plinth hit is no longer strictly behind the Worker's +0.15 body anchor (plinth=%.3f anchor=%.3f)" % [anchor_building_depth, anchor_unit_depth])
		return
	_mouse_click(camera.unproject_position(gate_worker.global_position), MOUSE_BUTTON_LEFT)
	if not await _wait_until(func():
			return int(gate_worker.get_meta("unit_id", -1)) in main.selected_ids \
				and int(main.selected_building_id) == -1,
			5.0, "the completed Gate stole the click from the Worker inside its corridor (units=%s building=%s)" % [str(main.selected_ids), str(main.selected_building_id)]):
		return

	# The plinth click above covers only the shallow threshold: a completed
	# Gate also spans an overhead beam ring across the corridor, and a
	# Worker parked just inside its rear half projects its upper body over
	# beam pixels the beam can be nearer than the feet anchor yet still sit
	# behind — the one place the feet-reference rule could hand the click to
	# the Gate although the Worker's mesh occludes the beam there. March the
	# Worker through the corridor at 0.05x and, per physics frame inside
	# that band, scan its screen column for a beam pixel its mesh covers —
	# the verdict click fires only on a pick re-derived at the post-deselect
	# position (the run's middle row), so march drift can never slide the
	# ray off the mesh edge into the anchor rule it is meant to discriminate. The
	# deselection click first empties the selection so the verdict can only
	# ever come from the beam click itself, never a stale selection.
	var beam_verdict := false
	var beam_gate_depth := -1.0
	var beam_unit_depth := -1.0
	if not GrusBridge.set_sim_speed(0.05):
		_fail("could not slow the sim for the beam-overlap transit")
		return
	var beam_deadline := Time.get_ticks_msec() + 15000
	for beam_goal in [Vector2(gate_center.x, gate_center.y - 2.0),
			Vector2(gate_center.x, gate_center.y + 2.0)]:
		if beam_verdict or Time.get_ticks_msec() >= beam_deadline:
			break
		var beam_revision := int(GrusBridge.command_feedback_revision())
		if not GrusBridge.move_units(PackedInt32Array([gate_worker_id]), beam_goal):
			_fail("the beam-overlap transit move was not queued")
			return
		if not await _wait_until(_accepted_since.bind(beam_revision), 10.0,
				"the beam-overlap transit move was not accepted by the sim"):
			return
		while not beam_verdict and Time.get_ticks_msec() < beam_deadline:
			var transit_view := _unit_view(gate_worker_id)
			if transit_view == null:
				break
			var transit_z := transit_view.global_position.z
			if transit_z > gate_center.y - 0.55 and transit_z < gate_center.y + 0.1:
				var transit_model := transit_view.get_node_or_null("Model") as Node3D
				if transit_model != null:
					var beam_pick := _beam_overlap_pick(transit_view, transit_model,
							camera, main, gate_view)
					if not beam_pick.is_empty():
						var beam_clear := _clear_click_point(viewport_size, main)
						if beam_clear == Vector2.INF:
							_fail("no empty pixel exists to deselect before the beam-overlap click")
							return
						_mouse_click(beam_clear, MOUSE_BUTTON_LEFT)
						# The deselection must land before the verdict click:
						# the corridor click already selected this Worker, and
						# a verdict poll that samples the stale selection
						# would pass without the beam click ever being read.
						if not await _wait_until(func():
								return int(main.selected_building_id) == -1 \
									and (main.selected_ids as Array).is_empty(),
								3.0, "the pre-beam deselection click did not clear the selection"):
							return
						# Re-verify at the post-deselect position: the march
						# moved the view meanwhile, so the interior pixel is
						# re-derived — never the stale first sample.
						var verify_view := _unit_view(gate_worker_id)
						if verify_view != null:
							var verify_model := verify_view.get_node_or_null("Model") as Node3D
							if verify_model != null:
								beam_pick = _beam_overlap_pick(verify_view, verify_model,
										camera, main, gate_view)
						if not beam_pick.is_empty():
							_mouse_click(beam_pick[0], MOUSE_BUTTON_LEFT)
							beam_gate_depth = float(beam_pick[1])
							beam_unit_depth = float(beam_pick[2])
			await get_tree().physics_frame
			if beam_gate_depth >= 0.0:
				# Three verdicts: the Worker wins (fixed rule), the Gate's
				# beam steals (the bug this leg exists to catch), or nothing
				# resolves — the march carried the mesh off the sampled pixel
				# before the click landed, a stale sample to retry, never a
				# verdict.
				var verdict_deadline := mini(beam_deadline, Time.get_ticks_msec() + 2000)
				while Time.get_ticks_msec() < verdict_deadline:
					if gate_worker_id in (main.selected_ids as Array) \
							or int(main.selected_building_id) != -1:
						break
					await get_tree().physics_frame
				if gate_worker_id in (main.selected_ids as Array):
					beam_verdict = true
				elif int(main.selected_building_id) == gate_id:
					_fail("the Gate's rear beam stole the upper-body click from the corridor Worker (beam=%.3f worker=%.3f units=%s building=%s)" % [
						beam_gate_depth, beam_unit_depth, str(main.selected_ids), str(main.selected_building_id)])
					return
				else:
					beam_gate_depth = -1.0
	if not GrusBridge.stop_units(PackedInt32Array([gate_worker_id])):
		_fail("the beam-overlap stop was not queued")
		return
	if not GrusBridge.set_sim_speed(1.0):
		_fail("could not restore sim speed after the beam-overlap click")
		return
	if not beam_verdict:
		_fail("the corridor transits never produced a Worker-in-front-of-beam overlap pixel")
		return
	if not await _wait_until(_unit_idle.bind(gate_worker), 10.0,
			"the corridor worker never idled after the beam-overlap stop"):
		return

	print("GRUS_GODOT_SMOKE_OK units=4 controls=input-derived camera=pan-zoomed framing=54_zoomout_72 unreachable=feedback cadence=20hz interpolation=worst_excess_%.3f fog=scout_reveal_hide explored_persist=true markers=fog_gated recenter=command_free pre_wave=true winter_art=gltf_exact_cover_input_gather structures=painted_catalogue_hookup gate_corridor=clear wall_gate=edge_contact wall_gate_run=edge_contact push_contract=cached_once silhouette_pick=opaque_sprite_rc radius_steal=rect_scoped unstyled_fallback=center_radius overlap_pick=depth_scoped_right_click presentation_overlap=labelled_probe" % worst_excess)
	var quit_controller = get_node("Main")
	await quit_controller.request_quit()
