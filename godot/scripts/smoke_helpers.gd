extends RefCounted

## Shared checks and live-view derivations for the headless smoke scenes.
## Coordinates come from live views and read-only bridge probes — fixture
## cells are never re-typed into test scripts.

## The grid cell under a live view's origin — the sim's world_to_cell.
static func cell_of(view: Node3D) -> Vector2i:
	return Vector2i(int(view.global_position.x), int(view.global_position.z))

## A building view's footprint anchor (northwest cell): the bridge stamps
## the view at the footprint center and scales it by the footprint size.
static func footprint_anchor(view: Node3D) -> Vector2i:
	return Vector2i(
		int(view.global_position.x - view.scale.x * 0.5),
		int(view.global_position.z - view.scale.z * 0.5))

## Pre-wave Playing smokes must finish before the first 300 simulated-second
## preparation countdown expires. A launched wave means the fixture choreo-
## graphy ran long enough for alien combat to intrude, which is a smoke
## contract failure even if the scripted interaction otherwise passed.
static func no_launched_wave_error(where: String) -> String:
	if not GrusBridge.has_method("wave_snapshot"):
		return "%s: wave_snapshot bridge method is missing" % where
	var snapshot: Dictionary = GrusBridge.wave_snapshot()
	if snapshot.is_empty() or not snapshot.has("wave"):
		return "%s: wave_snapshot is missing the launched wave number" % where
	if int(snapshot.get("wave", 0)) >= 1:
		return "smoke overran wave prep"
	return ""

## A valid Barracks anchor in the ring slot just west of the Research
## Center, probed through the read-only placement preview starting from the
## live view's footprint — never a copied fixture coordinate. Returns
## (-1,-1) when no candidate validates so the caller can fail loudly.
static func barracks_anchor(builder_id: int, research_center: Node3D) -> Vector2i:
	var rc_anchor := footprint_anchor(research_center)
	for offset in range(4, 12):
		var candidate := Vector2i(rc_anchor.x - offset, rc_anchor.y)
		var preview: Dictionary = GrusBridge.placement_preview(
			builder_id, "Barracks", candidate.x, candidate.y)
		if bool(preview.get("valid", false)):
			return candidate
	return Vector2i(-1, -1)

## Fog overlay contract: the two MultiMesh layers are discrete tiles
## compacted in row-major order by state — Unexplored (opaque) and
## Explored (translucent); Visible cells draw no tile. The buffers must
## equal the snapshot exactly — instance count, packed per-tile transform
## floats, layer visibility — and under the project's globally enabled
## physics interpolation each layer must be explicitly non-interpolated:
## a rebuild reassigns instance indices to different cells rather than
## moving a mesh, so an interpolated tile is a wrong tile, and buffer
## writes from _process trip the engine's outside-physics-transform
## warning. Returns "" on success, else the failure message.
static func fog_overlay_error(where: String, fog: Node, snap: Dictionary) -> String:
	var layers: Array[MultiMeshInstance3D] = [fog.get("_unexplored"), fog.get("_explored")]
	if layers[0] == null or layers[1] == null:
		return "%s: fog overlay layers are missing" % where
	if not bool(ProjectSettings.get_setting("physics/common/physics_interpolation", false)):
		return "%s: project physics interpolation is disabled — the non-interpolated overlay contract requires the global setting" % where
	for layer in layers:
		if layer.physics_interpolation_mode != Node.PHYSICS_INTERPOLATION_MODE_OFF:
			return "%s: fog overlay layer is not explicitly non-interpolated (physics_interpolation_mode=%d)" % [where, layer.physics_interpolation_mode]
		if layer.is_physics_interpolated_and_enabled():
			return "%s: fog overlay layer is still physics-interpolated — compacted tiles would lerp between unrelated cells" % where
	var width := int(snap.get("width", 0))
	var states: PackedInt32Array = snap.get("states", PackedInt32Array())
	if width <= 0 or states.is_empty() or states.size() % width != 0:
		return "%s: malformed visibility snapshot for the overlay check: %s" % [where, snap.keys()]
	for state in layers.size():
		var layer := layers[state]
		var multimesh := layer.multimesh
		var expected := 0
		for value in states:
			if value == state:
				expected += 1
		if multimesh.instance_count != expected:
			return "%s: fog layer %d drew %d tiles; snapshot has %d state-%d cells" % [where, state, multimesh.instance_count, expected, state]
		if layer.visible != (expected > 0):
			return "%s: fog layer %d visibility is %s; expected %s for %d tiles" % [where, state, layer.visible, expected > 0, expected]
		if expected == 0:
			continue
		# Exact tile buffer: 12 floats per TRANSFORM_3D tile — identity
		# basis (offsets 0/5/10) and the cell-center origin (offsets
		# 3/7/11) in the same row-major compaction order _fill writes.
		# get_buffer round-trips through the stored multimesh data on every
		# renderer, including the headless dummy driver.
		var buffer: PackedFloat32Array = multimesh.get_buffer()
		if buffer.size() != expected * 12:
			return "%s: fog layer %d buffer is %d floats; expected %d for %d tiles" % [where, state, buffer.size(), expected * 12, expected]
		var index := 0
		var x := 0
		var y := 0
		for value in states:
			if value == state:
				var o := index * 12
				var want_origin := Vector3(x + 0.5, float(fog.FOG_Y), y + 0.5)
				var got_origin := Vector3(buffer[o + 3], buffer[o + 7], buffer[o + 11])
				var basis_ok := is_equal_approx(buffer[o], 1.0) and is_zero_approx(buffer[o + 1]) \
						and is_zero_approx(buffer[o + 2]) and is_zero_approx(buffer[o + 4]) \
						and is_equal_approx(buffer[o + 5], 1.0) and is_zero_approx(buffer[o + 6]) \
						and is_zero_approx(buffer[o + 8]) and is_zero_approx(buffer[o + 9]) \
						and is_equal_approx(buffer[o + 10], 1.0)
				if not basis_ok or not got_origin.is_equal_approx(want_origin):
					return "%s: fog layer %d tile %d origin %s != cell (%d, %d) at %s" % [where, state, index, got_origin, x, y, want_origin]
				index += 1
			x += 1
			if x >= width:
				x = 0
				y += 1
	return ""
